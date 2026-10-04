use std::{
    collections::VecDeque,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, warn};
use x11rb::protocol::xtest::ConnectionExt as XTestConnectionExt;
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT},
    wrapper::ConnectionExt as _,
};

use crate::{
    backend::{AtspiBackend, BackendError},
    semantic::{
        BackendLocator, CachedSemanticNode, RuntimeNodeId, SemanticAction, SemanticCache,
        SemanticCapability, SemanticRole, SemanticState, TextInputKind,
    },
    transcompile::SceneBinding,
};

use super::observation::ObservationResult;

/// A user-level operation. It deliberately does not name an AT-SPI method or
/// a platform input primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticActionKind {
    Focus,
    Edit,
    Activate,
    Submit,
    RawEnter,
    Toggle,
    Select,
    Expand,
    Collapse,
    Increment,
    Decrement,
    Scroll,
}

/// A semantic request carrying any user payload. Provider action records use
/// `semantic::SemanticAction`; this type represents the user's intent instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticActuation {
    Edit(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundTarget {
    pub runtime_id: RuntimeNodeId,
    pub locator: BackendLocator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextTargetBinding {
    pub source_target: BoundTarget,
    pub edit_target: BoundTarget,
    pub focus_target: BoundTarget,
    pub readback_target: BoundTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditStrategy {
    AtspiSetContents,
    AtspiDeleteAndInsert,
    NativeKeyboardReplace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditDelivery {
    Accepted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditAttempt {
    pub strategy: TextEditStrategy,
    pub delivery: TextEditDelivery,
    pub observed_characters: Option<usize>,
    pub verified: bool,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditTrace {
    pub targets: TextTargetBinding,
    pub requested_characters: usize,
    pub attempts: Vec<TextEditAttempt>,
    pub window: Option<WindowTarget>,
    pub native_window: Option<NativeWindowHandle>,
    pub window_activation_verified: bool,
    pub focus_verified: bool,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditResult {
    pub observed_text: String,
    pub trace: TextEditTrace,
}

struct PreparedNativeInput {
    window: WindowTarget,
    native_window: NativeWindowHandle,
}

impl fmt::Display for SemanticActionKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Focus => "Focus",
            Self::Edit => "Edit",
            Self::Activate => "Activate",
            Self::Submit => "Submit",
            Self::RawEnter => "RawEnter",
            Self::Toggle => "Toggle",
            Self::Select => "Select",
            Self::Expand => "Expand",
            Self::Collapse => "Collapse",
            Self::Increment => "Increment",
            Self::Decrement => "Decrement",
            Self::Scroll => "Scroll",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeKey {
    Return,
}

impl NativeKey {
    fn keysym(self) -> u32 {
        // X11 keysyms from the public X11 protocol. Keeping these values here
        // avoids a second toolkit-specific key vocabulary in the TUI layer.
        match self {
            Self::Return => 0xff0d,
        }
    }
}

impl fmt::Display for NativeKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Return => "Return",
        })
    }
}

#[derive(Debug, Error)]
pub enum KeyboardBackendError {
    #[error("X11 keyboard backend is unavailable: {0}")]
    Unavailable(String),
    #[error("X11 keyboard backend could not resolve key {key}")]
    KeyUnavailable { key: NativeKey },
    #[error("X11 keyboard backend failed: {0}")]
    Protocol(String),
    #[error("X11 text input currently supports printable ASCII only")]
    UnsupportedText,
}

pub trait KeyboardBackend: Send + Sync {
    fn send_key(&self, key: NativeKey) -> Result<(), KeyboardBackendError>;
}

/// Platform text entry is separate from semantic action selection. It is only
/// called after the delivery component has verified the target window and
/// focus through the operation authority supplied by its caller.
pub trait NativeTextInputBackend: Send + Sync {
    fn replace_text(&self, text: &str) -> Result<(), KeyboardBackendError>;
}

#[async_trait]
trait EditableTextBackend: Send + Sync {
    async fn deliver_set_contents(
        &self,
        target: &BackendLocator,
        text: &str,
    ) -> Result<(), BackendError>;

    async fn deliver_delete_and_insert(
        &self,
        target: &BackendLocator,
        existing_characters: usize,
        text: &str,
    ) -> Result<(), BackendError>;

    async fn read_authoritative_text(
        &self,
        target: &BackendLocator,
    ) -> Result<String, BackendError>;
}

#[async_trait]
impl EditableTextBackend for AtspiBackend {
    async fn deliver_set_contents(
        &self,
        target: &BackendLocator,
        text: &str,
    ) -> Result<(), BackendError> {
        self.deliver_set_text_contents(target, text).await
    }

    async fn deliver_delete_and_insert(
        &self,
        target: &BackendLocator,
        existing_characters: usize,
        text: &str,
    ) -> Result<(), BackendError> {
        self.deliver_delete_and_insert_text(target, existing_characters, text)
            .await
    }

    async fn read_authoritative_text(
        &self,
        target: &BackendLocator,
    ) -> Result<String, BackendError> {
        AtspiBackend::read_authoritative_text(self, target).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeWindowHandle {
    X11(u64),
}

impl fmt::Display for NativeWindowHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X11(window) => write!(formatter, "0x{window:x}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowResolutionConfidence {
    ProcessUnique,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowTarget {
    pub accessible_node_id: RuntimeNodeId,
    pub accessible_locator: BackendLocator,
    pub process_id: Option<u32>,
    pub native_handle: Option<NativeWindowHandle>,
    pub confidence: Option<WindowResolutionConfidence>,
}

#[derive(Debug, Error)]
pub enum NativeWindowResolveError {
    #[error("X11 display is unavailable: {0}")]
    Unavailable(String),
    #[error("X11 window property protocol failed: {0}")]
    Protocol(String),
    #[error("no native X11 window exposes process id {0}")]
    NotFound(u32),
    #[error("multiple native X11 windows expose process id {0}")]
    Ambiguous(u32),
    #[error("target has no process identity")]
    MissingProcessId,
}

#[derive(Debug, Error)]
pub enum WindowActivationError {
    #[error("X11 display is unavailable: {0}")]
    Unavailable(String),
    #[error("X11 window activation protocol failed: {0}")]
    Protocol(String),
    #[error("native window handle is not supported by this activator")]
    UnsupportedHandle,
}

pub trait NativeWindowResolver: Send + Sync {
    fn resolve(
        &self,
        target: &WindowTarget,
    ) -> Result<NativeWindowHandle, NativeWindowResolveError>;
}

pub trait WindowActivator: Send + Sync {
    fn activate(&self, window: &NativeWindowHandle) -> Result<(), WindowActivationError>;
    fn is_active(&self, window: &NativeWindowHandle) -> Result<bool, WindowActivationError>;
}

#[async_trait]
pub trait FocusManager: Send + Sync {
    async fn request_focus(
        &self,
        backend: &AtspiBackend,
        target: &BackendLocator,
    ) -> Result<(), String>;

    async fn is_focused(
        &self,
        backend: &AtspiBackend,
        target: &BackendLocator,
    ) -> Result<bool, String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct X11WindowResolver;

impl NativeWindowResolver for X11WindowResolver {
    fn resolve(
        &self,
        target: &WindowTarget,
    ) -> Result<NativeWindowHandle, NativeWindowResolveError> {
        let process_id = target
            .process_id
            .ok_or(NativeWindowResolveError::MissingProcessId)?;
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| NativeWindowResolveError::Unavailable(error.to_string()))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| {
                NativeWindowResolveError::Unavailable("X11 screen is unavailable".into())
            })?;
        let client_list = intern_atom(&connection, b"_NET_CLIENT_LIST")
            .map_err(NativeWindowResolveError::Protocol)?;
        let wm_pid =
            intern_atom(&connection, b"_NET_WM_PID").map_err(NativeWindowResolveError::Protocol)?;
        let window_type = intern_atom(&connection, b"_NET_WM_WINDOW_TYPE")
            .map_err(NativeWindowResolveError::Protocol)?;
        let normal_window_type = intern_atom(&connection, b"_NET_WM_WINDOW_TYPE_NORMAL")
            .map_err(NativeWindowResolveError::Protocol)?;
        let mut windows = property_values(&connection, root, client_list)
            .map_err(NativeWindowResolveError::Protocol)?;
        if windows.is_empty() {
            windows = connection
                .query_tree(root)
                .map_err(|error| NativeWindowResolveError::Protocol(error.to_string()))?
                .reply()
                .map_err(|error| NativeWindowResolveError::Protocol(error.to_string()))?
                .children
                .into_iter()
                .collect();
        }
        let mut matches = Vec::new();
        let mut normal_matches = Vec::new();
        for window in windows {
            let Some(candidate_pid) = property_values(&connection, window, wm_pid)
                .map_err(NativeWindowResolveError::Protocol)?
                .into_iter()
                .next()
            else {
                continue;
            };
            if candidate_pid == process_id {
                matches.push(window);
                if property_values(&connection, window, window_type)
                    .map_err(NativeWindowResolveError::Protocol)?
                    .contains(&normal_window_type)
                {
                    normal_matches.push(window);
                }
            }
        }
        // Managed Xvfb commonly has no window manager, so _NET_CLIENT_LIST is
        // absent and the root tree can contain the application's utility
        // windows alongside its actual top-level window. Prefer the public
        // EWMH normal-window classification, while retaining ambiguity refusal
        // when more than one normal window belongs to the process.
        if !normal_matches.is_empty() {
            matches = normal_matches;
        }
        match matches.as_slice() {
            [window] => Ok(NativeWindowHandle::X11(u64::from(*window))),
            [] => Err(NativeWindowResolveError::NotFound(process_id)),
            _ => Err(NativeWindowResolveError::Ambiguous(process_id)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct X11WindowActivator;

impl WindowActivator for X11WindowActivator {
    fn activate(&self, window: &NativeWindowHandle) -> Result<(), WindowActivationError> {
        let NativeWindowHandle::X11(window) = window;
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| WindowActivationError::Unavailable(error.to_string()))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| {
                WindowActivationError::Unavailable("X11 screen is unavailable".into())
            })?;
        let window =
            u32::try_from(*window).map_err(|_| WindowActivationError::UnsupportedHandle)?;
        let active_atom = intern_atom(&connection, b"_NET_ACTIVE_WINDOW")
            .map_err(WindowActivationError::Protocol)?;
        let supported_atom =
            intern_atom(&connection, b"_NET_SUPPORTED").map_err(WindowActivationError::Protocol)?;
        let supported = property_values(&connection, root, supported_atom)
            .map_err(WindowActivationError::Protocol)?;
        if supported.contains(&active_atom) {
            let event = x11rb::protocol::xproto::ClientMessageEvent::new(
                32,
                window,
                active_atom,
                x11rb::protocol::xproto::ClientMessageData::from([1, 0, 0, 0, 0]),
            );
            connection
                .send_event(
                    false,
                    root,
                    x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_REDIRECT
                        | x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )
                .map_err(|error| WindowActivationError::Protocol(error.to_string()))?;
        } else {
            connection
                .configure_window(
                    window,
                    &x11rb::protocol::xproto::ConfigureWindowAux::new()
                        .stack_mode(x11rb::protocol::xproto::StackMode::ABOVE),
                )
                .map_err(|error| WindowActivationError::Protocol(error.to_string()))?;
            connection
                .set_input_focus(
                    x11rb::protocol::xproto::InputFocus::PARENT,
                    window,
                    x11rb::CURRENT_TIME,
                )
                .map_err(|error| WindowActivationError::Protocol(error.to_string()))?;
        }
        connection
            .flush()
            .map_err(|error| WindowActivationError::Protocol(error.to_string()))
    }

    fn is_active(&self, window: &NativeWindowHandle) -> Result<bool, WindowActivationError> {
        let NativeWindowHandle::X11(window) = window;
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| WindowActivationError::Unavailable(error.to_string()))?;
        let root = connection
            .setup()
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| {
                WindowActivationError::Unavailable("X11 screen is unavailable".into())
            })?;
        let active_atom = intern_atom(&connection, b"_NET_ACTIVE_WINDOW")
            .map_err(WindowActivationError::Protocol)?;
        let active = property_values(&connection, root, active_atom)
            .map_err(WindowActivationError::Protocol)?
            .into_iter()
            .next()
            .map(u64::from);
        let input_focus = connection
            .get_input_focus()
            .map_err(|error| WindowActivationError::Protocol(error.to_string()))?
            .reply()
            .map_err(|error| WindowActivationError::Protocol(error.to_string()))?
            .focus;
        Ok(active == Some(*window) || u64::from(input_focus) == *window)
    }
}

fn intern_atom(
    connection: &x11rb::rust_connection::RustConnection,
    name: &[u8],
) -> Result<u32, String> {
    connection
        .intern_atom(false, name)
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn property_values(
    connection: &x11rb::rust_connection::RustConnection,
    window: u32,
    property: u32,
) -> Result<Vec<u32>, String> {
    connection
        .get_property(
            false,
            window,
            property,
            x11rb::protocol::xproto::AtomEnum::ANY,
            0,
            u32::MAX,
        )
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| error.to_string())?
        .value32()
        .map(|values| values.collect())
        .ok_or_else(|| "X11 property is not 32-bit".into())
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AtspiFocusManager;

#[async_trait]
impl FocusManager for AtspiFocusManager {
    async fn request_focus(
        &self,
        backend: &AtspiBackend,
        target: &BackendLocator,
    ) -> Result<(), String> {
        backend
            .focus_node(target)
            .await
            .map_err(|error| error.to_string())
    }

    async fn is_focused(
        &self,
        backend: &AtspiBackend,
        target: &BackendLocator,
    ) -> Result<bool, String> {
        let node = backend
            .refresh_node(target, false)
            .await
            .map_err(|error| error.to_string())?;
        Ok(node.states.contains(&SemanticState::Focused))
    }
}

/// Minimal X11/XTEST backend. It never chooses a target by coordinates: the
/// delivery component verifies the Accessibility focus first, then emits only
/// the explicitly requested key to that already-focused GUI object.
#[derive(Clone, Copy, Debug, Default)]
pub struct X11KeyboardBackend;

impl KeyboardBackend for X11KeyboardBackend {
    fn send_key(&self, key: NativeKey) -> Result<(), KeyboardBackendError> {
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| KeyboardBackendError::Unavailable(error.to_string()))?;
        let setup = connection.setup();
        let min = setup.min_keycode;
        let count = setup.max_keycode.saturating_sub(min).saturating_add(1);
        let mapping = connection
            .get_keyboard_mapping(min, count)
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?
            .reply()
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        let keycode = mapping
            .keysyms
            .chunks(usize::from(mapping.keysyms_per_keycode))
            .enumerate()
            .find_map(|(offset, symbols)| {
                symbols
                    .contains(&key.keysym())
                    .then_some(min.saturating_add(offset as u8))
            })
            .ok_or(KeyboardBackendError::KeyUnavailable { key })?;
        let root = setup
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| KeyboardBackendError::Unavailable("X11 screen is unavailable".into()))?;

        connection
            .xtest_fake_input(KEY_PRESS_EVENT, keycode, 0, root, 0, 0, 0)
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        connection
            .xtest_fake_input(KEY_RELEASE_EVENT, keycode, 0, root, 0, 0, 0)
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        connection
            .flush()
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        Ok(())
    }
}

impl NativeTextInputBackend for X11KeyboardBackend {
    fn replace_text(&self, text: &str) -> Result<(), KeyboardBackendError> {
        if !text
            .chars()
            .all(|character| character.is_ascii() && !character.is_ascii_control())
        {
            return Err(KeyboardBackendError::UnsupportedText);
        }
        let (connection, screen) = x11rb::connect(None)
            .map_err(|error| KeyboardBackendError::Unavailable(error.to_string()))?;
        let setup = connection.setup();
        let min = setup.min_keycode;
        let count = setup.max_keycode.saturating_sub(min).saturating_add(1);
        let mapping = connection
            .get_keyboard_mapping(min, count)
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?
            .reply()
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        let root = setup
            .roots
            .get(screen)
            .map(|screen| screen.root)
            .ok_or_else(|| KeyboardBackendError::Unavailable("X11 screen is unavailable".into()))?;
        let symbols_per_key = usize::from(mapping.keysyms_per_keycode);
        let find = |keysym: u32| {
            mapping
                .keysyms
                .chunks(symbols_per_key)
                .enumerate()
                .find_map(|(offset, symbols)| {
                    symbols
                        .iter()
                        .position(|symbol| *symbol == keysym)
                        .map(|column| (min.saturating_add(offset as u8), column % 2 == 1))
                })
        };
        let control = find(0xffe3)
            .or_else(|| find(0xffe4))
            .map(|(keycode, _)| keycode)
            .ok_or(KeyboardBackendError::UnsupportedText)?;
        let shift = find(0xffe1)
            .or_else(|| find(0xffe2))
            .map(|(keycode, _)| keycode)
            .ok_or(KeyboardBackendError::UnsupportedText)?;
        let select_all = find(u32::from(b'a'))
            .map(|(keycode, _)| keycode)
            .ok_or(KeyboardBackendError::UnsupportedText)?;

        let emit = |event_type, keycode| {
            connection
                .xtest_fake_input(event_type, keycode, 0, root, 0, 0, 0)
                .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))
        };
        emit(KEY_PRESS_EVENT, control)?;
        emit(KEY_PRESS_EVENT, select_all)?;
        emit(KEY_RELEASE_EVENT, select_all)?;
        emit(KEY_RELEASE_EVENT, control)?;

        for character in text.chars() {
            let (keycode, shifted) =
                find(character as u32).ok_or(KeyboardBackendError::UnsupportedText)?;
            if shifted {
                emit(KEY_PRESS_EVENT, shift)?;
            }
            emit(KEY_PRESS_EVENT, keycode)?;
            emit(KEY_RELEASE_EVENT, keycode)?;
            if shifted {
                emit(KEY_RELEASE_EVENT, shift)?;
            }
        }
        connection
            .flush()
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))?;
        connection
            .sync()
            .map_err(|error| KeyboardBackendError::Protocol(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ActuationError {
    #[error("target {0} is stale or no longer resolves to the bound Accessibility object")]
    TargetStale(RuntimeNodeId),
    #[error("target {0} is not focusable")]
    TargetNotFocusable(RuntimeNodeId),
    #[error("owning Accessibility window could not be resolved for target {target}: {reason}")]
    WindowResolveFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("native window could not be resolved for target {target}: {reason}")]
    NativeWindowResolveFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("native window activation failed for target {target}: {reason}")]
    WindowActivationFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("native window activation verification failed for target {0}")]
    WindowActivationVerificationFailed(RuntimeNodeId),
    #[error("focus request failed for target {target}: {reason}")]
    FocusRequestFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("focus verification failed for target {0}")]
    FocusVerificationFailed(RuntimeNodeId),
    #[error("keyboard backend is unavailable: {0}")]
    KeyboardBackendUnavailable(String),
    #[error("text target {target} cannot be resolved safely: {reason}")]
    TextTargetResolveFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("authoritative text readback failed for target {target}: {reason}")]
    TextReadbackFailed {
        target: RuntimeNodeId,
        reason: String,
    },
    #[error("text edit for target {0} was delivered but authoritative readback did not match")]
    TextEditNotVerified(RuntimeNodeId),
    #[error("Accessibility submit action failed: {0}")]
    AccessibilityActionFailed(String),
    #[error("native delivery authority is no longer current: {0}")]
    AuthorityRejected(String),
    #[error("actuation timed out")]
    ActuationTimeout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActuationStrategy {
    AccessibilityAction(SemanticAction),
    NativeKeyboard(NativeKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryStatus {
    Success,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActuationTrace {
    pub target: RuntimeNodeId,
    pub action: SemanticActionKind,
    pub strategy: ActuationStrategy,
    pub delivery: DeliveryStatus,
    pub observation: Option<ObservationResult>,
    pub window: Option<WindowTarget>,
    pub native_window: Option<NativeWindowHandle>,
    pub window_activation_requested: bool,
    pub window_activation_verified: bool,
    pub focus_target: Option<RuntimeNodeId>,
    pub focus_requested: bool,
    pub focus_verified: bool,
    pub key_sent: bool,
    pub result: String,
}

#[derive(Clone)]
pub struct NativeInputDelivery {
    keyboard: Arc<dyn KeyboardBackend>,
    native_text: Arc<dyn NativeTextInputBackend>,
    window_resolver: Arc<dyn NativeWindowResolver>,
    window_activator: Arc<dyn WindowActivator>,
    focus_manager: Arc<dyn FocusManager>,
}

impl fmt::Debug for NativeInputDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeInputDelivery")
            .finish_non_exhaustive()
    }
}

impl Default for NativeInputDelivery {
    fn default() -> Self {
        Self::with_backends(
            Arc::new(X11KeyboardBackend),
            Arc::new(X11WindowResolver),
            Arc::new(X11WindowActivator),
            Arc::new(AtspiFocusManager),
        )
    }
}

impl NativeInputDelivery {
    pub fn new(keyboard: Arc<dyn KeyboardBackend>) -> Self {
        Self::with_backends(
            keyboard,
            Arc::new(X11WindowResolver),
            Arc::new(X11WindowActivator),
            Arc::new(AtspiFocusManager),
        )
    }

    pub fn with_backends(
        keyboard: Arc<dyn KeyboardBackend>,
        window_resolver: Arc<dyn NativeWindowResolver>,
        window_activator: Arc<dyn WindowActivator>,
        focus_manager: Arc<dyn FocusManager>,
    ) -> Self {
        Self::with_all_backends(
            keyboard,
            Arc::new(X11KeyboardBackend),
            window_resolver,
            window_activator,
            focus_manager,
        )
    }

    pub fn with_all_backends(
        keyboard: Arc<dyn KeyboardBackend>,
        native_text: Arc<dyn NativeTextInputBackend>,
        window_resolver: Arc<dyn NativeWindowResolver>,
        window_activator: Arc<dyn WindowActivator>,
        focus_manager: Arc<dyn FocusManager>,
    ) -> Self {
        Self {
            keyboard,
            native_text,
            window_resolver,
            window_activator,
            focus_manager,
        }
    }

    pub fn text_targets(
        &self,
        cache: &SemanticCache,
        binding: &SceneBinding,
    ) -> Result<TextTargetBinding, ActuationError> {
        resolve_text_targets(cache, binding)
    }

    pub async fn read_authoritative_text(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        binding: &SceneBinding,
    ) -> Result<(TextTargetBinding, String), ActuationError> {
        let targets = resolve_text_targets(cache, binding)?;
        let value = backend
            .read_authoritative_text(&targets.readback_target.locator)
            .await
            .map_err(|error| ActuationError::TextReadbackFailed {
                target: binding.runtime_id,
                reason: error.to_string(),
            })?;
        Ok((targets, value))
    }

    /// Execute one explicit semantic Edit. Accessibility mutation is preferred;
    /// every strategy is authoritative only after a fresh Text-interface
    /// readback exactly matches the requested value.
    pub async fn edit(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        binding: &SceneBinding,
        action: SemanticActuation,
        authority: &dyn Fn() -> Result<(), ActuationError>,
    ) -> Result<TextEditResult, ActuationError> {
        let SemanticActuation::Edit(desired) = action;
        let targets = resolve_text_targets(cache, binding)?;
        authority()?;
        let target = targets.source_target.runtime_id;
        let requested_characters = desired.chars().count();
        let mut trace = TextEditTrace {
            targets: targets.clone(),
            requested_characters,
            attempts: Vec::new(),
            window: None,
            native_window: None,
            window_activation_verified: false,
            focus_verified: false,
            verified: false,
        };
        let accessibility = try_accessibility_text_edit(
            backend,
            &targets,
            &desired,
            TEXT_EDIT_VERIFICATION_TIMEOUT,
            TEXT_EDIT_VERIFICATION_POLL,
        )
        .await
        .map_err(|error| ActuationError::TextReadbackFailed {
            target,
            reason: error.to_string(),
        })?;
        let observed = accessibility.observed;
        trace.attempts = accessibility.attempts;
        if accessibility.verified {
            trace.verified = true;
            log_text_edit_trace(&trace);
            return Ok(TextEditResult {
                observed_text: observed,
                trace,
            });
        }

        let focus_node = resolve_bound_target(cache, &targets.focus_target)?;
        for _ in 0..2 {
            let prepared = self
                .prepare_native_input(backend, cache, target, focus_node)
                .await?;
            trace.window = Some(prepared.window.clone());
            trace.native_window = Some(prepared.native_window.clone());
            trace.window_activation_verified = true;
            trace.focus_verified = true;
            // Window and focus preparation can await external providers. The
            // authority gate therefore runs again at the last possible point,
            // immediately before native input is emitted.
            authority()?;
            if let Err(error) = self.native_text.replace_text(&desired) {
                trace.attempts.push(TextEditAttempt {
                    strategy: TextEditStrategy::NativeKeyboardReplace,
                    delivery: TextEditDelivery::Failed,
                    observed_characters: Some(observed.chars().count()),
                    verified: false,
                    detail: Some(error.to_string()),
                });
                log_text_edit_trace(&trace);
                return Err(ActuationError::KeyboardBackendUnavailable(
                    error.to_string(),
                ));
            }
            let verification =
                wait_for_text_readback(backend, &targets.readback_target.locator, &desired)
                    .await
                    .map_err(|error| ActuationError::TextReadbackFailed {
                        target,
                        reason: error.to_string(),
                    })?;
            trace.attempts.push(TextEditAttempt {
                strategy: TextEditStrategy::NativeKeyboardReplace,
                delivery: TextEditDelivery::Accepted,
                observed_characters: Some(verification.observed.chars().count()),
                verified: verification.verified,
                detail: None,
            });
            if verification.verified {
                trace.verified = true;
                log_text_edit_trace(&trace);
                return Ok(TextEditResult {
                    observed_text: verification.observed,
                    trace,
                });
            }
        }
        log_text_edit_trace(&trace);
        Err(ActuationError::TextEditNotVerified(target))
    }

    /// Deliver the user's explicit raw Enter request. This never changes its
    /// trace label to Submit, even when the GUI consequence resembles one.
    pub async fn deliver_raw_enter(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        binding: &SceneBinding,
        authority: &dyn Fn() -> Result<(), ActuationError>,
    ) -> Result<ActuationTrace, ActuationError> {
        self.native_key(
            backend,
            cache,
            binding,
            SemanticActionKind::RawEnter,
            NativeKey::Return,
            authority,
        )
        .await
    }

    async fn resolve_native_window(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        target: RuntimeNodeId,
    ) -> Result<(WindowTarget, NativeWindowHandle), ActuationError> {
        let window_node = resolve_owning_window(cache, target).ok_or_else(|| {
            ActuationError::WindowResolveFailed {
                target,
                reason: "no Window ancestor in the current semantic cache".to_owned(),
            }
        })?;
        let process_id = backend
            .locator_process_id(&window_node.backend_locator)
            .await
            .map_err(|error| ActuationError::WindowResolveFailed {
                target,
                reason: error.to_string(),
            })?;
        let window = WindowTarget {
            accessible_node_id: window_node.runtime_id,
            accessible_locator: window_node.backend_locator.clone(),
            process_id: Some(process_id),
            native_handle: None,
            confidence: None,
        };
        let native_window = self.window_resolver.resolve(&window).map_err(|error| {
            ActuationError::NativeWindowResolveFailed {
                target,
                reason: error.to_string(),
            }
        })?;
        Ok((
            WindowTarget {
                native_handle: Some(native_window.clone()),
                confidence: Some(WindowResolutionConfidence::ProcessUnique),
                ..window
            },
            native_window,
        ))
    }

    async fn activate_and_verify_window(
        &self,
        target: RuntimeNodeId,
        native_window: &NativeWindowHandle,
    ) -> Result<(), ActuationError> {
        self.window_activator
            .activate(native_window)
            .map_err(|error| ActuationError::WindowActivationFailed {
                target,
                reason: error.to_string(),
            })?;
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match self.window_activator.is_active(native_window) {
                Ok(true) => return Ok(()),
                Ok(false) if Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Ok(false) => {
                    return Err(ActuationError::WindowActivationVerificationFailed(target));
                }
                Err(error) => {
                    return Err(ActuationError::WindowActivationFailed {
                        target,
                        reason: error.to_string(),
                    });
                }
            }
        }
    }

    async fn prepare_native_input(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        target: RuntimeNodeId,
        focus_node: &CachedSemanticNode,
    ) -> Result<PreparedNativeInput, ActuationError> {
        if !is_focusable(&focus_node.states) {
            return Err(ActuationError::TargetNotFocusable(target));
        }
        let (window, native_window) = self
            .resolve_native_window(backend, cache, focus_node.runtime_id)
            .await?;
        self.activate_and_verify_window(target, &native_window)
            .await?;
        // Some providers reject grab_focus when the exact target is already
        // focused.  That is not a routing failure: an authoritative focused
        // read before injection is sufficient, and we still perform the
        // mandatory post-request verification below.  If it is not focused,
        // request it through Accessibility and verify again.
        let already_focused = self
            .focus_manager
            .is_focused(backend, &focus_node.backend_locator)
            .await
            .map_err(|_reason| ActuationError::FocusVerificationFailed(target))?;
        if !already_focused {
            self.focus_manager
                .request_focus(backend, &focus_node.backend_locator)
                .await
                .map_err(|reason| ActuationError::FocusRequestFailed { target, reason })?;
        }
        let focused = self
            .focus_manager
            .is_focused(backend, &focus_node.backend_locator)
            .await
            .map_err(|_| ActuationError::FocusVerificationFailed(target))?;
        if !focused {
            return Err(ActuationError::FocusVerificationFailed(target));
        }
        Ok(PreparedNativeInput {
            window,
            native_window,
        })
    }

    async fn native_key(
        &self,
        backend: &AtspiBackend,
        cache: &SemanticCache,
        binding: &SceneBinding,
        action: SemanticActionKind,
        key: NativeKey,
        authority: &dyn Fn() -> Result<(), ActuationError>,
    ) -> Result<ActuationTrace, ActuationError> {
        let target = binding.runtime_id;
        if let Err(error) = resolve_target(cache, binding) {
            warn!(
                target: "gui2tui::product",
                target = %target,
                semantic_action = %action,
                strategy = ?ActuationStrategy::NativeKeyboard(key),
                result = %error,
                "semantic actuation aborted"
            );
            return Err(error);
        }
        let focus_node = match resolve_focus_target(cache, binding) {
            Ok(node) => node,
            Err(error) => {
                warn!(
                    target: "gui2tui::product",
                    target = %target,
                    semantic_action = %action,
                    strategy = ?ActuationStrategy::NativeKeyboard(key),
                    key_sent = false,
                    result = %error,
                    "semantic actuation aborted"
                );
                return Err(error);
            }
        };
        let prepared = match self
            .prepare_native_input(backend, cache, target, focus_node)
            .await
        {
            Ok(prepared) => prepared,
            Err(error) => {
                warn!(
                    target: "gui2tui::product",
                    target = %target,
                    semantic_action = %action,
                    strategy = ?ActuationStrategy::NativeKeyboard(key),
                    key_sent = false,
                    result = %error,
                    "semantic actuation aborted"
                );
                return Err(error);
            }
        };
        let window = prepared.window;
        let native_window = prepared.native_window;
        if let Err(error) = self.send_key_if_authorized(key, authority) {
            warn!(
                target: "gui2tui::product",
                target = %target,
                semantic_action = %action,
                strategy = ?ActuationStrategy::NativeKeyboard(key),
                focus_requested = true,
                focus_verified = true,
                result = %error,
                "semantic actuation failed"
            );
            return Err(error);
        }
        let trace = ActuationTrace {
            target,
            action,
            strategy: ActuationStrategy::NativeKeyboard(key),
            delivery: DeliveryStatus::Success,
            observation: None,
            window: Some(window),
            native_window: Some(native_window.clone()),
            window_activation_requested: true,
            window_activation_verified: true,
            focus_target: Some(focus_node.runtime_id),
            focus_requested: true,
            focus_verified: true,
            key_sent: true,
            result: "success".to_owned(),
        };
        debug!(
            target: "gui2tui::product",
            target = %target,
            semantic_action = %action,
            strategy = ?trace.strategy,
            native_window = %native_window,
            window_activation_requested = true,
            window_activation_verified = true,
            focus_requested = true,
            focus_verified = true,
            key_sent = true,
            key = %key,
            result = %trace.result,
            "semantic actuation"
        );
        Ok(trace)
    }

    fn send_key_if_authorized(
        &self,
        key: NativeKey,
        authority: &dyn Fn() -> Result<(), ActuationError>,
    ) -> Result<(), ActuationError> {
        authority()?;
        self.keyboard
            .send_key(key)
            .map_err(|error| ActuationError::KeyboardBackendUnavailable(error.to_string()))
    }

    pub fn keyboard_available(&self) -> bool {
        std::env::var_os("DISPLAY").is_some()
    }

    pub fn warn_if_unavailable(&self) {
        if !self.keyboard_available() {
            warn!(
                target: "gui2tui::product",
                "native keyboard actuation unavailable because DISPLAY is unset"
            );
        }
    }
}

