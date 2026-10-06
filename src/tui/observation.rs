use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use async_trait::async_trait;

use crate::semantic::{BackendLocator, RuntimeNodeId, SemanticCache};

/// The semantic operation that an observer is verifying.  The observer does
/// not infer whether a particular action ought to navigate, toggle, or open a
/// dialog; it only compares the authoritative surface before and after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActuationContext {
    pub target: RuntimeNodeId,
    pub target_locator: BackendLocator,
    pub action: String,
    /// Exact owning Window and Document set captured before delivery. They
    /// bound observation so unrelated application churn cannot confirm an
    /// operation.
    pub owning_window: Option<BackendLocator>,
    pub relevant_documents: Vec<BackendLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotNode {
    role: String,
    name: Option<String>,
    value: Option<String>,
    states: Vec<String>,
    parent: Option<BackendLocator>,
    children: Vec<BackendLocator>,
}

/// A stable, comparison-oriented view of the semantic cache.  Runtime node
/// IDs are intentionally not used for identity here: a full refresh may
/// allocate new IDs while preserving the same public Accessibility locator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceSnapshot {
    pub generation: u64,
    pub root: BackendLocator,
    pub node_count: usize,
    nodes: HashMap<BackendLocator, SnapshotNode>,
    windows: HashMap<BackendLocator, u64>,
    documents: HashMap<BackendLocator, u64>,
}

impl SurfaceSnapshot {
    pub fn from_cache(cache: &SemanticCache) -> Self {
        let mut nodes = HashMap::new();
        for node in cache.nodes() {
            let states = node
                .states
                .iter()
                .filter(|state| **state != crate::semantic::SemanticState::Focused)
                .map(|state| format!("{state:?}"))
                .collect();
            let parent = node
                .parent
                .and_then(|parent| cache.node(parent))
                .map(|parent| parent.backend_locator.clone());
            let children = node
                .children
                .iter()
                .filter_map(|child| cache.node(*child))
                .map(|child| child.backend_locator.clone())
                .collect();
            nodes.insert(
                node.backend_locator.clone(),
                SnapshotNode {
                    role: node.role.to_string(),
                    name: node.name.clone(),
                    value: node.value.clone(),
                    states,
                    parent,
                    children,
                },
            );
        }

        let mut windows = HashMap::new();
        let mut documents = HashMap::new();
        for node in cache.nodes() {
            match node.role {
                crate::semantic::SemanticRole::Window | crate::semantic::SemanticRole::Dialog => {
                    // Window verification is intentionally local to the
                    // window object. Hashing its entire subtree lets an
                    // unrelated clock/status sibling falsely confirm an
                    // operation. Relevant document subtrees are tracked
                    // separately below.
                    windows.insert(
                        node.backend_locator.clone(),
                        node_fingerprint(cache, node.runtime_id, false),
                    );
                }
                crate::semantic::SemanticRole::Document => {
                    documents.insert(
                        node.backend_locator.clone(),
                        subtree_fingerprint(cache, node.runtime_id, false),
                    );
                }
                _ => {}
            }
        }

        let root = cache
            .node(cache.root_id())
            .map(|node| node.backend_locator.clone())
            .unwrap_or_else(|| BackendLocator::new("", "/"));
        Self {
            generation: cache.generation(),
            root,
            node_count: cache.node_count(),
            nodes,
            windows,
            documents,
        }
    }

    pub fn target_is_live(&self, locator: &BackendLocator) -> bool {
        self.nodes.contains_key(locator)
    }

