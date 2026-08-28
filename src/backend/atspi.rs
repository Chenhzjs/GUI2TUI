use std::{collections::HashSet, future::Future, pin::Pin, time::Duration};

use atspi::{
    AccessibilityConnection, CoordType, Interface, ObjectRef, ObjectRefOwned, Role,
    proxy::{
        accessible::ObjectRefExt, component::ComponentProxy, proxy_ext::ProxyExt, text::TextProxy,
        value::ValueProxy,
    },
};
use thiserror::Error;
use tracing::warn;
use zbus::{names::UniqueName, zvariant::ObjectPath};

use crate::semantic::{
    BackendLocator, DebugInfo, Geometry, RuntimeIdAllocator, SemanticAction, SemanticNode,
    SemanticRole, SemanticState, TreeTruncation,
};

#[derive(Clone, Debug)]
pub struct ApplicationRef {
    pub index: usize,
    pub name: String,
    pub backend_locator: BackendLocator,
    object: ObjectRefOwned,
}

#[derive(Clone, Copy, Debug)]
pub struct InspectOptions {
    pub verbose: bool,
    pub max_depth: usize,
    pub max_nodes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct SessionEnvironment {
    pub xdg_session_type: Option<String>,
    pub dbus_session_bus_address: Option<String>,
    pub display: Option<String>,
    pub wayland_display: Option<String>,
}

impl SessionEnvironment {
    pub fn detect() -> Self {
        Self {
            xdg_session_type: std::env::var("XDG_SESSION_TYPE").ok(),
            dbus_session_bus_address: std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
            display: std::env::var("DISPLAY").ok(),
            wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "XDG_SESSION_TYPE={}, DBUS_SESSION_BUS_ADDRESS={}, DISPLAY={}, WAYLAND_DISPLAY={}",
            shown(&self.xdg_session_type),
            shown(&self.dbus_session_bus_address),
            shown(&self.display),
            shown(&self.wayland_display)
        )
    }
}

fn shown(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("<unset>")
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("No accessible AT-SPI desktop session found. {environment}. Cause: {source}")]
    NoDesktopSession {
        environment: String,
        source: Box<zbus::Error>,
    },
    #[error(
        "No accessible AT-SPI desktop session found. The session D-Bus is reachable, but the AT-SPI bus is unavailable. {environment}. Cause: {source}"
    )]
    AtspiUnavailable {
        environment: String,
        source: atspi::AtspiError,
    },
    #[error("failed to enumerate applications from the AT-SPI registry: {0}")]
    EnumerateApplications(atspi::AtspiError),
    #[error("no accessible applications are currently exposed by AT-SPI")]
    NoApplications,
    #[error("application index {0} does not exist; run with --list to see current indices")]
    ApplicationIndexNotFound(usize),
    #[error("application '{0}' was not found; run with --list to see accessible applications")]
    ApplicationNotFound(String),
    #[error("application selector '{selector}' is ambiguous; matches: {matches}")]
    AmbiguousApplication { selector: String, matches: String },
    #[error("invalid NODE_ID: {0}")]
    InvalidNodeId(#[from] crate::semantic::BackendLocatorError),
    #[error("AT-SPI object {0} is unavailable or has become stale: {1}")]
    ObjectUnavailable(String, atspi::AtspiError),
    #[error("AT-SPI object {0} does not expose the Action interface")]
    ActionUnsupported(String),
    #[error("AT-SPI object {0} exposes the Action interface but has no available actions")]
    NoActions(String),
    #[error("action index {index} does not exist on {node_id}; available actions: {count}")]
    ActionNotFound {
        node_id: String,
        index: i32,
        count: usize,
    },
    #[error("action {name:?} was not found on {node_id}\nAvailable actions:\n{available}")]
    ActionNameNotFound {
        node_id: String,
        name: String,
        available: String,
    },
    #[error("action name {name:?} is ambiguous on {node_id}; matching indices: {indices}")]
    AmbiguousActionName {
        node_id: String,
        name: String,
        indices: String,
    },
    #[error("AT-SPI action {index} on {node_id} returned false")]
    ActionRejected { node_id: String, index: i32 },
    #[error("D-Bus call failed while accessing {node_id}: {source}")]
    DbusCall {
        node_id: String,
        source: atspi::AtspiError,
    },
    #[error("permission denied while accessing AT-SPI object {node_id}: {source}")]
    PermissionDenied {
        node_id: String,
        source: atspi::AtspiError,
    },
    #[error("--action requires a zero-based --index")]
    MissingActionIndex,
    #[error("--action-name requires NODE_ID and NAME")]
    MissingActionNameArguments,
    #[error("AT-SPI operation {operation:?} timed out for {node_id} after {timeout_ms} ms")]
    OperationTimeout {
        operation: &'static str,
        node_id: String,
        timeout_ms: u128,
    },
}