const TEXT_EDIT_VERIFICATION_TIMEOUT: Duration = Duration::from_millis(750);
const TEXT_EDIT_VERIFICATION_POLL: Duration = Duration::from_millis(25);

struct TextReadbackVerification {
    observed: String,
    verified: bool,
}

struct AccessibilityTextEditResult {
    observed: String,
    attempts: Vec<TextEditAttempt>,
    verified: bool,
}

async fn try_accessibility_text_edit<B: EditableTextBackend + ?Sized>(
    backend: &B,
    targets: &TextTargetBinding,
    desired: &str,
    timeout: Duration,
    poll: Duration,
) -> Result<AccessibilityTextEditResult, BackendError> {
    let mut observed = backend
        .read_authoritative_text(&targets.readback_target.locator)
        .await?;
    let mut attempts = Vec::new();

    match backend
        .deliver_set_contents(&targets.edit_target.locator, desired)
        .await
    {
        Ok(()) => {
            let verification = wait_for_text_readback_with_policy(
                backend,
                &targets.readback_target.locator,
                desired,
                timeout,
                poll,
            )
            .await?;
            observed = verification.observed;
            attempts.push(TextEditAttempt {
                strategy: TextEditStrategy::AtspiSetContents,
                delivery: TextEditDelivery::Accepted,
                observed_characters: Some(observed.chars().count()),
                verified: verification.verified,
                detail: None,
            });
            if verification.verified {
                return Ok(AccessibilityTextEditResult {
                    observed,
                    attempts,
                    verified: true,
                });
            }
        }
        Err(error) => attempts.push(TextEditAttempt {
            strategy: TextEditStrategy::AtspiSetContents,
            delivery: TextEditDelivery::Failed,
            observed_characters: Some(observed.chars().count()),
            verified: false,
            detail: Some(error.to_string()),
        }),
    }

    match backend
        .deliver_delete_and_insert(
            &targets.edit_target.locator,
            observed.chars().count(),
            desired,
        )
        .await
    {
        Ok(()) => {
            let verification = wait_for_text_readback_with_policy(
                backend,
                &targets.readback_target.locator,
                desired,
                timeout,
                poll,
            )
            .await?;
            observed = verification.observed;
            attempts.push(TextEditAttempt {
                strategy: TextEditStrategy::AtspiDeleteAndInsert,
                delivery: TextEditDelivery::Accepted,
                observed_characters: Some(observed.chars().count()),
                verified: verification.verified,
                detail: None,
            });
            if verification.verified {
                return Ok(AccessibilityTextEditResult {
                    observed,
                    attempts,
                    verified: true,
                });
            }
        }
        Err(error) => attempts.push(TextEditAttempt {
            strategy: TextEditStrategy::AtspiDeleteAndInsert,
            delivery: TextEditDelivery::Failed,
            observed_characters: Some(observed.chars().count()),
            verified: false,
            detail: Some(error.to_string()),
        }),
    }

    Ok(AccessibilityTextEditResult {
        observed,
        attempts,
        verified: false,
    })
}

