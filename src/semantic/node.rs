use std::fmt;

use atspi::{Role, State};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use thiserror::Error;

const NODE_ID_PREFIX: &str = "atspi1_";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BackendLocator {
    bus_name: String,
    object_path: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BackendLocatorError {
    #[error("node ID must start with '{NODE_ID_PREFIX}'")]
    MissingPrefix,
    #[error("node ID payload is not valid URL-safe Base64")]
    InvalidBase64,
    #[error("node ID payload is not valid UTF-8")]
    InvalidUtf8,
    #[error("node ID payload does not contain a bus name and object path")]
    MissingSeparator,
    #[error("node ID contains an invalid AT-SPI unique bus name: {0}")]
    InvalidBusName(String),
    #[error("node ID contains an invalid D-Bus object path: {0}")]
    InvalidObjectPath(String),
}

impl BackendLocator {
    pub fn new(bus_name: impl Into<String>, object_path: impl Into<String>) -> Self {
        Self {
            bus_name: bus_name.into(),
            object_path: object_path.into(),
        }
    }

    pub fn bus_name(&self) -> &str {
        &self.bus_name
    }

    pub fn object_path(&self) -> &str {
        &self.object_path
    }

    pub fn encode(&self) -> String {
        let raw = format!("{}\0{}", self.bus_name, self.object_path);
        format!("{NODE_ID_PREFIX}{}", URL_SAFE_NO_PAD.encode(raw))
    }

    pub fn decode(encoded: &str) -> Result<Self, BackendLocatorError> {
        let payload = encoded
            .strip_prefix(NODE_ID_PREFIX)
            .ok_or(BackendLocatorError::MissingPrefix)?;
        let raw = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| BackendLocatorError::InvalidBase64)?;
        let raw = String::from_utf8(raw).map_err(|_| BackendLocatorError::InvalidUtf8)?;
        let (bus_name, object_path) = raw
            .split_once('\0')
            .ok_or(BackendLocatorError::MissingSeparator)?;

        zbus::names::UniqueName::try_from(bus_name)
            .map_err(|_| BackendLocatorError::InvalidBusName(bus_name.to_owned()))?;
        zbus::zvariant::ObjectPath::try_from(object_path)
            .map_err(|_| BackendLocatorError::InvalidObjectPath(object_path.to_owned()))?;

        Ok(Self::new(bus_name, object_path))
    }
}

impl fmt::Display for BackendLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encode())
    }
}

/// Compact identity assigned while building one semantic snapshot.
///
/// Runtime IDs are suitable for focus, renderer state, and hit testing. They are
/// deliberately regenerated on refresh and cannot be used to locate AT-SPI objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuntimeNodeId(u64);

impl RuntimeNodeId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RuntimeNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticRole {
    Application,
    Window,
    Dialog,
    Container,
    Label,
    Button,
    ToggleButton,
    CheckBox,
    RadioButton,
    Text,
    TextInput,
    Document,
    Heading,
    Paragraph,
    Link,
    Image,
    Quote,
    Landmark,
    Form,
    Comment,
    Audio,
    Video,
    ComboBox,
    MenuBar,
    Menu,
    MenuItem,
    List,
    ListItem,
    Tab,
    TabList,
    Tree,
    TreeItem,
    Table,
    Row,
    Cell,
    Slider,
    ProgressBar,
    StatusBar,
    Unknown(String),
}

/// Semantic kind for editable text controls.
///
/// This is intentionally separate from AT-SPI's `sensitive` state, which means
/// that a control can respond to user input and says nothing about secrecy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputKind {
    Plain,
    Password,
}

