use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use atspi::{
    AccessibilityConnection, CoordType, Granularity, Interface, MatchType, ObjectMatchRule,
    ObjectRef, ObjectRefOwned, Role, SortOrder, State,
    events::{CacheEvents, Event, ObjectEvents, WindowEvents},
    proxy::{
        accessible::ObjectRefExt, action::ActionProxy, collection::CollectionProxy,
        component::ComponentProxy, document::DocumentProxy, hypertext::HypertextProxy,
        proxy_ext::ProxyExt, text::TextProxy, value::ValueProxy,
    },
};
use futures_lite::StreamExt;
use thiserror::Error;
use tracing::warn;
use zbus::{MessageStream, message::Type as MessageType, names::UniqueName, zvariant::ObjectPath};

use crate::{
    backend::{
        bootstrap::{
            BootstrapMetrics, BootstrapResult, BootstrapStrategy, BootstrapUsed, reconstruct_tree,
        },
        protocol_compat::{
            BulkAccessibleRecord, CacheFetch, fetch_cache, normalize_legacy_property_event,
        },
    },
    semantic::{
        BackendLocator, BackendRelation, DebugInfo, Geometry, RelationState, RuntimeIdAllocator,
        RuntimeNodeId, SemanticAction, SemanticCache, SemanticCapability, SemanticNode,
        SemanticRelationKind, SemanticRole, SemanticState, TextInputKind, TreeTruncation,
    },
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
    #[error("failed to register or consume AT-SPI events: {0}")]
    EventStream(atspi::AtspiError),
    #[error("semantic cache error: {0}")]
    SemanticCache(String),
    #[error("AT-SPI cache bootstrap failed: {0}")]
    CacheBootstrap(String),
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
    #[error("AT-SPI container {0} does not expose the Selection interface")]
    SelectionUnsupported(String),
    #[error("selection child index {index} is too large for AT-SPI container {node_id}")]
    SelectionIndexOutOfRange { node_id: String, index: usize },
    #[error("selection of child {index} was rejected by AT-SPI container {node_id}")]
    SelectionRejected { node_id: String, index: usize },
    #[error("AT-SPI object {0} is not a supported editable plain-text input")]
    TextEditUnsupported(String),
    #[error("password editing is disabled by GUI2TUI for AT-SPI object {0}")]
    PasswordEditDisabled(String),
    #[error("password/secret content reading is disabled by GUI2TUI for AT-SPI object {0}")]
    SecretContentDisabled(String),
    #[error("AT-SPI object {0} does not expose readable Text content")]
    ContentTextUnsupported(String),
    #[error("AT-SPI text range did not advance for {node_id}: {start}..{end}")]
    NonAdvancingTextRange {
        node_id: String,
        start: i32,
        end: i32,
    },
    #[error("application rejected text update for AT-SPI object {0}")]
    TextUpdateRejected(String),
    #[error(
        "no safe convenience action was found on {node_id}\nAvailable actions:\n{available}\nUse --action-name or --action --index for explicit low-level invocation"
    )]
    NoCompatibleAction { node_id: String, available: String },
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
    #[error("--select-child requires a zero-based --child-index")]
    MissingSelectionChildIndex,
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

pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 2048;

#[derive(Clone, Debug)]
pub enum EventDelivery {
    Event(crate::events::NormalizedEvent),
    ResyncRequired { dropped: u64 },
}

pub struct EventSubscription {
    receiver: tokio::sync::mpsc::Receiver<crate::events::NormalizedEvent>,
    resync_required: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    notify: Arc<tokio::sync::Notify>,
}