pub struct AtspiBackend {
    connection: AccessibilityConnection,
    operation_timeout: Duration,
}

impl AtspiBackend {
    pub async fn connect(operation_timeout: Duration) -> Result<Self, BackendError> {
        let environment = SessionEnvironment::detect();
        let _session_connection =
            tokio::time::timeout(operation_timeout, zbus::Connection::session())
                .await
                .map_err(|_| {
                    timeout_error(
                        operation_timeout,
                        "connect session D-Bus",
                        "desktop-session",
                    )
                })?
                .map_err(|source| BackendError::NoDesktopSession {
                    environment: environment.summary(),
                    source: Box::new(source),
                })?;
        let connection = tokio::time::timeout(operation_timeout, AccessibilityConnection::new())
            .await
            .map_err(|_| timeout_error(operation_timeout, "connect AT-SPI bus", "desktop-session"))?
            .map_err(|source| BackendError::AtspiUnavailable {
                environment: environment.summary(),
                source,
            })?;
        Ok(Self {
            connection,
            operation_timeout,
        })
    }

    pub async fn applications(&self) -> Result<Vec<ApplicationRef>, BackendError> {
        let root = atspi_operation(
            self.operation_timeout,
            "create registry root proxy",
            "atspi-registry",
            self.connection.root_accessible_on_registry(),
        )
        .await?;
        let children = dbus_operation(
            self.operation_timeout,
            "enumerate applications",
            "atspi-registry",
            root.get_children(),
        )
        .await?;

        let mut applications = Vec::with_capacity(children.len());
        for object in children {
            if object.is_null() {
                continue;
            }
            let Some(id) = node_id_from_ref(&object) else {
                continue;
            };
            match object
                .as_accessible_proxy(self.connection.connection())
                .await
            {
                Ok(proxy) => {
                    let name = dbus_operation(
                        self.operation_timeout,
                        "read application name",
                        &id.encode(),
                        proxy.name(),
                    )
                    .await
                    .unwrap_or_else(|_| "<unnamed application>".to_owned());
                    applications.push(ApplicationRef {
                        index: applications.len() + 1,
                        name,
                        backend_locator: id,
                        object,
                    });
                }
                Err(error) => warn!(%error, "skipping stale AT-SPI application object"),
            }
        }

        Ok(applications)
    }