async fn wait_for_text_readback<B: EditableTextBackend + ?Sized>(
    backend: &B,
    target: &BackendLocator,
    desired: &str,
) -> Result<TextReadbackVerification, BackendError> {
    wait_for_text_readback_with_policy(
        backend,
        target,
        desired,
        TEXT_EDIT_VERIFICATION_TIMEOUT,
        TEXT_EDIT_VERIFICATION_POLL,
    )
    .await
}

async fn wait_for_text_readback_with_policy<B: EditableTextBackend + ?Sized>(
    backend: &B,
    target: &BackendLocator,
    desired: &str,
    timeout: Duration,
    poll: Duration,
) -> Result<TextReadbackVerification, BackendError> {
    let deadline = Instant::now() + timeout;
    loop {
        let observed = backend.read_authoritative_text(target).await?;
        if observed == desired {
            return Ok(TextReadbackVerification {
                observed,
                verified: true,
            });
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(TextReadbackVerification {
                observed,
                verified: false,
            });
        }
        tokio::time::sleep(poll.min(remaining)).await;
    }
}

fn log_text_edit_trace(trace: &TextEditTrace) {
    debug!(
        target: "gui2tui::product",
        semantic_action = %SemanticActionKind::Edit,
        source_target = %trace.targets.source_target.runtime_id,
        edit_target = %trace.targets.edit_target.runtime_id,
        focus_target = %trace.targets.focus_target.runtime_id,
        readback_target = %trace.targets.readback_target.runtime_id,
        requested_characters = trace.requested_characters,
        attempts = ?trace.attempts,
        window_activation_verified = trace.window_activation_verified,
        focus_verified = trace.focus_verified,
        verified = trace.verified,
        "verified text edit"
    );
}