#[derive(Clone, Debug)]
pub struct CollectionQueryProbe {
    pub query: &'static str,
    pub count: Option<usize>,
    pub duration: Duration,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CollectionProbe {
    pub collection_nodes: usize,
    pub source: Option<BackendLocator>,
    pub queries: Vec<CollectionQueryProbe>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentProbe {
    pub locator: Option<BackendLocator>,
    pub document_interface: bool,
    pub text_interface: bool,
    pub hypertext_interface: bool,
    pub locale: Option<String>,
    pub current_page: Option<i32>,
    pub page_count: Option<i32>,
    pub attributes: std::collections::HashMap<String, String>,
    pub character_count: Option<i32>,
    pub hyperlink_count: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRangeRead {
    pub start: i32,
    pub end: i32,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct RelationEnrichmentMetrics {
    pub candidate_nodes: usize,
    pub rpc_count: usize,
    pub relations_found: usize,
    pub unresolved_targets: usize,
    pub unavailable_nodes: usize,
    pub duration: Duration,
}

impl EventSubscription {
    pub async fn recv(&mut self) -> Option<EventDelivery> {
        loop {
            if let Some(resync) = self.take_resync() {
                return Some(resync);
            }
            tokio::select! {
                event = self.receiver.recv() => return event.map(EventDelivery::Event),
                _ = self.notify.notified() => {
                    // A notification permit may outlive the flag consumption
                    // when the owner observed overflow before awaiting here.
                    // It is not an event-stream closure; loop and re-check.
                }
            }
        }
    }

    pub fn try_recv(
        &mut self,
    ) -> Result<crate::events::NormalizedEvent, tokio::sync::mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn take_resync(&self) -> Option<EventDelivery> {
        self.resync_required
            .swap(false, Ordering::AcqRel)
            .then(|| EventDelivery::ResyncRequired {
                dropped: self.dropped.swap(0, Ordering::AcqRel),
            })
    }
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

    pub async fn probe_cache(
        &self,
        application: &ApplicationRef,
    ) -> Result<CacheFetch, BackendError> {
        tokio::time::timeout(
            self.operation_timeout,
            fetch_cache(
                self.connection.connection(),
                application.backend_locator.bus_name(),
            ),
        )
        .await
        .map_err(|_| {
            timeout_error(
                self.operation_timeout,
                "AT-SPI Cache.GetItems",
                application.backend_locator.encode(),
            )
        })?
        .map_err(|error| BackendError::CacheBootstrap(error.to_string()))
    }

    pub async fn probe_collection(
        &self,
        application: &ApplicationRef,
    ) -> Result<CollectionProbe, BackendError> {
        let fetch = self.probe_cache(application).await?;
        let collection_nodes: Vec<_> = fetch
            .records
            .iter()
            .filter(|record| record.interfaces.contains(Interface::Collection))
            .map(|record| record.locator.clone())
            .collect();
        let Some(source) = collection_nodes
            .iter()
            .find(|locator| **locator == application.backend_locator)
            .cloned()
            .or_else(|| collection_nodes.first().cloned())
        else {
            return Ok(CollectionProbe {
                collection_nodes: 0,
                source: None,
                queries: Vec::new(),
            });
        };
        let proxy = CollectionProxy::builder(self.connection.connection())
            .destination(source.bus_name())
            .and_then(|builder| builder.path(source.object_path()))
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
            .build()
            .await
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
        let queries = vec![
            (
                "buttons",
                ObjectMatchRule::builder()
                    .roles(&[Role::Button, Role::PushButtonMenu], MatchType::Any)
                    .build(),
            ),
            (
                "text-inputs",
                ObjectMatchRule::builder()
                    .roles(
                        &[Role::Entry, Role::PasswordText, Role::Text],
                        MatchType::Any,
                    )
                    .build(),
            ),
            (
                "checkboxes",
                ObjectMatchRule::builder()
                    .roles(&[Role::CheckBox], MatchType::Any)
                    .build(),
            ),
            (
                "focusable",
                ObjectMatchRule::builder()
                    .states([State::Focusable], MatchType::All)
                    .build(),
            ),
        ];
        let mut results = Vec::new();
        for (query, rule) in queries {
            let started = Instant::now();
            let result = tokio::time::timeout(
                self.operation_timeout,
                proxy.get_matches(rule, SortOrder::Canonical, 0, false),
            )
            .await;
            let (count, error) = match result {
                Ok(Ok(objects)) => (Some(objects.len()), None),
                Ok(Err(error)) => (None, Some(error.to_string())),
                Err(_) => (None, Some("operation timed out".to_owned())),
            };
            results.push(CollectionQueryProbe {
                query,
                count,
                duration: started.elapsed(),
                error,
            });
        }
        Ok(CollectionProbe {
            collection_nodes: collection_nodes.len(),
            source: Some(source),
            queries: results,
        })
    }

    pub async fn bootstrap_application(
        &self,
        application: &ApplicationRef,
        options: InspectOptions,
        strategy: BootstrapStrategy,
    ) -> Result<BootstrapResult, BackendError> {
        let started = Instant::now();
        if strategy != BootstrapStrategy::Walk && !options.verbose {
            match self
                .bootstrap_from_cache(application, options, started)
                .await
            {
                Ok(result) => return Ok(result),
                Err(error) if strategy == BootstrapStrategy::Cache => return Err(error),
                Err(error) => {
                    let reason = error.to_string();
                    let root = self.inspect_application(application, options).await?;
                    return Ok(BootstrapResult {
                        metrics: BootstrapMetrics {
                            strategy: BootstrapUsed::Walk,
                            node_count: semantic_node_count(&root),
                            cache_format: None,
                            cache_items: 0,
                            cache_rpc: Duration::ZERO,
                            enrichment: Duration::ZERO,
                            enrichment_rpc_count: 0,
                            reconstruction: Duration::ZERO,
                            total: started.elapsed(),
                            orphans_ignored: 0,
                            fallback_reason: Some(reason),
                        },
                        root,
                    });
                }
            }
        }

        let root = self.inspect_application(application, options).await?;
        Ok(BootstrapResult {
            metrics: BootstrapMetrics {
                strategy: BootstrapUsed::Walk,
                node_count: semantic_node_count(&root),
                cache_format: None,
                cache_items: 0,
                cache_rpc: Duration::ZERO,
                enrichment: Duration::ZERO,
                enrichment_rpc_count: 0,
                reconstruction: Duration::ZERO,
                total: started.elapsed(),
                orphans_ignored: 0,
                fallback_reason: (strategy == BootstrapStrategy::Auto && options.verbose)
                    .then(|| "verbose inspection uses recursive debug path".to_owned()),
            },
            root,
        })
    }

    async fn bootstrap_from_cache(
        &self,
        application: &ApplicationRef,
        options: InspectOptions,
        started: Instant,
    ) -> Result<BootstrapResult, BackendError> {
        let fetch = self.probe_cache(application).await?;
        if fetch.records.is_empty() {
            return Err(BackendError::CacheBootstrap(
                "Cache.GetItems returned no records".to_owned(),
            ));
        }
        let cache_items = fetch.records.len();
        let cache_format = fetch.format;
        let cache_rpc = fetch.rpc_duration;
        let enrichment_started = Instant::now();
        let mut records = fetch.records;
        let mut root_relationship_rpc_count = 0;
        if let Ok(proxy) = application
            .object
            .as_accessible_proxy(self.connection.connection())
            .await
        {
            root_relationship_rpc_count = 1;
            if let Ok(root_children) = dbus_operation(
                self.operation_timeout,
                "read application root children for cache reconstruction",
                &application.backend_locator.encode(),
                proxy.get_children(),
            )
            .await
            {
                repair_root_relationships(
                    &mut records,
                    &application.backend_locator,
                    &root_children,
                );
            }
        }
        let (records, enrichment_rpc_count) = enrich_records(
            self.connection.connection().clone(),
            self.operation_timeout,
            records,
        )
        .await;
        let enrichment_rpc_count = enrichment_rpc_count + root_relationship_rpc_count;
        let enrichment = enrichment_started.elapsed();
        let reconstruction_started = Instant::now();
        let (root, stats) = reconstruct_tree(records, &application.backend_locator, options)
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
        let reconstruction = reconstruction_started.elapsed();
        Ok(BootstrapResult {
            metrics: BootstrapMetrics {
                strategy: BootstrapUsed::Cache,
                node_count: semantic_node_count(&root),
                cache_format: Some(cache_format),
                cache_items,
                cache_rpc,
                enrichment,
                enrichment_rpc_count,
                reconstruction,
                total: started.elapsed(),
                orphans_ignored: stats.orphans_ignored,
                fallback_reason: None,
            },
            root,
        })
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
        self.build_node(application.object.clone(), 0, None, &mut context)
            .await
    }

    pub async fn refresh_node(
        &self,
        locator: &BackendLocator,
        verbose: bool,
    ) -> Result<SemanticNode, BackendError> {
        let mut node = self
            .refresh_subtree(
                locator,
                InspectOptions {
                    verbose,
                    max_depth: 0,
                    max_nodes: 1,
                },
            )
            .await?;
        node.truncations.clear();
        Ok(node)
    }

    /// Read one object's AT-SPI RelationSet without traversing its subtree.
    pub async fn relations(
        &self,
        locator: &BackendLocator,
    ) -> Result<Vec<BackendRelation>, BackendError> {
        fetch_relation_set(
            self.connection.connection(),
            self.operation_timeout,
            locator,
        )
        .await
    }

    /// Lazily enrich only requested arena nodes. The caller remains the sole
    /// semantic-cache writer; spawned tasks perform remote reads only.
    pub async fn enrich_relations(
        &self,
        cache: &mut SemanticCache,
        candidates: &[RuntimeNodeId],
    ) -> RelationEnrichmentMetrics {
        let started = Instant::now();
        let mut tasks = tokio::task::JoinSet::new();
        let mut metrics = RelationEnrichmentMetrics {
            candidate_nodes: candidates.len(),
            ..Default::default()
        };
        for id in candidates {
            if !matches!(cache.relation_state(*id), Some(RelationState::Unknown)) {
                continue;
            }
            let Some(node) = cache.node(*id) else {
                continue;
            };
            let id = *id;
            let locator = node.backend_locator.clone();
            let connection = self.connection.connection().clone();
            let timeout = self.operation_timeout;
            tasks.spawn(async move {
                let result = fetch_relation_set(&connection, timeout, &locator).await;
                (id, result)
            });
        }
        while let Some(result) = tasks.join_next().await {
            let Ok((id, result)) = result else {
                metrics.unavailable_nodes += 1;
                continue;
            };
            metrics.rpc_count += 1;
            match result {
                Ok(relations) => {
                    metrics.relations_found += relations.len();
                    let targets = relations
                        .iter()
                        .map(|relation| relation.targets.len())
                        .sum::<usize>();
                    if cache.set_relations(id, relations).is_err() {
                        metrics.unavailable_nodes += 1;
                        continue;
                    }
                    let resolved = match cache.relation_state(id) {
                        Some(RelationState::Known(relations)) => relations
                            .iter()
                            .flat_map(|relation| &relation.targets)
                            .filter(|target| target.runtime_id.is_some())
                            .count(),
                        _ => 0,
                    };
                    metrics.unresolved_targets += targets.saturating_sub(resolved);
                }
                Err(_) => {
                    let _ = cache.mark_relations_unavailable(id);
                    metrics.unavailable_nodes += 1;
                }
            }
        }
        metrics.duration = started.elapsed();
        metrics
    }

    /// Read the complete authoritative value used to seed a local edit buffer.
    /// Password nodes are rejected before the Text interface is accessed.
    pub async fn read_full_editable_text(
        &self,
        locator: &BackendLocator,
    ) -> Result<String, BackendError> {
        let (encoded_id, object) = self.validate_plain_editable_text(locator).await?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
        let proxies = atspi_operation(
            self.operation_timeout,
            "create interface proxies for text read",
            &encoded_id,
            proxy.proxies(),
        )
        .await?;
        let text = atspi_operation(
            self.operation_timeout,
            "create Text proxy for complete edit value",
            &encoded_id,
            proxies.text(),
        )
        .await?;
        let count = dbus_operation(
            self.operation_timeout,
            "read complete editable character count",
            &encoded_id,
            text.character_count(),
        )
        .await?;
        dbus_operation(
            self.operation_timeout,
            "read complete editable text",
            &encoded_id,
            text.get_text(0, count.max(0)),
        )
        .await
    }

    /// Read document metadata without making it a tree-membership or toolkit contract.
    pub async fn probe_document(
        &self,
        locator: &BackendLocator,
    ) -> Result<DocumentProbe, BackendError> {
        let (encoded_id, object, interfaces, role) = self.validate_content_object(locator).await?;
        let mut result = DocumentProbe {
            locator: Some(locator.clone()),
            document_interface: interfaces.contains(Interface::Document),
            text_interface: interfaces.contains(Interface::Text),
            hypertext_interface: interfaces.contains(Interface::Hypertext),
            ..Default::default()
        };
        if role == Role::PasswordText {
            return Err(BackendError::SecretContentDisabled(encoded_id));
        }
        if interfaces.contains(Interface::Document) {
            let proxy = DocumentProxy::builder(self.connection.connection())
                .destination(locator.bus_name())
                .and_then(|builder| builder.path(locator.object_path()))
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
                .build()
                .await
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
            result.locale = dbus_operation(
                self.operation_timeout,
                "read Document locale",
                &encoded_id,
                proxy.get_locale(),
            )
            .await
            .ok()
            .and_then(nonempty);
            result.current_page = dbus_operation(
                self.operation_timeout,
                "read Document current page",
                &encoded_id,
                proxy.current_page_number(),
            )
            .await
            .ok();
            result.page_count = dbus_operation(
                self.operation_timeout,
                "read Document page count",
                &encoded_id,
                proxy.page_count(),
            )
            .await
            .ok();
            result.attributes = dbus_operation(
                self.operation_timeout,
                "read Document attributes",
                &encoded_id,
                proxy.get_attributes(),
            )
            .await
            .unwrap_or_default();
        }
        if interfaces.contains(Interface::Text) {
            let proxy = TextProxy::builder(self.connection.connection())
                .destination(locator.bus_name())
                .and_then(|builder| builder.path(locator.object_path()))
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
                .build()
                .await
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
            result.character_count = dbus_operation(
                self.operation_timeout,
                "read content character count",
                &encoded_id,
                proxy.character_count(),
            )
            .await
            .ok();
        }
        if interfaces.contains(Interface::Hypertext) {
            let proxy = HypertextProxy::builder(self.connection.connection())
                .destination(locator.bus_name())
                .and_then(|builder| builder.path(locator.object_path()))
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
                .build()
                .await
                .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
            result.hyperlink_count = dbus_operation(
                self.operation_timeout,
                "read Hypertext link count",
                &encoded_id,
                proxy.get_n_links(),
            )
            .await
            .ok();
        }
        drop(object);
        Ok(result)
    }

    /// Read one already-semantic non-password block without re-segmenting it.
    pub async fn read_semantic_text_block(
        &self,
        locator: &BackendLocator,
    ) -> Result<TextRangeRead, BackendError> {
        let (encoded_id, _object, interfaces, _) = self.validate_content_object(locator).await?;
        if !interfaces.contains(Interface::Text) {
            return Err(BackendError::ContentTextUnsupported(encoded_id));
        }
        let proxy = TextProxy::builder(self.connection.connection())
            .destination(locator.bus_name())
            .and_then(|builder| builder.path(locator.object_path()))
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
            .build()
            .await
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
        let character_count = dbus_operation(
            self.operation_timeout,
            "read semantic block character count",
            &encoded_id,
            proxy.character_count(),
        )
        .await?
        .max(0);
        let text = dbus_operation(
            self.operation_timeout,
            "read semantic block text",
            &encoded_id,
            proxy.get_text(0, character_count),
        )
        .await?;
        Ok(TextRangeRead {
            start: 0,
            end: character_count,
            text,
        })
    }

    /// Progressively read paragraph ranges from one non-password Text object.
    pub async fn read_content_paragraphs(
        &self,
        locator: &BackendLocator,
        start_offset: i32,
        max_ranges: usize,
    ) -> Result<(i32, Vec<TextRangeRead>), BackendError> {
        let (encoded_id, _object, interfaces, _) = self.validate_content_object(locator).await?;
        if !interfaces.contains(Interface::Text) {
            return Err(BackendError::ContentTextUnsupported(encoded_id));
        }
        let proxy = TextProxy::builder(self.connection.connection())
            .destination(locator.bus_name())
            .and_then(|builder| builder.path(locator.object_path()))
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?
            .build()
            .await
            .map_err(|error| BackendError::CacheBootstrap(error.to_string()))?;
        let character_count = dbus_operation(
            self.operation_timeout,
            "read content character count",
            &encoded_id,
            proxy.character_count(),
        )
        .await?
        .max(0);
        let mut offset = start_offset.clamp(0, character_count);
        let mut ranges = Vec::new();
        while offset < character_count && ranges.len() < max_ranges {
            let (text, start, end) = dbus_operation(
                self.operation_timeout,
                "read semantic paragraph",
                &encoded_id,
                proxy.get_string_at_offset(offset, Granularity::Paragraph),
            )
            .await?;
            if end <= offset {
                // GTK TextView can advertise paragraph granularity while
                // returning a non-advancing range. Fall back to a bounded
                // character chunk; never spin and never fetch an unbounded
                // whole document.
                let chunk_end = offset.saturating_add(4096).min(character_count);
                if chunk_end <= offset {
                    return Err(BackendError::NonAdvancingTextRange {
                        node_id: encoded_id,
                        start,
                        end,
                    });
                }
                let text = dbus_operation(
                    self.operation_timeout,
                    "read bounded content chunk",
                    &encoded_id,
                    proxy.get_text(offset, chunk_end),
                )
                .await?;
                ranges.push(TextRangeRead {
                    start: offset,
                    end: chunk_end,
                    text,
                });
                offset = chunk_end;
                continue;
            }
            ranges.push(TextRangeRead { start, end, text });
            offset = end;
        }
        Ok((character_count, ranges))
    }

    async fn validate_content_object(
        &self,
        locator: &BackendLocator,
    ) -> Result<(String, ObjectRefOwned, atspi::InterfaceSet, Role), BackendError> {
        let encoded_id = locator.encode();
        let object = object_ref_from_id(locator)?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
        let role = dbus_operation(
            self.operation_timeout,
            "validate content role",
            &encoded_id,
            proxy.get_role(),
        )
        .await?;
        if role == Role::PasswordText {
            return Err(BackendError::SecretContentDisabled(encoded_id));
        }
        let interfaces = dbus_operation(
            self.operation_timeout,
            "read content interfaces",
            &encoded_id,
            proxy.get_interfaces(),
        )
        .await?;
        drop(proxy);
        Ok((encoded_id, object, interfaces, role))
    }

    /// Atomically replace a plain editable text control through AT-SPI.
    pub async fn set_text_contents(
        &self,
        locator: &BackendLocator,
        new_text: &str,
    ) -> Result<(), BackendError> {
        let (encoded_id, object) = self.validate_plain_editable_text(locator).await?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
        let proxies = atspi_operation(
            self.operation_timeout,
            "create interface proxies for text update",
            &encoded_id,
            proxy.proxies(),
        )
        .await?;
        let editable = atspi_operation(
            self.operation_timeout,
            "create EditableText proxy",
            &encoded_id,
            proxies.editable_text(),
        )
        .await?;
        let accepted = dbus_operation(
            self.operation_timeout,
            "replace editable text contents",
            &encoded_id,
            editable.set_text_contents(new_text),
        )
        .await?;
        if accepted {
            Ok(())
        } else {
            Err(BackendError::TextUpdateRejected(encoded_id))
        }
    }

    async fn validate_plain_editable_text(
        &self,
        locator: &BackendLocator,
    ) -> Result<(String, ObjectRefOwned), BackendError> {
        let encoded_id = locator.encode();
        let object = object_ref_from_id(locator)?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
        let role = dbus_operation(
            self.operation_timeout,
            "validate editable text role",
            &encoded_id,
            proxy.get_role(),
        )
        .await?;
        if role == Role::PasswordText {
            return Err(BackendError::PasswordEditDisabled(encoded_id));
        }
        let interfaces = dbus_operation(
            self.operation_timeout,
            "validate EditableText interfaces",
            &encoded_id,
            proxy.get_interfaces(),
        )
        .await?;
        let states = dbus_operation(
            self.operation_timeout,
            "validate editable state",
            &encoded_id,
            proxy.get_state(),
        )
        .await?;
        let semantic_role =
            SemanticRole::from_atspi(role, interfaces.contains(Interface::EditableText));
        if semantic_role != SemanticRole::TextInput
            || !interfaces.contains(Interface::EditableText)
            || !interfaces.contains(Interface::Text)
            || !states.contains(State::Editable)
        {
            return Err(BackendError::TextEditUnsupported(encoded_id));
        }
        drop(proxy);
        Ok((encoded_id, object))
    }

    pub async fn refresh_subtree(
        &self,
        locator: &BackendLocator,
        options: InspectOptions,
    ) -> Result<SemanticNode, BackendError> {
        let object = object_ref_from_id(locator)?;
        let mut context = TraversalContext {
            options,
            visited: HashSet::new(),
            nodes: 0,
            runtime_ids: RuntimeIdAllocator::default(),
        };
        self.build_node(object, 0, None, &mut context).await
    }

    /// Print a normalized, read-only event stream for one selected application.
    pub async fn watch_events(&self, application: &ApplicationRef) -> Result<(), BackendError> {
        self.watch_events_with_capacity(application, DEFAULT_EVENT_BUFFER_CAPACITY)
            .await
    }

    pub async fn watch_events_with_capacity(
        &self,
        application: &ApplicationRef,
        capacity: usize,
    ) -> Result<(), BackendError> {
        let mut events = self.subscribe_events(application, capacity).await?;
        let mut sequence = 0_u64;
        while let Some(delivery) = events.recv().await {
            match delivery {
                EventDelivery::Event(event) => {
                    sequence = sequence.saturating_add(1);
                    println!("EVENT #{sequence}\n{event}\n");
                }
                EventDelivery::ResyncRequired { dropped } => {
                    println!("EVENT OVERFLOW\ndropped={dropped}\nresync-required=true\n");
                }
            }
        }
        Ok(())
    }

    pub async fn subscribe_events(
        &self,
        application: &ApplicationRef,
        capacity: usize,
    ) -> Result<EventSubscription, BackendError> {
        if capacity == 0 {
            return Err(BackendError::SemanticCache(
                "event buffer capacity must be greater than zero".to_owned(),
            ));
        }
        self.connection
            .register_event::<ObjectEvents>()
            .await
            .map_err(BackendError::EventStream)?;
        self.connection
            .register_event::<WindowEvents>()
            .await
            .map_err(BackendError::EventStream)?;
        self.connection
            .register_event::<CacheEvents>()
            .await
            .map_err(BackendError::EventStream)?;

        let selected_bus = application.backend_locator.bus_name().to_owned();
        let application_locator = application.backend_locator.clone();
        let connection = self.connection.connection().clone();
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        let resync_required = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let notify = Arc::new(tokio::sync::Notify::new());
        let producer_resync = resync_required.clone();
        let producer_dropped = dropped.clone();
        let producer_notify = notify.clone();
        tokio::spawn(async move {
            let events = MessageStream::from(&connection);
            futures_lite::pin!(events);
            while let Some(message) = events.next().await {
                let message = match message {
                    Ok(message) => message,
                    Err(error) => {
                        warn!(%error, "skipping unreadable D-Bus message on AT-SPI event stream");
                        if deliver_event(
                            &sender,
                            crate::events::NormalizedEvent::Unknown {
                                locator: application_locator.clone(),
                                interface: "event-stream".to_owned(),
                                member: error.to_string(),
                            },
                            &producer_resync,
                            &producer_dropped,
                            &producer_notify,
                        )
                        .is_err()
                        {
                            break;
                        }
                        continue;
                    }
                };
                if message.message_type() != MessageType::Signal {
                    continue;
                }
                let header = message.header();
                let Some(message_sender) = header.sender() else {
                    continue;
                };
                if message_sender.as_str() != selected_bus {
                    continue;
                }

                let normalized = match Event::try_from(&message) {
                    Ok(event) => crate::events::NormalizedEvent::from_atspi(&event),
                    Err(error) => match normalize_legacy_property_event(&message) {
                        Some(event) => {
                            warn!(%error, "accepted legacy-compatible AT-SPI PropertyChange event body");
                            event
                        }
                        None => {
                            warn!(%error, "falling back for malformed or unsupported AT-SPI event");
                            let interface = header
                                .interface()
                                .map(|value| value.as_str().to_owned())
                                .unwrap_or_else(|| "<missing>".to_owned());
                            let member = header
                                .member()
                                .map(|value| value.as_str().to_owned())
                                .unwrap_or_else(|| "<missing>".to_owned());
                            let Some(path) = header.path() else {
                                continue;
                            };
                            crate::events::NormalizedEvent::Unknown {
                                locator: BackendLocator::new(
                                    message_sender.as_str(),
                                    path.as_str(),
                                ),
                                interface,
                                member,
                            }
                        }
                    },
                };
                if deliver_event(
                    &sender,
                    normalized,
                    &producer_resync,
                    &producer_dropped,
                    &producer_notify,
                )
                .is_err()
                {
                    break;
                }
            }
        });
        Ok(EventSubscription {
            receiver,
            resync_required,
            dropped,
            notify,
        })
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
        let selected = select_convenience_action(encoded_id, &actions)?;
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

    /// Select a direct accessible child through its parent's Selection interface.
    pub async fn select_child(
        &self,
        parent: &BackendLocator,
        child_index: usize,
    ) -> Result<(), BackendError> {
        let encoded_id = parent.encode();
        let index =
            i32::try_from(child_index).map_err(|_| BackendError::SelectionIndexOutOfRange {
                node_id: encoded_id.clone(),
                index: child_index,
            })?;
        let object = object_ref_from_id(parent)?;
        let proxy = object
            .as_accessible_proxy(self.connection.connection())
            .await
            .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
        let interfaces = dbus_operation(
            self.operation_timeout,
            "read interfaces for selection",
            &encoded_id,
            proxy.get_interfaces(),
        )
        .await?;
        if !interfaces.contains(Interface::Selection) {
            return Err(BackendError::SelectionUnsupported(encoded_id));
        }
        let proxies = atspi_operation(
            self.operation_timeout,
            "create interface proxies for selection",
            &encoded_id,
            proxy.proxies(),
        )
        .await?;
        let selection = atspi_operation(
            self.operation_timeout,
            "create Selection proxy",
            &encoded_id,
            proxies.selection(),
        )
        .await?;
        let accepted = dbus_operation(
            self.operation_timeout,
            "select child",
            &encoded_id,
            selection.select_child(index),
        )
        .await?;
        if !accepted {
            return Err(BackendError::SelectionRejected {
                node_id: encoded_id,
                index: child_index,
            });
        }
        Ok(())
    }

    fn build_node<'a>(
        &'a self,
        object: ObjectRefOwned,
        depth: usize,
        index_in_parent: Option<usize>,
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
                        for (child_index, child) in child_refs.into_iter().enumerate() {
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
                            match self
                                .build_node(child, depth + 1, Some(child_index), context)
                                .await
                            {
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

            let (semantic_role, text_input_kind) = semantic_role_and_input_kind(role, interfaces);
            let capabilities =
                semantic_capabilities(interfaces, &states, semantic_role.clone(), text_input_kind);

            Ok(SemanticNode {
                runtime_id,
                backend_locator: id,
                index_in_parent,
                role: semantic_role,
                name,
                description,
                value,
                text_input_kind,
                states,
                actions,
                capabilities,
                children,
                truncations,
                debug,
            })
        })
    }
}

fn deliver_event(
    sender: &tokio::sync::mpsc::Sender<crate::events::NormalizedEvent>,
    event: crate::events::NormalizedEvent,
    resync_required: &AtomicBool,
    dropped: &AtomicU64,
    notify: &tokio::sync::Notify,
) -> Result<(), ()> {
    match sender.try_send(event) {
        Ok(()) => Ok(()),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            dropped.fetch_add(1, Ordering::Relaxed);
            if !resync_required.swap(true, Ordering::AcqRel) {
                notify.notify_one();
            }
            Ok(())
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err(()),
    }
}

const ENRICHMENT_CONCURRENCY: usize = 32;

async fn enrich_records(
    connection: zbus::Connection,
    timeout: Duration,
    records: Vec<BulkAccessibleRecord>,
) -> (Vec<BulkAccessibleRecord>, usize) {
    let mut pending = records.into_iter();
    let mut tasks = tokio::task::JoinSet::new();
    let mut enriched = Vec::new();
    let mut rpc_count = 0;

    for _ in 0..ENRICHMENT_CONCURRENCY {
        let Some(record) = pending.next() else { break };
        let connection = connection.clone();
        tasks.spawn(enrich_record(connection, timeout, record));
    }
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((record, calls)) => {
                enriched.push(record);
                rpc_count += calls;
            }
            Err(error) => warn!(%error, "selective cache enrichment task failed"),
        }
        if let Some(record) = pending.next() {
            let connection = connection.clone();
            tasks.spawn(enrich_record(connection, timeout, record));
        }
    }
    (enriched, rpc_count)
}

async fn enrich_record(
    connection: zbus::Connection,
    timeout: Duration,
    mut record: BulkAccessibleRecord,
) -> (BulkAccessibleRecord, usize) {
    let mut calls = 0;
    let node_id = record.locator.encode();
    let object = match object_ref_from_id(&record.locator) {
        Ok(object) => object,
        Err(_) => return (record, calls),
    };

    if record.name.is_none() && role_needs_name(record.role) {
        calls += 1;
        if let Ok(proxy) = object.as_accessible_proxy(&connection).await
            && let Ok(Ok(name)) = tokio::time::timeout(timeout, proxy.name()).await
        {
            record.name = nonempty(name);
        }
    }

    if record.interfaces.contains(Interface::Action) && role_needs_actions(record.role) {
        calls += 1;
        if let Ok(proxy) = ActionProxy::builder(&connection)
            .destination(record.locator.bus_name())
            .and_then(|builder| builder.path(record.locator.object_path()))
            && let Ok(Ok(proxy)) = tokio::time::timeout(timeout, proxy.build()).await
            && let Ok(Ok(actions)) = tokio::time::timeout(timeout, proxy.get_actions()).await
        {
            record.actions = map_actions(actions);
        }
    }

    if role_allows_text_value(record.role, record.interfaces)
        && record.interfaces.contains(Interface::Text)
        && let Ok(proxy) = TextProxy::builder(&connection)
            .destination(record.locator.bus_name())
            .and_then(|builder| builder.path(record.locator.object_path()))
        && let Ok(Ok(proxy)) = tokio::time::timeout(timeout, proxy.build()).await
    {
        calls += 1;
        if let Ok(Ok(total_count)) = tokio::time::timeout(timeout, proxy.character_count()).await {
            let count = total_count.clamp(0, 256);
            calls += 1;
            if let Ok(Ok(mut text)) = tokio::time::timeout(timeout, proxy.get_text(0, count)).await
            {
                if total_count > count {
                    text.push('…');
                }
                record.value = nonempty(text);
            }
        }
    }
    tracing::trace!(node_id, calls, "selectively enriched cache record");
    (record, calls)
}

fn role_needs_name(role: Role) -> bool {
    matches!(
        SemanticRole::from(role),
        SemanticRole::Application
            | SemanticRole::Window
            | SemanticRole::Dialog
            | SemanticRole::Label
            | SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::RadioButton
            | SemanticRole::Text
            | SemanticRole::TextInput
            | SemanticRole::ComboBox
            | SemanticRole::Menu
            | SemanticRole::MenuItem
            | SemanticRole::List
            | SemanticRole::ListItem
            | SemanticRole::StatusBar
    )
}

fn role_needs_actions(role: Role) -> bool {
    matches!(
        SemanticRole::from(role),
        SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::ListItem
            | SemanticRole::MenuItem
    )
}

fn semantic_node_count(node: &SemanticNode) -> usize {
    1 + node.children.iter().map(semantic_node_count).sum::<usize>()
}

fn repair_root_relationships(
    records: &mut [BulkAccessibleRecord],
    application: &BackendLocator,
    root_children: &[ObjectRefOwned],
) {
    let indices: std::collections::HashMap<_, _> = root_children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| node_id_from_ref(child).map(|locator| (locator, index)))
        .collect();
    for record in records {
        if let Some(index) = indices.get(&record.locator) {
            record.parent = Some(application.clone());
            record.index_in_parent = Some(*index);
        }
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

async fn fetch_relation_set(
    connection: &zbus::Connection,
    timeout: Duration,
    locator: &BackendLocator,
) -> Result<Vec<BackendRelation>, BackendError> {
    let encoded_id = locator.encode();
    let object = object_ref_from_id(locator)?;
    let proxy = object
        .as_accessible_proxy(connection)
        .await
        .map_err(|error| BackendError::ObjectUnavailable(encoded_id.clone(), error))?;
    let relation_set = dbus_operation(
        timeout,
        "read accessible relation set",
        &encoded_id,
        proxy.get_relation_set(),
    )
    .await?;
    Ok(relation_set
        .into_iter()
        .map(|(kind, targets)| BackendRelation {
            kind: SemanticRelationKind::from(kind),
            targets: targets.iter().filter_map(node_id_from_ref).collect(),
        })
        .collect())
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

fn select_convenience_action(
    node_id: &str,
    actions: &[SemanticAction],
) -> Result<SemanticAction, BackendError> {
    if actions.is_empty() {
        return Err(BackendError::NoActions(node_id.to_owned()));
    }
    ["click", "press", "activate"]
        .iter()
        .find_map(|name| {
            actions
                .iter()
                .find(|action| action.name.eq_ignore_ascii_case(name))
        })
        .cloned()
        .ok_or_else(|| BackendError::NoCompatibleAction {
            node_id: node_id.to_owned(),
            available: format_available_actions(actions),
        })
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

fn semantic_role_and_input_kind(
    role: Role,
    interfaces: atspi::InterfaceSet,
) -> (SemanticRole, Option<TextInputKind>) {
    let semantic_role =
        SemanticRole::from_atspi(role, interfaces.contains(Interface::EditableText));
    let input_kind =
        (semantic_role == SemanticRole::TextInput).then_some(if role == Role::PasswordText {
            TextInputKind::Password
        } else {
            TextInputKind::Plain
        });
    (semantic_role, input_kind)
}

fn semantic_capabilities(
    interfaces: atspi::InterfaceSet,
    states: &[SemanticState],
    role: SemanticRole,
    input_kind: Option<TextInputKind>,
) -> Vec<SemanticCapability> {
    let mut capabilities = Vec::new();
    if interfaces.contains(Interface::Selection) {
        capabilities.push(SemanticCapability::SelectChildren);
    }
    if role == SemanticRole::TextInput
        && input_kind == Some(TextInputKind::Plain)
        && interfaces.contains(Interface::EditableText)
        && interfaces.contains(Interface::Text)
        && states.contains(&SemanticState::Editable)
    {
        capabilities.push(SemanticCapability::EditText);
    }
    capabilities
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

    fn test_event(path: &str) -> crate::events::NormalizedEvent {
        crate::events::NormalizedEvent::NodePropertyChanged {
            locator: BackendLocator::new(":1.2", path),
            property: "accessible-name".to_owned(),
        }
    }

    fn test_subscription(
        capacity: usize,
    ) -> (
        tokio::sync::mpsc::Sender<crate::events::NormalizedEvent>,
        EventSubscription,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
        let resync_required = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let notify = Arc::new(tokio::sync::Notify::new());
        (
            sender,
            EventSubscription {
                receiver,
                resync_required,
                dropped,
                notify,
            },
        )
    }

    fn send_test_event(
        sender: &tokio::sync::mpsc::Sender<crate::events::NormalizedEvent>,
        subscription: &EventSubscription,
        path: &str,
    ) {
        deliver_event(
            sender,
            test_event(path),
            &subscription.resync_required,
            &subscription.dropped,
            &subscription.notify,
        )
        .unwrap();
    }

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
    fn convenience_activate_never_falls_back_to_the_first_action() {
        let error = select_convenience_action("node", &actions(&["delete", "open", "properties"]))
            .unwrap_err();
        assert!(matches!(error, BackendError::NoCompatibleAction { .. }));
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
    fn password_role_and_atspi_sensitive_state_are_independent() {
        let editable_text = atspi::InterfaceSet::new(Interface::EditableText);
        assert_eq!(
            semantic_role_and_input_kind(Role::Text, editable_text),
            (SemanticRole::TextInput, Some(TextInputKind::Plain))
        );
        assert_eq!(
            semantic_role_and_input_kind(Role::PasswordText, editable_text),
            (SemanticRole::TextInput, Some(TextInputKind::Password))
        );
    }

    #[test]
    fn selection_interface_maps_to_container_selection_capability() {
        assert_eq!(
            semantic_capabilities(Interface::Selection.into(), &[], SemanticRole::List, None),
            vec![SemanticCapability::SelectChildren]
        );
        assert!(
            semantic_capabilities(Interface::Action.into(), &[], SemanticRole::Button, None)
                .is_empty()
        );
    }

    #[test]
    fn editable_capability_requires_plain_text_interface_and_editable_state() {
        let mut interfaces = atspi::InterfaceSet::new(Interface::EditableText);
        interfaces.insert(Interface::Text);
        assert_eq!(
            semantic_capabilities(
                interfaces,
                &[SemanticState::Editable],
                SemanticRole::TextInput,
                Some(TextInputKind::Plain)
            ),
            vec![SemanticCapability::EditText]
        );
        assert!(
            semantic_capabilities(
                interfaces,
                &[SemanticState::Editable],
                SemanticRole::TextInput,
                Some(TextInputKind::Password)
            )
            .is_empty()
        );
        assert!(
            semantic_capabilities(
                interfaces,
                &[],
                SemanticRole::TextInput,
                Some(TextInputKind::Plain)
            )
            .is_empty()
        );
    }

    #[test]
    fn application_get_children_repairs_cached_root_relationships() {
        let root = BackendLocator::new(":1.2", "/root");
        let window = ObjectRefOwned::from_static_str_unchecked(":1.2", "/window");
        let mut records = vec![BulkAccessibleRecord {
            locator: BackendLocator::new(":1.2", "/window"),
            application: Some(root.clone()),
            parent: Some(BackendLocator::new(":1.2", "/wrong-parent")),
            index_in_parent: Some(7),
            child_count: Some(0),
            explicit_children: None,
            interfaces: Interface::Accessible.into(),
            name: Some("Window".to_owned()),
            role: Role::Frame,
            description: None,
            states: Default::default(),
            actions: Vec::new(),
            value: None,
        }];
        repair_root_relationships(&mut records, &root, &[window]);
        assert_eq!(records[0].parent.as_ref(), Some(&root));
        assert_eq!(records[0].index_in_parent, Some(0));
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

    #[tokio::test]
    async fn bounded_event_delivery_preserves_normal_events() {
        let (sender, mut subscription) = test_subscription(2);
        send_test_event(&sender, &subscription, "/one");
        assert!(matches!(
            subscription.recv().await,
            Some(EventDelivery::Event(
                crate::events::NormalizedEvent::NodePropertyChanged { locator, .. }
            )) if locator.object_path() == "/one"
        ));
        assert!(subscription.take_resync().is_none());
    }

    #[test]
    fn event_overflow_requests_one_resync_and_counts_all_drops() {
        let (sender, subscription) = test_subscription(1);
        send_test_event(&sender, &subscription, "/queued");
        send_test_event(&sender, &subscription, "/dropped-one");
        send_test_event(&sender, &subscription, "/dropped-two");
        assert!(matches!(
            subscription.take_resync(),
            Some(EventDelivery::ResyncRequired { dropped: 2 })
        ));
        assert!(subscription.take_resync().is_none());

        send_test_event(&sender, &subscription, "/next-flood");
        assert!(matches!(
            subscription.take_resync(),
            Some(EventDelivery::ResyncRequired { dropped: 1 })
        ));
    }

    #[tokio::test]
    async fn consumed_overflow_notification_never_looks_like_stream_closure() {
        let (sender, mut subscription) = test_subscription(1);
        send_test_event(&sender, &subscription, "/queued");
        send_test_event(&sender, &subscription, "/dropped");
        assert!(matches!(
            subscription.take_resync(),
            Some(EventDelivery::ResyncRequired { dropped: 1 })
        ));
        assert!(matches!(
            subscription.recv().await,
            Some(EventDelivery::Event(_))
        ));
    }

    #[test]
    fn bootstrap_events_are_replayable_and_bootstrap_overflow_forces_resync() {
        let (sender, mut subscription) = test_subscription(2);
        send_test_event(&sender, &subscription, "/during-bootstrap");
        assert!(matches!(
            subscription.try_recv(),
            Ok(crate::events::NormalizedEvent::NodePropertyChanged { locator, .. })
                if locator.object_path() == "/during-bootstrap"
        ));

        send_test_event(&sender, &subscription, "/one");
        send_test_event(&sender, &subscription, "/two");
        send_test_event(&sender, &subscription, "/overflow");
        assert!(matches!(
            subscription.take_resync(),
            Some(EventDelivery::ResyncRequired { dropped: 1 })
        ));
    }
}