    pub fn select_application<'a>(
        applications: &'a [ApplicationRef],
        name: Option<&str>,
        index: Option<usize>,
    ) -> Result<&'a ApplicationRef, BackendError> {
        if let Some(index) = index {
            return applications
                .iter()
                .find(|app| app.index == index)
                .ok_or(BackendError::ApplicationIndexNotFound(index));
        }

        let selector = name.ok_or(BackendError::NoApplications)?;
        let selector_lower = selector.to_lowercase();
        if let Some(exact) = applications
            .iter()
            .find(|app| app.name.to_lowercase() == selector_lower)
        {
            return Ok(exact);
        }

        let matches: Vec<_> = applications
            .iter()
            .filter(|app| app.name.to_lowercase().contains(&selector_lower))
            .collect();
        match matches.as_slice() {
            [] => Err(BackendError::ApplicationNotFound(selector.to_owned())),
            [app] => Ok(app),
            _ => Err(BackendError::AmbiguousApplication {
                selector: selector.to_owned(),
                matches: matches
                    .iter()
                    .map(|app| app.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }),
        }
    }

    pub async fn inspect_application(
        &self,
        application: &ApplicationRef,
        options: InspectOptions,
    ) -> Result<SemanticNode, BackendError> {
        let mut context = TraversalContext {
            options,
            visited: HashSet::new(),
            nodes: 0,
            runtime_ids: RuntimeIdAllocator::default(),
        };
        self.build_node(application.object.clone(), 0, &mut context)
            .await
    }

    pub async fn actions(&self, encoded_id: &str) -> Result<Vec<SemanticAction>, BackendError> {
        let id = BackendLocator::decode(encoded_id)?;
        let object = object_ref_from_id(&id)?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(id.encode(), error))?;
        let interfaces = dbus_operation(
            self.operation_timeout,
            "read interfaces",
            &id.encode(),
            proxy.get_interfaces(),
        )
        .await?;
        if !interfaces.contains(Interface::Action) {
            return Err(BackendError::ActionUnsupported(id.encode()));
        }
        let proxies = atspi_operation(
            self.operation_timeout,
            "create interface proxies",
            &id.encode(),
            proxy.proxies(),
        )
        .await?;
        let action_proxy = atspi_operation(
            self.operation_timeout,
            "create Action proxy",
            &id.encode(),
            proxies.action(),
        )
        .await?;
        let actions = dbus_operation(
            self.operation_timeout,
            "read actions",
            &id.encode(),
            action_proxy.get_actions(),
        )
        .await?;
        Ok(map_actions(actions))
    }

    pub async fn activate(&self, encoded_id: &str) -> Result<SemanticAction, BackendError> {
        let actions = self.actions(encoded_id).await?;
        let preferred = ["press", "click", "activate", "open"];
        let selected = preferred
            .iter()
            .find_map(|name| {
                actions
                    .iter()
                    .find(|action| action.name.eq_ignore_ascii_case(name))
            })
            .or_else(|| actions.first())
            .ok_or_else(|| BackendError::NoActions(encoded_id.to_owned()))?
            .clone();
        self.do_action(encoded_id, selected.index).await?;
        Ok(selected)
    }

    pub async fn do_action_by_name(
        &self,
        encoded_id: &str,
        name: &str,
    ) -> Result<SemanticAction, BackendError> {
        let actions = self.actions(encoded_id).await?;
        let selected = select_action_by_name(encoded_id, &actions, name)?;
        self.do_action(encoded_id, selected.index).await?;
        Ok(selected)
    }

    pub async fn do_action(
        &self,
        encoded_id: &str,
        index: i32,
    ) -> Result<SemanticAction, BackendError> {
        let actions = self.actions(encoded_id).await?;
        let selected = actions
            .get(usize::try_from(index).unwrap_or(usize::MAX))
            .ok_or_else(|| BackendError::ActionNotFound {
                node_id: encoded_id.to_owned(),
                index,
                count: actions.len(),
            })?
            .clone();

        let id = BackendLocator::decode(encoded_id)?;
        let object = object_ref_from_id(&id)?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(id.encode(), error))?;
        let proxies = atspi_operation(
            self.operation_timeout,
            "create interface proxies",
            &id.encode(),
            proxy.proxies(),
        )
        .await?;
        let action_proxy = atspi_operation(
            self.operation_timeout,
            "create Action proxy",
            &id.encode(),
            proxies.action(),
        )
        .await?;
        let accepted = dbus_operation(
            self.operation_timeout,
            "invoke action",
            &id.encode(),
            action_proxy.do_action(index),
        )
        .await?;
        if !accepted {
            return Err(BackendError::ActionRejected {
                node_id: id.encode(),
                index,
            });
        }
        Ok(selected)
    }

    fn build_node<'a>(
        &'a self,
        object: ObjectRefOwned,
        depth: usize,
        context: &'a mut TraversalContext,
    ) -> Pin<Box<dyn Future<Output = Result<SemanticNode, BackendError>> + Send + 'a>> {
        Box::pin(async move {
            context.nodes += 1;
            let runtime_id = context.runtime_ids.allocate();

            let id = node_id_from_ref(&object).ok_or_else(|| {
                BackendError::ObjectUnavailable(
                    "<null>".to_owned(),
                    atspi::AtspiError::NullRef("tree contains a null object reference"),
                )
            })?;
            let encoded_id = id.encode();
            context.visited.insert(encoded_id.clone());

            let proxy = object
                .as_accessible_proxy(self.connection.connection())
                .await
                .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
            let role = dbus_operation(
                self.operation_timeout,
                "read role",
                &encoded_id,
                proxy.get_role(),
            )
            .await?;
            let name = dbus_operation(
                self.operation_timeout,
                "read name",
                &encoded_id,
                proxy.name(),
            )
            .await
            .map(nonempty)
            .unwrap_or_else(|error| {
                warn!(node_id = %encoded_id, %error, "could not read AT-SPI name");
                None
            });
            let description = dbus_operation(
                self.operation_timeout,
                "read description",
                &encoded_id,
                proxy.description(),
            )
            .await
            .map(nonempty)
            .unwrap_or_else(|error| {
                warn!(node_id = %encoded_id, %error, "could not read AT-SPI description");
                None
            });
            let states = dbus_operation(
                self.operation_timeout,
                "read states",
                &encoded_id,
                proxy.get_state(),
            )
            .await
            .map(|set| set.into_iter().map(SemanticState::from).collect())
            .unwrap_or_else(|error| {
                warn!(node_id = %encoded_id, %error, "could not read AT-SPI states");
                Vec::new()
            });
            let interfaces = dbus_operation(
                self.operation_timeout,
                "read interfaces",
                &encoded_id,
                proxy.get_interfaces(),
            )
            .await
            .unwrap_or_else(|error| {
                warn!(node_id = %encoded_id, %error, "could not read AT-SPI interfaces");
                Default::default()
            });

            let proxies = atspi_operation(
                self.operation_timeout,
                "create interface proxies",
                &encoded_id,
                proxy.proxies(),
            )
            .await
            .map_err(|error| {
                warn!(node_id = %encoded_id, %error, "could not create interface proxies");
                error
            })
            .ok();
            let actions = if interfaces.contains(Interface::Action) {
                if let Some(proxies) = &proxies {
                    match atspi_operation(
                        self.operation_timeout,
                        "create Action proxy",
                        &encoded_id,
                        proxies.action(),
                    )
                    .await
                    {
                        Ok(action_proxy) => dbus_operation(
                            self.operation_timeout,
                            "read actions",
                            &encoded_id,
                            action_proxy.get_actions(),
                        )
                        .await
                        .map(map_actions)
                        .unwrap_or_else(|error| {
                            warn!(node_id = %encoded_id, %error, "could not read AT-SPI actions");
                            Vec::new()
                        }),
                        Err(error) => {
                            warn!(node_id = %encoded_id, %error, "could not create Action proxy");
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let value = read_value(
                self.operation_timeout,
                &encoded_id,
                role,
                interfaces,
                proxies.as_ref(),
            )
            .await;
            let geometry = if context.options.verbose && interfaces.contains(Interface::Component) {
                read_geometry(self.operation_timeout, &encoded_id, &proxy).await
            } else {
                None
            };
            let debug = DebugInfo {
                atspi_role: role.name().to_owned(),
                bus_name: id.bus_name().to_owned(),
                object_path: id.object_path().to_owned(),
                interfaces: interfaces
                    .iter()
                    .map(|interface| format!("{interface:?}"))
                    .collect(),
                geometry,
            };

            let mut children = Vec::new();
            let mut truncations = Vec::new();
            if depth < context.options.max_depth {
                match dbus_operation(
                    self.operation_timeout,
                    "read children",
                    &encoded_id,
                    proxy.get_children(),
                )
                .await
                {
                    Ok(child_refs) => {
                        for child in child_refs {
                            if context.nodes >= context.options.max_nodes {
                                truncations.push(TreeTruncation::MaxNodes {
                                    limit: context.options.max_nodes,
                                });
                                break;
                            }
                            let Some(child_id) = node_id_from_ref(&child) else {
                                continue;
                            };
                            if context.visited.contains(&child_id.encode()) {
                                warn!(node_id = %child_id, "skipping AT-SPI cycle or duplicate object");
                                continue;
                            }
                            match self.build_node(child, depth + 1, context).await {
                                Ok(child) => children.push(child),
                                Err(BackendError::ObjectUnavailable(node_id, error)) => {
                                    warn!(%node_id, %error, "skipping stale AT-SPI child object");
                                }
                                Err(BackendError::OperationTimeout {
                                    operation, node_id, ..
                                }) => {
                                    warn!(%node_id, %operation, "skipping timed-out AT-SPI child object");
                                    truncations.push(TreeTruncation::OperationTimeout {
                                        operation,
                                        node_id,
                                    });
                                }
                                Err(error) => return Err(error),
                            }
                        }
                    }
                    Err(error) => {
                        warn!(node_id = %encoded_id, %error, "could not read AT-SPI children");
                    }
                }
            } else {
                match dbus_operation(
                    self.operation_timeout,
                    "read child count",
                    &encoded_id,
                    proxy.child_count(),
                )
                .await
                {
                    Ok(count) if count > 0 => truncations.push(TreeTruncation::MaxDepth {
                        limit: context.options.max_depth,
                    }),
                    Ok(_) => {}
                    Err(BackendError::OperationTimeout {
                        operation, node_id, ..
                    }) => truncations.push(TreeTruncation::OperationTimeout { operation, node_id }),
                    Err(error) => {
                        warn!(node_id = %encoded_id, %error, "could not determine max-depth truncation");
                    }
                }
            }

            Ok(SemanticNode {
                runtime_id,
                backend_locator: id,
                role: SemanticRole::from_atspi(role, interfaces.contains(Interface::EditableText)),
                name,
                description,
                value,
                sensitive: role == Role::PasswordText,
                states,
                actions,
                children,
                truncations,
                debug,
            })
        })
    }
}

struct TraversalContext {
    options: InspectOptions,
    visited: HashSet<String>,
    nodes: usize,
    runtime_ids: RuntimeIdAllocator,
}

fn node_id_from_ref(object: &ObjectRefOwned) -> Option<BackendLocator> {
    Some(BackendLocator::new(
        object.name_as_str()?,
        object.path_as_str().to_owned(),
    ))
}

fn object_ref_from_id(id: &BackendLocator) -> Result<ObjectRefOwned, BackendError> {
    let name = UniqueName::try_from(id.bus_name().to_owned()).map_err(|_| {
        crate::semantic::BackendLocatorError::InvalidBusName(id.bus_name().to_owned())
    })?;
    let path = ObjectPath::try_from(id.object_path().to_owned()).map_err(|_| {
        crate::semantic::BackendLocatorError::InvalidObjectPath(id.object_path().to_owned())
    })?;
    Ok(ObjectRef::new_owned(name, path))
}

async fn dbus_operation<T, F>(
    timeout: Duration,
    operation: &'static str,
    node_id: &str,
    future: F,
) -> Result<T, BackendError>
where
    F: Future<Output = zbus::Result<T>>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| timeout_error(timeout, operation, node_id))?
        .map_err(|error| map_dbus_error(node_id.to_owned(), error))
}