fn is_focusable(states: &[SemanticState]) -> bool {
    states.iter().any(|state| {
        matches!(state, SemanticState::Other(value) if value.eq_ignore_ascii_case("focusable"))
    })
}

fn bound(node: &CachedSemanticNode) -> BoundTarget {
    BoundTarget {
        runtime_id: node.runtime_id,
        locator: node.backend_locator.clone(),
    }
}

fn resolve_bound_target<'a>(
    cache: &'a SemanticCache,
    target: &BoundTarget,
) -> Result<&'a CachedSemanticNode, ActuationError> {
    let Some(node) = cache.node(target.runtime_id) else {
        return Err(ActuationError::TargetStale(target.runtime_id));
    };
    if node.backend_locator != target.locator {
        return Err(ActuationError::TargetStale(target.runtime_id));
    }
    Ok(node)
}

fn has_interface(node: &CachedSemanticNode, name: &str) -> bool {
    node.debug
        .interfaces
        .iter()
        .any(|interface| interface.eq_ignore_ascii_case(name))
}

fn is_plain_edit_target(node: &CachedSemanticNode) -> bool {
    node.text_input_kind == Some(TextInputKind::Plain)
        && matches!(node.role, SemanticRole::TextInput | SemanticRole::ComboBox)
        && node.capabilities.contains(&SemanticCapability::EditText)
        && node.states.contains(&SemanticState::Editable)
        && has_interface(node, "EditableText")
}