impl From<Role> for SemanticRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Application => Self::Application,
            Role::Frame | Role::Window | Role::InternalFrame => Self::Window,
            Role::Dialog | Role::Alert | Role::FileChooser | Role::ColorChooser => Self::Dialog,
            Role::Panel
            | Role::Filler
            | Role::RootPane
            | Role::ScrollPane
            | Role::Viewport
            | Role::Grouping
            | Role::Section
            | Role::Article => Self::Container,
            Role::Form => Self::Form,
            Role::Label | Role::Static | Role::Caption => Self::Label,
            Role::Button | Role::PushButtonMenu => Self::Button,
            Role::ToggleButton => Self::ToggleButton,
            Role::CheckBox | Role::CheckMenuItem => Self::CheckBox,
            Role::RadioButton | Role::RadioMenuItem => Self::RadioButton,
            Role::Text | Role::Terminal => Self::Text,
            Role::Paragraph => Self::Paragraph,
            Role::Entry | Role::PasswordText | Role::DateEditor | Role::Editbar => Self::TextInput,
            Role::DocumentFrame
            | Role::DocumentSpreadsheet
            | Role::DocumentPresentation
            | Role::DocumentText
            | Role::DocumentWeb
            | Role::DocumentEmail
            | Role::HTMLContainer => Self::Document,
            Role::Heading | Role::Header => Self::Heading,
            Role::Link => Self::Link,
            Role::Image | Role::ImageMap | Role::Icon => Self::Image,
            Role::BlockQuote => Self::Quote,
            Role::Landmark => Self::Landmark,
            Role::Comment | Role::Footnote => Self::Comment,
            Role::Audio => Self::Audio,
            Role::Video => Self::Video,
            Role::ComboBox => Self::ComboBox,
            Role::MenuBar => Self::MenuBar,
            Role::Menu | Role::PopupMenu => Self::Menu,
            Role::MenuItem | Role::TearoffMenuItem => Self::MenuItem,
            Role::List | Role::ListBox => Self::List,
            Role::ListItem => Self::ListItem,
            Role::PageTab => Self::Tab,
            Role::PageTabList => Self::TabList,
            Role::Tree | Role::TreeTable => Self::Tree,
            Role::TreeItem => Self::TreeItem,
            Role::Table => Self::Table,
            Role::TableRow => Self::Row,
            Role::TableCell | Role::ColumnHeader | Role::RowHeader => Self::Cell,
            Role::Slider | Role::ScrollBar | Role::SpinButton | Role::Dial => Self::Slider,
            Role::ProgressBar | Role::LevelBar => Self::ProgressBar,
            Role::StatusBar => Self::StatusBar,
            other => Self::Unknown(other.name().to_owned()),
        }
    }
}

impl SemanticRole {
    pub fn from_atspi(role: Role, editable: bool) -> Self {
        if role == Role::Text && editable {
            Self::TextInput
        } else {
            Self::from(role)
        }
    }
}