async fn atspi_operation<T, F>(
    timeout: Duration,
    operation: &'static str,
    node_id: &str,
    future: F,
) -> Result<T, BackendError>
where
    F: Future<Output = Result<T, atspi::AtspiError>>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| timeout_error(timeout, operation, node_id))?
        .map_err(|source| BackendError::DbusCall {
            node_id: node_id.to_owned(),
            source,
        })
}

fn timeout_error(
    timeout: Duration,
    operation: &'static str,
    node_id: impl Into<String>,
) -> BackendError {
    BackendError::OperationTimeout {
        operation,
        node_id: node_id.into(),
        timeout_ms: timeout.as_millis(),
    }
}

fn map_dbus_error(node_id: String, error: zbus::Error) -> BackendError {
    let stale = match &error {
        zbus::Error::FDO(fdo) => matches!(
            fdo.as_ref(),
            zbus::fdo::Error::UnknownObject(_)
                | zbus::fdo::Error::NameHasNoOwner(_)
                | zbus::fdo::Error::ServiceUnknown(_)
        ),
        zbus::Error::MethodError(name, _, _) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.UnknownObject"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
                | "org.freedesktop.DBus.Error.ServiceUnknown"
        ),
        _ => false,
    };
    let permission_denied = match &error {
        zbus::Error::FDO(fdo) => matches!(
            fdo.as_ref(),
            zbus::fdo::Error::AccessDenied(_) | zbus::fdo::Error::AuthFailed(_)
        ),
        zbus::Error::MethodError(name, _, _) => matches!(
            name.as_str(),
            "org.freedesktop.DBus.Error.AccessDenied" | "org.freedesktop.DBus.Error.AuthFailed"
        ),
        _ => false,
    };
    let source = error.into();
    if stale {
        BackendError::ObjectUnavailable(node_id, source)
    } else if permission_denied {
        BackendError::PermissionDenied { node_id, source }
    } else {
        BackendError::DbusCall { node_id, source }
    }
}