fn is_plain_readback_target(node: &CachedSemanticNode) -> bool {
    node.text_input_kind != Some(TextInputKind::Password)
        && has_interface(node, "Text")
        && (matches!(node.role, SemanticRole::TextInput | SemanticRole::ComboBox)
            || node.states.contains(&SemanticState::Editable)
            || node.capabilities.contains(&SemanticCapability::EditText))
}

fn descendants_with_depth<'a>(
    cache: &'a SemanticCache,
    source: &'a CachedSemanticNode,
) -> Vec<(&'a CachedSemanticNode, usize)> {
    let mut found = Vec::new();
    let mut pending = VecDeque::from_iter(
        source
            .children
            .iter()
            .copied()
            .map(|child| (child, 1_usize)),
    );
    while let Some((id, depth)) = pending.pop_front() {
        let Some(node) = cache.node(id) else {
            continue;
        };
        found.push((node, depth));
        pending.extend(
            node.children
                .iter()
                .copied()
                .map(|child| (child, depth + 1)),
        );
    }
    found
}

fn unique_best<'a>(
    target: RuntimeNodeId,
    purpose: &str,
    mut candidates: Vec<(&'a CachedSemanticNode, usize, usize)>,
) -> Result<&'a CachedSemanticNode, ActuationError> {
    candidates.sort_by_key(|(_, depth, rank)| (std::cmp::Reverse(*rank), *depth));
    let Some((best, best_depth, best_rank)) = candidates.first().copied() else {
        return Err(ActuationError::TextTargetResolveFailed {
            target,
            reason: format!("no compatible {purpose} target"),
        });
    };
    if candidates
        .iter()
        .skip(1)
        .any(|(_, depth, rank)| *depth == best_depth && *rank == best_rank)
    {
        return Err(ActuationError::TextTargetResolveFailed {
            target,
            reason: format!("ambiguous {purpose} targets at the same semantic depth"),
        });
    }
    Ok(best)
}