    pub fn context(
        &self,
        target: RuntimeNodeId,
        target_locator: BackendLocator,
        action: impl Into<String>,
    ) -> ActuationContext {
        let owning_window = self.ancestor_in(&target_locator, &self.windows);
        let target_document = self.ancestor_in(&target_locator, &self.documents);
        let relevant_documents = if let Some(document) = target_document {
            vec![document]
        } else if let Some(window) = owning_window.as_ref() {
            self.documents
                .keys()
                .filter(|document| self.is_descendant_of(document, window))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        ActuationContext {
            target,
            target_locator,
            action: action.into(),
            owning_window,
            relevant_documents,
        }
    }

    fn ancestor_in(
        &self,
        locator: &BackendLocator,
        candidates: &HashMap<BackendLocator, u64>,
    ) -> Option<BackendLocator> {
        let mut current = Some(locator.clone());
        for _ in 0..64 {
            let locator = current?;
            if candidates.contains_key(&locator) {
                return Some(locator);
            }
            current = self
                .nodes
                .get(&locator)
                .and_then(|node| node.parent.clone());
        }
        None
    }

    fn is_descendant_of(&self, locator: &BackendLocator, ancestor: &BackendLocator) -> bool {
        let mut current = Some(locator.clone());
        for _ in 0..64 {
            let Some(locator) = current else {
                return false;
            };
            if &locator == ancestor {
                return true;
            }
            current = self
                .nodes
                .get(&locator)
                .and_then(|node| node.parent.clone());
        }
        false
    }

    fn compare(&self, after: &Self, context: &ActuationContext) -> ObservationResult {
        let target_became_stale = !after.target_is_live(&context.target_locator);
        let window_changed = context
            .owning_window
            .as_ref()
            .is_some_and(|window| self.windows.get(window) != after.windows.get(window));
        let before_documents: HashMap<_, _> = context
            .relevant_documents
            .iter()
            .filter_map(|document| self.documents.get(document).map(|value| (document, value)))
            .collect();
        let after_documents: HashMap<_, _> = context
            .owning_window
            .as_ref()
            .map(|window| {
                after
                    .documents
                    .iter()
                    .filter(|(document, _)| after.is_descendant_of(document, window))
                    .collect()
            })
            .unwrap_or_else(|| {
                context
                    .relevant_documents
                    .iter()
                    .filter_map(|document| {
                        after.documents.get(document).map(|value| (document, value))
                    })
                    .collect()
            });
        let document_changed = before_documents != after_documents;

        let before_target = self.nodes.get(&context.target_locator);
        let after_target = after.nodes.get(&context.target_locator);
        let value_changed = matches!((before_target, after_target), (Some(before), Some(after)) if before.value != after.value);
        let state_changed = matches!((before_target, after_target), (Some(before), Some(after)) if before.states != after.states);
        let target_structure_changed = matches!((before_target, after_target), (Some(before), Some(after)) if before.role != after.role || before.name != after.name || before.parent != after.parent || before.children != after.children);
        let structure_changed =
            document_changed || target_structure_changed || self.root != after.root;
        let changed = target_became_stale
            || window_changed
            || document_changed
            || value_changed
            || state_changed
            || structure_changed;
        ObservationResult {
            status: if changed {
                ObservationStatus::Changed
            } else {
                ObservationStatus::NoDetectableChange
            },
            changed,
            target_became_stale,
            window_changed,
            document_changed,
            value_changed,
            state_changed,
            structure_changed,
            before_generation: self.generation,
            after_generation: Some(after.generation),
            polls: 1,
            timed_out: false,
            error: None,
        }
    }
}

fn node_fingerprint(cache: &SemanticCache, id: RuntimeNodeId, include_value: bool) -> u64 {
    let Some(node) = cache.node(id) else {
        return 0;
    };
    let mut hash = 0xcbf29ce484222325_u64;
    let mut add = |value: &str| {
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    };
    add(&node.role.to_string());
    add(node.name.as_deref().unwrap_or(""));
    if include_value {
        add(node.value.as_deref().unwrap_or(""));
    }
    // Window activation and target focus are delivery preparation, not the
    // effect of the requested keyboard/action operation.
    hash
}

fn subtree_fingerprint(cache: &SemanticCache, id: RuntimeNodeId, include_values: bool) -> u64 {
    let Some(node) = cache.node(id) else {
        return 0;
    };
    let mut hash = 0xcbf29ce484222325_u64;
    fn add(hash: &mut u64, value: &str) {
        for byte in value.as_bytes() {
            *hash ^= u64::from(*byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
        *hash ^= 0xff;
        *hash = hash.wrapping_mul(0x100000001b3);
    }
    add(&mut hash, &node.role.to_string());
    add(&mut hash, node.name.as_deref().unwrap_or(""));
    if include_values {
        add(&mut hash, node.value.as_deref().unwrap_or(""));
    }
    for child in &node.children {
        let child_hash = subtree_fingerprint(cache, *child, include_values);
        add(&mut hash, &child_hash.to_string());
    }
    hash
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationStatus {
    Changed,
    NoDetectableChange,
    Timeout,
    RefreshFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationResult {
    pub status: ObservationStatus,
    pub changed: bool,
    pub target_became_stale: bool,
    pub window_changed: bool,
    pub document_changed: bool,
    pub value_changed: bool,
    pub state_changed: bool,
    pub structure_changed: bool,
    pub before_generation: u64,
    pub after_generation: Option<u64>,
    pub polls: u32,
    pub timed_out: bool,
    pub error: Option<String>,
}

#[async_trait(?Send)]
pub trait SurfaceSnapshotRefresher {
    async fn refresh_surface_snapshot(&mut self) -> Result<SurfaceSnapshot, String>;
}

impl ObservationResult {
    fn no_detectable_change(
        before: &SurfaceSnapshot,
        after: Option<&SurfaceSnapshot>,
        polls: u32,
    ) -> Self {
        Self {
            status: ObservationStatus::NoDetectableChange,
            changed: false,
            target_became_stale: false,
            window_changed: false,
            document_changed: false,
            value_changed: false,
            state_changed: false,
            structure_changed: false,
            before_generation: before.generation,
            after_generation: after.map(|snapshot| snapshot.generation),
            polls,
            timed_out: true,
            error: None,
        }
    }

    fn timeout(before: &SurfaceSnapshot, polls: u32, error: Option<String>) -> Self {
        Self {
            status: ObservationStatus::Timeout,
            changed: false,
            target_became_stale: false,
            window_changed: false,
            document_changed: false,
            value_changed: false,
            state_changed: false,
            structure_changed: false,
            before_generation: before.generation,
            after_generation: None,
            polls,
            timed_out: true,
            error,
        }
    }

    fn refresh_failed(before: &SurfaceSnapshot, polls: u32, error: String) -> Self {
        Self {
            status: ObservationStatus::RefreshFailed,
            changed: false,
            target_became_stale: false,
            window_changed: false,
            document_changed: false,
            value_changed: false,
            state_changed: false,
            structure_changed: false,
            before_generation: before.generation,
            after_generation: None,
            polls,
            timed_out: false,
            error: Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationPolicy {
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for ObservationPolicy {
    fn default() -> Self {
        Self {
            // Accessibility providers may publish the post-input subtree
            // after more than one event-loop turn.  Keep this bounded, but
            // leave enough room for a real window/document convergence.
            timeout: Duration::from_millis(3000),
            poll_interval: Duration::from_millis(40),
        }
    }
}

/// Bounded observer for authoritative Surface snapshots.  The refresh
/// refresher is deliberately supplied by the application owner so it can use
/// the normal bootstrap/cache/normalization/presentation pipeline instead of
/// creating a second Accessibility parser.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActuationObserver {
    pub policy: ObservationPolicy,
}

impl ActuationObserver {
    pub fn new(policy: ObservationPolicy) -> Self {
        Self { policy }
    }

    pub async fn observe<R>(
        &self,
        before: &SurfaceSnapshot,
        context: &ActuationContext,
        refresher: &mut R,
    ) -> ObservationResult
    where
        R: SurfaceSnapshotRefresher,
    {
        let started = Instant::now();
        let deadline = started + self.policy.timeout;
        let mut polls = 0_u32;
        let mut last_unchanged: Option<(ObservationResult, SurfaceSnapshot)> = None;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                if let Some((mut result, _)) = last_unchanged.take() {
                    result.status = ObservationStatus::NoDetectableChange;
                    result.timed_out = true;
                    return result;
                }
                return ObservationResult::no_detectable_change(before, None, polls);
            }
            match tokio::time::timeout(remaining, refresher.refresh_surface_snapshot()).await {
                Ok(Ok(after)) => {
                    polls = polls.saturating_add(1);
                    let mut result = before.compare(&after, context);
                    result.polls = polls;
                    if result.changed {
                        return result;
                    }
                    last_unchanged = Some((result, after));
                }
                Ok(Err(error)) => {
                    return ObservationResult::refresh_failed(before, polls, error);
                }
                Err(_) => {
                    return ObservationResult::timeout(
                        before,
                        polls,
                        Some("surface refresh exceeded observation deadline".to_owned()),
                    );
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let mut result = if let Some((result, _)) = last_unchanged.take() {
                    result
                } else {
                    ObservationResult::no_detectable_change(before, None, polls)
                };
                result.status = ObservationStatus::NoDetectableChange;
                result.timed_out = true;
                return result;
            }
            tokio::time::sleep(self.policy.poll_interval.min(remaining)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::semantic::{DebugInfo, SemanticNode, SemanticRole, SemanticState};
    use async_trait::async_trait;

    use super::*;

    fn node(path: &str, role: SemanticRole) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", path),
            index_in_parent: None,
            role,
            name: Some(path.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    fn surface(document: &str, value: Option<&str>, checked: bool) -> SurfaceSnapshot {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        let mut input = node("/input", SemanticRole::TextInput);
        input.value = value.map(str::to_owned);
        input
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        if checked {
            input.states.push(SemanticState::Checked);
        }
        let mut document_node = node(document, SemanticRole::Document);
        document_node.children.push(input);
        window.children.push(document_node);
        root.children.push(window);
        SurfaceSnapshot::from_cache(&SemanticCache::from_snapshot(root).unwrap())
    }

    fn surface_without_target(document: &str) -> SurfaceSnapshot {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        window.children.push(node(document, SemanticRole::Document));
        root.children.push(window);
        SurfaceSnapshot::from_cache(&SemanticCache::from_snapshot(root).unwrap())
    }

    fn surface_with_unrelated_status(status: &str) -> SurfaceSnapshot {
        let mut root = node("/app", SemanticRole::Application);
        let mut window = node("/window", SemanticRole::Window);
        let mut document = node("/document", SemanticRole::Document);
        let mut input = node("/input", SemanticRole::TextInput);
        input.value = Some("query".to_owned());
        input
            .states
            .push(SemanticState::Other("focusable".to_owned()));
        document.children.push(input);
        window.children.push(document);
        let mut unrelated_window = node("/other-window", SemanticRole::Window);
        let mut unrelated = node("/clock", SemanticRole::StatusBar);
        unrelated.value = Some(status.to_owned());
        unrelated_window.children.push(unrelated);
        root.children.extend([window, unrelated_window]);
        SurfaceSnapshot::from_cache(&SemanticCache::from_snapshot(root).unwrap())
    }

    fn context(before: &SurfaceSnapshot) -> ActuationContext {
        before.context(
            RuntimeNodeId::new(1),
            BackendLocator::new(":1.2", "/input"),
            "Submit",
        )
    }

    struct TestRefresher {
        samples: Arc<Mutex<Vec<Result<SurfaceSnapshot, String>>>>,
        fallback: SurfaceSnapshot,
    }

    #[async_trait(?Send)]
    impl SurfaceSnapshotRefresher for TestRefresher {
        async fn refresh_surface_snapshot(&mut self) -> Result<SurfaceSnapshot, String> {
            self.samples
                .lock()
                .unwrap()
                .pop()
                .unwrap_or_else(|| Ok(self.fallback.clone()))
        }
    }

    #[tokio::test]
    async fn immediate_value_change_is_observed() {
        let before = surface("/document-a", Some("a"), false);
        let after = surface("/document-a", Some("b"), false);
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(100),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Ok(after.clone())])),
            fallback: after.clone(),
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert_eq!(result.status, ObservationStatus::Changed);
        assert!(result.value_changed);
    }

    #[tokio::test]
    async fn delayed_document_replacement_is_observed() {
        let before = surface("/document-a", Some("a"), false);
        let unchanged = before.clone();
        let after = surface("/document-b", Some("b"), false);
        let samples = Arc::new(Mutex::new(vec![Ok(after.clone()), Ok(unchanged)]));
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(100),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples,
            fallback: after,
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert_eq!(result.status, ObservationStatus::Changed);
        assert!(result.document_changed);
        assert!(result.structure_changed);
    }

    #[tokio::test]
    async fn stale_target_after_delivery_is_an_observed_change() {
        let before = surface("/document-a", Some("a"), false);
        let after = surface_without_target("/document-b");
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(100),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Ok(after.clone())])),
            fallback: after,
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert!(result.changed);
        assert!(result.target_became_stale);
        assert_ne!(result.status, ObservationStatus::RefreshFailed);
    }

    #[tokio::test]
    async fn state_change_is_distinguished_from_document_replacement() {
        let before = surface("/document-a", Some("a"), false);
        let after = surface("/document-a", Some("a"), true);
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(100),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Ok(after.clone())])),
            fallback: after,
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert!(result.state_changed);
        assert!(!result.document_changed);
    }

    #[tokio::test]
    async fn no_change_is_not_delivery_failure() {
        let before = surface("/document-a", Some("a"), false);
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(8),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Ok(before.clone())])),
            fallback: before.clone(),
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert_eq!(result.status, ObservationStatus::NoDetectableChange);
        assert!(!result.changed);
        assert!(result.timed_out);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn unrelated_application_churn_does_not_confirm_the_operation() {
        let before = surface_with_unrelated_status("10:00");
        let after = surface_with_unrelated_status("10:01");
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(5),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Ok(after.clone())])),
            fallback: after,
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert_eq!(result.status, ObservationStatus::NoDetectableChange);
        assert!(!result.changed);
        assert!(!result.value_changed);
    }

    #[tokio::test]
    async fn refresh_failure_is_reported_separately() {
        let before = surface("/document-a", Some("a"), false);
        let observer = ActuationObserver::new(ObservationPolicy {
            timeout: Duration::from_millis(20),
            poll_interval: Duration::from_millis(1),
        });
        let mut refresher = TestRefresher {
            samples: Arc::new(Mutex::new(vec![Err("backend refresh failed".to_owned())])),
            fallback: before.clone(),
        };
        let result = observer
            .observe(&before, &context(&before), &mut refresher)
            .await;
        assert_eq!(result.status, ObservationStatus::RefreshFailed);
        assert!(!result.changed);
        assert_eq!(result.error.as_deref(), Some("backend refresh failed"));
    }

    #[test]
    fn sibling_name_churn_and_preparation_focus_are_not_keyboard_effects() {
        let make = |clock: &str, focused: bool| {
            let mut root = node("/app", SemanticRole::Application);
            let mut window = node("/window", SemanticRole::Window);
            let mut input = node("/input", SemanticRole::TextInput);
            if focused {
                input.states.push(SemanticState::Focused);
            }
            let mut status = node("/clock", SemanticRole::Label);
            status.name = Some(clock.to_owned());
            window.children.extend([input, status]);
            root.children.push(window);
            SurfaceSnapshot::from_cache(&SemanticCache::from_snapshot(root).unwrap())
        };
        let before = make("10:00", false);
        let after = make("10:01", true);
        let result = before.compare(&after, &context(&before));
        assert!(!result.changed);
        assert!(!result.window_changed);
    }
}