impl fmt::Display for SemanticRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Application => f.write_str("Application"),
            Self::Window => f.write_str("Window"),
            Self::Dialog => f.write_str("Dialog"),
            Self::Container => f.write_str("Container"),
            Self::Label => f.write_str("Label"),
            Self::Button => f.write_str("Button"),
            Self::ToggleButton => f.write_str("ToggleButton"),
            Self::CheckBox => f.write_str("CheckBox"),
            Self::RadioButton => f.write_str("RadioButton"),
            Self::Text => f.write_str("Text"),
            Self::TextInput => f.write_str("TextInput"),
            Self::Document => f.write_str("Document"),
            Self::Heading => f.write_str("Heading"),
            Self::Paragraph => f.write_str("Paragraph"),
            Self::Link => f.write_str("Link"),
            Self::Image => f.write_str("Image"),
            Self::Quote => f.write_str("Quote"),
            Self::Landmark => f.write_str("Landmark"),
            Self::Form => f.write_str("Form"),
            Self::Comment => f.write_str("Comment"),
            Self::Audio => f.write_str("Audio"),
            Self::Video => f.write_str("Video"),
            Self::ComboBox => f.write_str("ComboBox"),
            Self::MenuBar => f.write_str("MenuBar"),
            Self::Menu => f.write_str("Menu"),
            Self::MenuItem => f.write_str("MenuItem"),
            Self::List => f.write_str("List"),
            Self::ListItem => f.write_str("ListItem"),
            Self::Tab => f.write_str("Tab"),
            Self::TabList => f.write_str("TabList"),
            Self::Tree => f.write_str("Tree"),
            Self::TreeItem => f.write_str("TreeItem"),
            Self::Table => f.write_str("Table"),
            Self::Row => f.write_str("Row"),
            Self::Cell => f.write_str("Cell"),
            Self::Slider => f.write_str("Slider"),
            Self::ProgressBar => f.write_str("ProgressBar"),
            Self::StatusBar => f.write_str("StatusBar"),
            Self::Unknown(original) => write!(f, "Unknown({original})"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticState {
    Checked,
    Selected,
    Expanded,
    Collapsed,
    Enabled,
    Focused,
    Editable,
    Pressed,
    Busy,
    Indeterminate,
    ReadOnly,
    Other(String),
}

impl From<State> for SemanticState {
    fn from(state: State) -> Self {
        match state {
            State::Checked => Self::Checked,
            State::Selected => Self::Selected,
            State::Expanded => Self::Expanded,
            State::Collapsed => Self::Collapsed,
            State::Enabled => Self::Enabled,
            State::Focused => Self::Focused,
            State::Editable => Self::Editable,
            State::Pressed => Self::Pressed,
            State::Busy => Self::Busy,
            State::Indeterminate => Self::Indeterminate,
            State::ReadOnly => Self::ReadOnly,
            other => Self::Other(other.to_string()),
        }
    }
}

impl fmt::Display for SemanticState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Checked => "checked",
            Self::Selected => "selected",
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
            Self::Enabled => "enabled",
            Self::Focused => "focused",
            Self::Editable => "editable",
            Self::Pressed => "pressed",
            Self::Busy => "busy",
            Self::Indeterminate => "indeterminate",
            Self::ReadOnly => "read-only",
            Self::Other(value) => value,
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticAction {
    pub index: i32,
    pub name: String,
    pub description: Option<String>,
    pub keybinding: Option<String>,
}

/// Backend-independent capabilities advertised by a semantic node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticCapability {
    /// The container can select one of its direct children.
    SelectChildren,
    /// A plain text input supports atomic replacement through AT-SPI.
    EditText,
    /// A finite bounded AT-SPI Value with a positive advertised increment.
    Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub atspi_role: String,
    pub bus_name: String,
    pub object_path: String,
    pub interfaces: Vec<String>,
    pub geometry: Option<Geometry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeTruncation {
    MaxDepth {
        limit: usize,
    },
    MaxNodes {
        limit: usize,
    },
    OperationTimeout {
        operation: &'static str,
        node_id: String,
    },
}

impl fmt::Display for TreeTruncation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxDepth { limit } => write!(f, "tree truncated: max depth={limit}"),
            Self::MaxNodes { limit } => write!(f, "tree truncated: max nodes={limit}"),
            Self::OperationTimeout { operation, node_id } => {
                write!(f, "tree incomplete: {operation} timed out on {node_id}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticNode {
    pub runtime_id: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    /// Original child position in the direct parent, if this is not a root.
    pub index_in_parent: Option<usize>,
    pub role: SemanticRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    /// Present only when `role` is `SemanticRole::TextInput`.
    pub text_input_kind: Option<TextInputKind>,
    pub states: Vec<SemanticState>,
    pub actions: Vec<SemanticAction>,
    pub capabilities: Vec<SemanticCapability>,
    pub children: Vec<SemanticNode>,
    pub truncations: Vec<TreeTruncation>,
    pub debug: DebugInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_locator_round_trip() {
        let id = BackendLocator::new(":1.42", "/org/a11y/atspi/accessible/17");
        let encoded = id.encode();
        assert!(!encoded.contains('/'));
        assert_eq!(BackendLocator::decode(&encoded).unwrap(), id);
    }

    #[test]
    fn invalid_node_ids_are_rejected() {
        assert_eq!(
            BackendLocator::decode("abc"),
            Err(BackendLocatorError::MissingPrefix)
        );
        assert!(matches!(
            BackendLocator::decode("atspi1_!!!"),
            Err(BackendLocatorError::InvalidBase64)
        ));
    }

    #[test]
    fn runtime_node_id_is_compact_and_copyable() {
        let id = RuntimeNodeId::new(42);
        let copied = id;
        assert_eq!(copied.get(), 42);
        assert_eq!(std::mem::size_of::<RuntimeNodeId>(), 8);
    }

    #[test]
    fn maps_representative_roles() {
        assert_eq!(
            SemanticRole::from(Role::Application),
            SemanticRole::Application
        );
        assert_eq!(SemanticRole::from(Role::Entry), SemanticRole::TextInput);
        assert_eq!(SemanticRole::from(Role::ComboBox), SemanticRole::ComboBox);
        assert_eq!(SemanticRole::from(Role::PageTab), SemanticRole::Tab);
        assert_eq!(
            SemanticRole::from_atspi(Role::Text, true),
            SemanticRole::TextInput
        );
        assert_eq!(
            SemanticRole::from_atspi(Role::Text, false),
            SemanticRole::Text
        );
        assert_eq!(SemanticRole::from(Role::Image), SemanticRole::Image);
        assert_eq!(
            SemanticRole::from(Role::DocumentWeb),
            SemanticRole::Document
        );
        assert_eq!(SemanticRole::from(Role::Heading), SemanticRole::Heading);
    }

    #[test]
    fn maps_representative_states() {
        assert_eq!(SemanticState::from(State::Checked), SemanticState::Checked);
        assert_eq!(
            SemanticState::from(State::Focusable),
            SemanticState::Other("focusable".to_owned())
        );
        assert_eq!(
            SemanticState::from(State::Sensitive),
            SemanticState::Other("sensitive".to_owned())
        );
    }
}