fn resolve_text_targets(
    cache: &SemanticCache,
    binding: &SceneBinding,
) -> Result<TextTargetBinding, ActuationError> {
    let source = resolve_target(cache, binding)?;
    let descendants = descendants_with_depth(cache, source);
    let mut edit_candidates = Vec::new();
    if is_plain_edit_target(source) {
        edit_candidates.push((
            source,
            0,
            usize::from(source.role == SemanticRole::TextInput) + 1,
        ));
    }
    edit_candidates.extend(descendants.iter().filter_map(|(node, depth)| {
        is_plain_edit_target(node).then_some((
            *node,
            *depth,
            usize::from(node.role == SemanticRole::TextInput) + 1,
        ))
    }));
    let edit = unique_best(source.runtime_id, "EditableText", edit_candidates)?;

    let readback = if is_plain_readback_target(edit) {
        edit
    } else {
        let candidates = descendants
            .iter()
            .filter_map(|(node, depth)| {
                is_plain_readback_target(node).then_some((
                    *node,
                    *depth,
                    usize::from(node.parent == Some(edit.runtime_id)) + 1,
                ))
            })
            .collect();
        unique_best(source.runtime_id, "Text readback", candidates)?
    };

    let focus = if is_focusable(&edit.states) {
        edit
    } else if is_focusable(&source.states) {
        source
    } else if is_focusable(&readback.states) {
        readback
    } else {
        let candidates = descendants
            .iter()
            .filter_map(|(node, depth)| {
                (is_focusable(&node.states)
                    && (node.capabilities.contains(&SemanticCapability::EditText)
                        || node.states.contains(&SemanticState::Editable)))
                .then_some((
                    *node,
                    *depth,
                    usize::from(node.parent == Some(edit.runtime_id)) + 1,
                ))
            })
            .collect();
        unique_best(source.runtime_id, "focus", candidates)?
    };

    Ok(TextTargetBinding {
        source_target: bound(source),
        edit_target: bound(edit),
        focus_target: bound(focus),
        readback_target: bound(readback),
    })
}

fn resolve_focus_target<'a>(
    cache: &'a SemanticCache,
    binding: &SceneBinding,
) -> Result<&'a CachedSemanticNode, ActuationError> {
    let source = resolve_target(cache, binding)?;
    if is_focusable(&source.states) {
        return Ok(source);
    }
    let prefers_editable = source.capabilities.contains(&SemanticCapability::EditText);
    let mut pending = source.children.clone();
    while let Some(candidate_id) = pending.pop() {
        let Some(candidate) = cache.node(candidate_id) else {
            continue;
        };
        let matches_capability = !prefers_editable
            || candidate
                .capabilities
                .contains(&SemanticCapability::EditText);
        if matches_capability && is_focusable(&candidate.states) {
            return Ok(candidate);
        }
        pending.extend(candidate.children.iter().copied());
    }
    Err(ActuationError::TargetNotFocusable(binding.runtime_id))
}

fn resolve_owning_window(
    cache: &SemanticCache,
    target: RuntimeNodeId,
) -> Option<&CachedSemanticNode> {
    let mut current = Some(target);
    for _ in 0..64 {
        let id = current?;
        let node = cache.node(id)?;
        if node.role == crate::semantic::SemanticRole::Window {
            return Some(node);
        }
        current = node.parent;
    }
    None
}