fn map_actions(actions: Vec<atspi::Action>) -> Vec<SemanticAction> {
    actions
        .into_iter()
        .enumerate()
        .map(|(index, action)| SemanticAction {
            index: index as i32,
            name: action.name,
            description: nonempty(action.description),
            keybinding: nonempty(action.keybinding),
        })
        .collect()
}

fn select_action_by_name(
    node_id: &str,
    actions: &[SemanticAction],
    requested_name: &str,
) -> Result<SemanticAction, BackendError> {
    let exact: Vec<_> = actions
        .iter()
        .filter(|action| action.name == requested_name)
        .collect();
    let matches = if exact.is_empty() {
        actions
            .iter()
            .filter(|action| action.name.eq_ignore_ascii_case(requested_name))
            .collect::<Vec<_>>()
    } else {
        exact
    };

    match matches.as_slice() {
        [action] => Ok((*action).clone()),
        [] => Err(BackendError::ActionNameNotFound {
            node_id: node_id.to_owned(),
            name: requested_name.to_owned(),
            available: format_available_actions(actions),
        }),
        duplicates => Err(BackendError::AmbiguousActionName {
            node_id: node_id.to_owned(),
            name: requested_name.to_owned(),
            indices: duplicates
                .iter()
                .map(|action| action.index.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

fn format_available_actions(actions: &[SemanticAction]) -> String {
    if actions.is_empty() {
        return "  <none>".to_owned();
    }
    actions
        .iter()
        .map(|action| format!("  {} {}", action.index, action.name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

async fn read_value(
    timeout: Duration,
    node_id: &str,
    role: Role,
    interfaces: atspi::InterfaceSet,
    proxies: Option<&atspi::proxy::proxy_ext::Proxies<'_>>,
) -> Option<String> {
    let proxies = proxies?;
    if role_allows_text_value(role, interfaces) && interfaces.contains(Interface::Text) {
        let text_proxy: TextProxy<'_> =
            atspi_operation(timeout, "create Text proxy", node_id, proxies.text())
                .await
                .ok()?;
        let total_count = dbus_operation(
            timeout,
            "read character count",
            node_id,
            text_proxy.character_count(),
        )
        .await
        .ok()?;
        let count = total_count.clamp(0, 256);
        let mut text = dbus_operation(
            timeout,
            "read text value",
            node_id,
            text_proxy.get_text(0, count),
        )
        .await
        .ok()?;
        if total_count > count {
            text.push('…');
        }
        return nonempty(text);
    }
    if interfaces.contains(Interface::Value)
        && matches!(
            role,
            Role::Slider | Role::ProgressBar | Role::LevelBar | Role::SpinButton
        )
    {
        let value_proxy: ValueProxy<'_> =
            atspi_operation(timeout, "create Value proxy", node_id, proxies.value())
                .await
                .ok()?;
        if let Ok(text) =
            dbus_operation(timeout, "read textual value", node_id, value_proxy.text()).await
            && let Some(text) = nonempty(text)
        {
            return Some(text);
        }
        return dbus_operation(
            timeout,
            "read numeric value",
            node_id,
            value_proxy.current_value(),
        )
        .await
        .ok()
        .map(|value| value.to_string());
    }
    None
}

fn role_allows_text_value(role: Role, interfaces: atspi::InterfaceSet) -> bool {
    role != Role::PasswordText
        && interfaces.contains(Interface::EditableText)
        && matches!(
            role,
            Role::Text | Role::Entry | Role::DateEditor | Role::Editbar
        )
}

async fn read_geometry(
    timeout: Duration,
    node_id: &str,
    proxy: &atspi::proxy::accessible::AccessibleProxy<'_>,
) -> Option<Geometry> {
    let component = ComponentProxy::builder(proxy.inner().connection())
        .destination(proxy.inner().destination())
        .ok()?
        .path(proxy.inner().path())
        .ok()?
        .build()
        .await
        .ok()?;
    let (x, y, width, height) = dbus_operation(
        timeout,
        "read geometry",
        node_id,
        component.get_extents(CoordType::Screen),
    )
    .await
    .ok()?;
    Some(Geometry {
        x,
        y,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applications() -> Vec<ApplicationRef> {
        ["Firefox", "GNOME Settings", "GNOME Text Editor"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| ApplicationRef {
                index: index + 1,
                name: name.to_owned(),
                backend_locator: BackendLocator::new(":1.1", format!("/app/{index}")),
                object: ObjectRefOwned::default(),
            })
            .collect()
    }

    fn actions(names: &[&str]) -> Vec<SemanticAction> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| SemanticAction {
                index: index as i32,
                name: (*name).to_owned(),
                description: None,
                keybinding: None,
            })
            .collect()
    }

    #[test]
    fn selects_application_by_exact_or_unique_partial_name() {
        let apps = applications();
        assert_eq!(
            AtspiBackend::select_application(&apps, Some("firefox"), None)
                .unwrap()
                .name,
            "Firefox"
        );
        assert_eq!(
            AtspiBackend::select_application(&apps, Some("settings"), None)
                .unwrap()
                .name,
            "GNOME Settings"
        );
        assert_eq!(
            AtspiBackend::select_application(&apps, None, Some(3))
                .unwrap()
                .name,
            "GNOME Text Editor"
        );
    }

    #[test]
    fn rejects_ambiguous_application_name() {
        let apps = applications();
        assert!(matches!(
            AtspiBackend::select_application(&apps, Some("gnome"), None),
            Err(BackendError::AmbiguousApplication { .. })
        ));
    }

    #[test]
    fn action_name_prefers_exact_then_ascii_case_insensitive_match() {
        let available = actions(&["click", "Press"]);
        assert_eq!(
            select_action_by_name("node", &available, "Press")
                .unwrap()
                .index,
            1
        );
        assert_eq!(
            select_action_by_name("node", &available, "CLICK")
                .unwrap()
                .index,
            0
        );
    }

    #[test]
    fn missing_action_name_reports_available_actions() {
        let error =
            select_action_by_name("node", &actions(&["click", "show-menu"]), "press").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("action \"press\" was not found"));
        assert!(message.contains("  0 click"));
        assert!(message.contains("  1 show-menu"));
    }

    #[test]
    fn duplicate_action_name_is_rejected() {
        let error =
            select_action_by_name("node", &actions(&["click", "click"]), "click").unwrap_err();
        assert!(matches!(error, BackendError::AmbiguousActionName { .. }));
        assert!(error.to_string().contains("matching indices: 0, 1"));
    }

    #[test]
    fn password_role_never_allows_text_value_reads() {
        let editable_text = atspi::InterfaceSet::new(Interface::EditableText);
        assert!(role_allows_text_value(Role::Entry, editable_text));
        assert!(role_allows_text_value(Role::Text, editable_text));
        assert!(role_allows_text_value(Role::Editbar, editable_text));
        assert!(!role_allows_text_value(Role::PasswordText, editable_text));
    }

    #[test]
    fn classifies_unknown_object_as_stale() {
        let error = zbus::Error::FDO(Box::new(zbus::fdo::Error::UnknownObject("gone".to_owned())));
        assert!(matches!(
            map_dbus_error("node".to_owned(), error),
            BackendError::ObjectUnavailable(_, _)
        ));
    }

    #[test]
    fn classifies_application_gone_as_stale() {
        let error = zbus::Error::FDO(Box::new(zbus::fdo::Error::ServiceUnknown(
            "application gone".to_owned(),
        )));
        assert!(matches!(
            map_dbus_error("node".to_owned(), error),
            BackendError::ObjectUnavailable(_, _)
        ));
    }

    #[tokio::test]
    async fn remote_operation_timeout_is_bounded_and_classified() {
        let error = dbus_operation(
            Duration::from_millis(1),
            "test operation",
            "node",
            std::future::pending::<zbus::Result<()>>(),
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            BackendError::OperationTimeout {
                operation: "test operation",
                node_id,
                ..
            } if node_id == "node"
        ));
    }
}