fn resolve_target<'a>(
    cache: &'a SemanticCache,
    binding: &SceneBinding,
) -> Result<&'a CachedSemanticNode, ActuationError> {
    let Some(node) = cache.node(binding.runtime_id) else {
        return Err(ActuationError::TargetStale(binding.runtime_id));
    };
    if node.backend_locator != binding.backend_locator {
        return Err(ActuationError::TargetStale(binding.runtime_id));
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use crate::semantic::{DebugInfo, SemanticNode, SemanticRole, TextInputKind};

    use super::*;

    #[derive(Default)]
    struct RecordingKeyboard {
        keys: Mutex<Vec<NativeKey>>,
    }

    impl KeyboardBackend for RecordingKeyboard {
        fn send_key(&self, key: NativeKey) -> Result<(), KeyboardBackendError> {
            self.keys.lock().unwrap().push(key);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingTextInput {
        replacements: Mutex<Vec<String>>,
    }

    impl NativeTextInputBackend for RecordingTextInput {
        fn replace_text(&self, text: &str) -> Result<(), KeyboardBackendError> {
            self.replacements.lock().unwrap().push(text.to_owned());
            Ok(())
        }
    }

    struct FakeEditableTextBackend {
        reads: Mutex<VecDeque<String>>,
        last: Mutex<String>,
        set_accepted: bool,
        delete_insert_accepted: bool,
    }

    impl FakeEditableTextBackend {
        fn with_reads(values: &[&str]) -> Self {
            Self {
                reads: Mutex::new(values.iter().map(|value| (*value).to_owned()).collect()),
                last: Mutex::new(values.last().copied().unwrap_or_default().to_owned()),
                set_accepted: true,
                delete_insert_accepted: true,
            }
        }
    }

    #[async_trait]
    impl EditableTextBackend for FakeEditableTextBackend {
        async fn deliver_set_contents(
            &self,
            _target: &BackendLocator,
            _text: &str,
        ) -> Result<(), BackendError> {
            self.set_accepted
                .then_some(())
                .ok_or_else(|| BackendError::TextUpdateRejected("test".to_owned()))
        }

        async fn deliver_delete_and_insert(
            &self,
            _target: &BackendLocator,
            _existing_characters: usize,
            _text: &str,
        ) -> Result<(), BackendError> {
            self.delete_insert_accepted
                .then_some(())
                .ok_or_else(|| BackendError::TextUpdateRejected("test".to_owned()))
        }

        async fn read_authoritative_text(
            &self,
            _target: &BackendLocator,
        ) -> Result<String, BackendError> {
            if let Some(value) = self.reads.lock().unwrap().pop_front() {
                *self.last.lock().unwrap() = value.clone();
                Ok(value)
            } else {
                Ok(self.last.lock().unwrap().clone())
            }
        }
    }

    #[derive(Default)]
    struct RecordingActivator {
        activated: Mutex<Vec<NativeWindowHandle>>,
        active: bool,
    }

    impl WindowActivator for RecordingActivator {
        fn activate(&self, window: &NativeWindowHandle) -> Result<(), WindowActivationError> {
            self.activated.lock().unwrap().push(window.clone());
            Ok(())
        }

        fn is_active(&self, _window: &NativeWindowHandle) -> Result<bool, WindowActivationError> {
            Ok(self.active)
        }
    }

    #[test]
    fn raw_enter_uses_return_without_exposing_backend_details() {
        assert_eq!(NativeKey::Return.keysym(), 0xff0d);
        assert_eq!(SemanticActionKind::RawEnter.to_string(), "RawEnter");
    }

    #[test]
    fn recording_keyboard_is_a_testable_backend_boundary() {
        let keyboard = Arc::new(RecordingKeyboard::default());
        let _engine = NativeInputDelivery::new(keyboard.clone());
        keyboard.send_key(NativeKey::Return).unwrap();
        assert_eq!(
            keyboard.keys.lock().unwrap().as_slice(),
            &[NativeKey::Return]
        );
    }

    #[test]
    fn failed_authority_gate_blocks_native_key_delivery() {
        let keyboard = Arc::new(RecordingKeyboard::default());
        let engine = NativeInputDelivery::new(keyboard.clone());
        let result = engine.send_key_if_authorized(NativeKey::Return, &|| {
            Err(ActuationError::AuthorityRejected(
                "application generation changed".to_owned(),
            ))
        });
        assert!(matches!(result, Err(ActuationError::AuthorityRejected(_))));
        assert!(keyboard.keys.lock().unwrap().is_empty());
    }

    #[test]
    fn scope_authority_rejection_blocks_native_key_delivery() {
        let keyboard = Arc::new(RecordingKeyboard::default());
        let engine = NativeInputDelivery::new(keyboard.clone());
        let result = engine.send_key_if_authorized(NativeKey::Return, &|| {
            Err(ActuationError::AuthorityRejected(
                "target is outside the active interaction scope".to_owned(),
            ))
        });
        assert!(matches!(result, Err(ActuationError::AuthorityRejected(_))));
        assert!(keyboard.keys.lock().unwrap().is_empty());
    }

    #[test]
    fn retired_application_generation_blocks_native_key_delivery() {
        let keyboard = Arc::new(RecordingKeyboard::default());
        let engine = NativeInputDelivery::new(keyboard.clone());
        let mut runtime = crate::runtime::RuntimeSession::default();
        let expected = runtime.open_application(BackendLocator::new(":1.2", "/app-a"));
        runtime.open_application(BackendLocator::new(":1.3", "/app-b"));
        let result = engine.send_key_if_authorized(NativeKey::Return, &|| {
            (runtime.generation() == Some(expected))
                .then_some(())
                .ok_or_else(|| {
                    ActuationError::AuthorityRejected("application generation changed".to_owned())
                })
        });
        assert!(matches!(result, Err(ActuationError::AuthorityRejected(_))));
        assert!(keyboard.keys.lock().unwrap().is_empty());
    }

    #[test]
    fn native_fallback_requires_explicit_focusable_state() {
        assert!(is_focusable(&[SemanticState::Other(
            "focusable".to_owned()
        )]));
        assert!(is_focusable(&[SemanticState::Other(
            "FOCUSABLE".to_owned()
        )]));
        assert!(!is_focusable(&[SemanticState::Other(
            "editable".to_owned()
        )]));
    }

    #[tokio::test]
    async fn provider_acceptance_is_not_text_edit_verification() {
        let backend = FakeEditableTextBackend::with_reads(&["old value"]);
        EditableTextBackend::deliver_set_contents(
            &backend,
            &BackendLocator::new(":1.2", "/edit"),
            "requested",
        )
        .await
        .unwrap();
        let verification = wait_for_text_readback_with_policy(
            &backend,
            &BackendLocator::new(":1.2", "/readback"),
            "requested",
            Duration::ZERO,
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert!(!verification.verified);
        assert_eq!(verification.observed, "old value");
    }

    #[tokio::test]
    async fn delayed_authoritative_readback_can_verify_the_edit() {
        let backend = FakeEditableTextBackend::with_reads(&["old value", "requested"]);
        let verification = wait_for_text_readback_with_policy(
            &backend,
            &BackendLocator::new(":1.2", "/readback"),
            "requested",
            Duration::from_millis(20),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        assert!(verification.verified);
        assert_eq!(verification.observed, "requested");
    }

    #[tokio::test]
    async fn accessibility_delete_insert_is_verified_after_set_contents_mismatch() {
        let backend = FakeEditableTextBackend::with_reads(&["old value", "old value", "requested"]);
        let target = BoundTarget {
            runtime_id: RuntimeNodeId::new(7),
            locator: BackendLocator::new(":1.2", "/edit"),
        };
        let targets = TextTargetBinding {
            source_target: target.clone(),
            edit_target: target.clone(),
            focus_target: target.clone(),
            readback_target: target,
        };
        let result = try_accessibility_text_edit(
            &backend,
            &targets,
            "requested",
            Duration::ZERO,
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert!(result.verified);
        assert_eq!(result.observed, "requested");
        assert_eq!(result.attempts.len(), 2);
        assert!(!result.attempts[0].verified);
        assert_eq!(
            result.attempts[1].strategy,
            TextEditStrategy::AtspiDeleteAndInsert
        );
        assert!(result.attempts[1].verified);
    }

    #[tokio::test]
    async fn rejected_set_contents_can_fall_back_to_verified_delete_insert() {
        let mut backend = FakeEditableTextBackend::with_reads(&["old value", "requested"]);
        backend.set_accepted = false;
        let target = BoundTarget {
            runtime_id: RuntimeNodeId::new(8),
            locator: BackendLocator::new(":1.2", "/edit"),
        };
        let targets = TextTargetBinding {
            source_target: target.clone(),
            edit_target: target.clone(),
            focus_target: target.clone(),
            readback_target: target,
        };
        let result = try_accessibility_text_edit(
            &backend,
            &targets,
            "requested",
            Duration::ZERO,
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert!(result.verified);
        assert_eq!(result.attempts[0].delivery, TextEditDelivery::Failed);
        assert_eq!(
            result.attempts[1].strategy,
            TextEditStrategy::AtspiDeleteAndInsert
        );
        assert!(result.attempts[1].verified);
    }

    #[tokio::test]
    async fn accessibility_mismatches_require_native_fallback() {
        let backend = FakeEditableTextBackend::with_reads(&["old value", "old value", "old value"]);
        let target = BoundTarget {
            runtime_id: RuntimeNodeId::new(9),
            locator: BackendLocator::new(":1.2", "/edit"),
        };
        let targets = TextTargetBinding {
            source_target: target.clone(),
            edit_target: target.clone(),
            focus_target: target.clone(),
            readback_target: target,
        };
        let result = try_accessibility_text_edit(
            &backend,
            &targets,
            "requested",
            Duration::ZERO,
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert!(!result.verified);
        assert_eq!(result.attempts.len(), 2);
        assert!(result.attempts.iter().all(|attempt| !attempt.verified));
    }

    #[tokio::test]
    async fn native_delivery_still_requires_authoritative_readback() {
        let native = RecordingTextInput::default();
        native.replace_text("requested").unwrap();
        let backend = FakeEditableTextBackend::with_reads(&["old value"]);
        let verification = wait_for_text_readback_with_policy(
            &backend,
            &BackendLocator::new(":1.2", "/readback"),
            "requested",
            Duration::ZERO,
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert_eq!(native.replacements.lock().unwrap().len(), 1);
        assert!(!verification.verified);
    }

    #[test]
    fn native_text_backend_boundary_is_separate_from_submit_keys() {
        let text = RecordingTextInput::default();
        text.replace_text("https://example.test/").unwrap();
        assert_eq!(
            text.replacements.lock().unwrap().as_slice(),
            &["https://example.test/"]
        );
    }

    #[test]
    fn x11_native_text_rejects_unicode_before_emitting_input() {
        assert!(matches!(
            X11KeyboardBackend.replace_text("搜索"),
            Err(KeyboardBackendError::UnsupportedText)
        ));
    }

    #[tokio::test]
    async fn activation_verification_is_a_separate_gate() {
        let keyboard = Arc::new(RecordingKeyboard::default());
        let activator = Arc::new(RecordingActivator {
            active: false,
            ..Default::default()
        });
        let engine = NativeInputDelivery::with_backends(
            keyboard.clone(),
            Arc::new(X11WindowResolver),
            activator.clone(),
            Arc::new(AtspiFocusManager),
        );
        let result = engine
            .activate_and_verify_window(RuntimeNodeId::new(42), &NativeWindowHandle::X11(0x4600007))
            .await;
        assert!(matches!(
            result,
            Err(ActuationError::WindowActivationVerificationFailed(target))
                if target == RuntimeNodeId::new(42)
        ));
        assert_eq!(activator.activated.lock().unwrap().len(), 1);
        assert!(keyboard.keys.lock().unwrap().is_empty());
    }

    #[test]
    fn owning_window_and_focus_target_are_resolved_from_semantic_ancestry() {
        let mut root = test_node("/app", SemanticRole::Application);
        let mut window = test_node("/window", SemanticRole::Window);
        let mut control = test_node("/control", SemanticRole::ComboBox);
        let mut edit = test_node("/edit", SemanticRole::TextInput);
        edit.states
            .push(SemanticState::Other("focusable".to_owned()));
        edit.capabilities.push(SemanticCapability::EditText);
        control.capabilities.push(SemanticCapability::EditText);
        control.children.push(edit);
        window.children.push(control);
        root.children.push(window);
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let control_id = cache
            .runtime_id(&BackendLocator::new(":1.2", "/control"))
            .unwrap();
        let binding = SceneBinding {
            runtime_id: control_id,
            backend_locator: BackendLocator::new(":1.2", "/control"),
            semantic_role: SemanticRole::ComboBox,
            actions: Vec::new(),
            capability: crate::tui::action::InteractionCapability::EditText,
            default_intent: crate::tui::action::UiIntent::BeginEdit,
        };
        let focus_target = resolve_focus_target(&cache, &binding).unwrap();
        assert_eq!(focus_target.backend_locator.object_path(), "/edit");
        assert_eq!(
            resolve_owning_window(&cache, focus_target.runtime_id)
                .unwrap()
                .backend_locator
                .object_path(),
            "/window"
        );
    }

    #[test]
    fn text_targets_can_bind_source_edit_focus_and_readback_independently() {
        let mut root = test_node("/app", SemanticRole::Application);
        let mut window = test_node("/window", SemanticRole::Window);
        let mut source = test_node("/control", SemanticRole::ComboBox);
        source.text_input_kind = Some(TextInputKind::Plain);
        source
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        let mut edit = test_node("/edit", SemanticRole::TextInput);
        edit.text_input_kind = Some(TextInputKind::Plain);
        edit.states.push(SemanticState::Editable);
        edit.capabilities.push(SemanticCapability::EditText);
        edit.debug.interfaces.push("EditableText".to_owned());
        let mut readback = test_node("/readback", SemanticRole::TextInput);
        readback.text_input_kind = Some(TextInputKind::Plain);
        readback.states.push(SemanticState::Editable);
        readback.debug.interfaces.push("Text".to_owned());
        edit.children.push(readback);
        source.children.push(edit);
        window.children.push(source);
        root.children.push(window);
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let source_id = cache
            .runtime_id(&BackendLocator::new(":1.2", "/control"))
            .unwrap();
        let binding = SceneBinding {
            runtime_id: source_id,
            backend_locator: BackendLocator::new(":1.2", "/control"),
            semantic_role: SemanticRole::ComboBox,
            actions: Vec::new(),
            capability: crate::tui::action::InteractionCapability::EditText,
            default_intent: crate::tui::action::UiIntent::BeginEdit,
        };
        let targets = resolve_text_targets(&cache, &binding).unwrap();
        assert_eq!(targets.source_target.locator.object_path(), "/control");
        assert_eq!(targets.edit_target.locator.object_path(), "/edit");
        assert_eq!(targets.focus_target.locator.object_path(), "/control");
        assert_eq!(targets.readback_target.locator.object_path(), "/readback");
    }

    #[test]
    fn stale_binding_is_rejected_before_window_resolution() {
        let cache =
            SemanticCache::from_snapshot(test_node("/app", SemanticRole::Application)).unwrap();
        let binding = SceneBinding {
            runtime_id: RuntimeNodeId::new(999),
            backend_locator: BackendLocator::new(":1.2", "/gone"),
            semantic_role: SemanticRole::Button,
            actions: Vec::new(),
            capability: crate::tui::action::InteractionCapability::Activate,
            default_intent: crate::tui::action::UiIntent::Activate,
        };
        assert!(matches!(
            resolve_target(&cache, &binding),
            Err(ActuationError::TargetStale(id)) if id == RuntimeNodeId::new(999)
        ));
    }

    fn test_node(path: &str, role: SemanticRole) -> SemanticNode {
        let text_input = role == SemanticRole::TextInput;
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", path),
            index_in_parent: None,
            role,
            name: Some(path.to_owned()),
            description: None,
            value: None,
            text_input_kind: text_input.then_some(TextInputKind::Plain),
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }
}
