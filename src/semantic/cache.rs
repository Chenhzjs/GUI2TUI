use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;

use super::{
    BackendLocator, DebugInfo, RuntimeNodeId, SemanticAction, SemanticCapability, SemanticNode,
    SemanticRole, SemanticState, TextInputKind, TreeTruncation,
};

static NEXT_CACHE_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheNodeContext {
    pub parent: Option<RuntimeNodeId>,
    pub children: Vec<RuntimeNodeId>,
}

/// Canonical arena entry. Relationships use compact runtime IDs; no recursive
/// `SemanticNode` is retained by the live cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedSemanticNode {
    pub runtime_id: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub parent: Option<RuntimeNodeId>,
    pub children: Vec<RuntimeNodeId>,
    pub index_in_parent: Option<usize>,
    pub role: SemanticRole,
    pub name: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub text_input_kind: Option<TextInputKind>,
    pub states: Vec<SemanticState>,
    pub actions: Vec<SemanticAction>,
    pub capabilities: Vec<SemanticCapability>,
    pub truncations: Vec<TreeTruncation>,
    pub debug: DebugInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CacheMutationReport {
    pub subtree_nodes: usize,
    pub locator_reconciled: usize,
    pub reconciled_runtime_ids: Vec<RuntimeNodeId>,
    pub new_runtime_ids: usize,
    pub removed_runtime_ids: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CacheError {
    #[error("semantic cache root is missing")]
    MissingRoot,
    #[error("duplicate RuntimeNodeId {0} in semantic cache")]
    DuplicateRuntimeId(RuntimeNodeId),
    #[error("duplicate BackendLocator {0} in semantic cache")]
    DuplicateLocator(BackendLocator),
    #[error("semantic cache cannot find dirty locator {0}")]
    LocatorNotFound(BackendLocator),
    #[error("semantic cache parent relation is broken for {0}")]
    BrokenParent(RuntimeNodeId),
    #[error("semantic cache child relation is broken: parent {parent}, child {child}")]
    BrokenChild {
        parent: RuntimeNodeId,
        child: RuntimeNodeId,
    },
    #[error("semantic cache contains duplicate child {child} under parent {parent}")]
    DuplicateChild {
        parent: RuntimeNodeId,
        child: RuntimeNodeId,
    },
    #[error("semantic cache contains a parent cycle at {0}")]
    ParentCycle(RuntimeNodeId),
    #[error("semantic cache contains {count} unreachable node(s)")]
    UnreachableNodes { count: usize },
}

/// Single-owner arena with session-stable runtime identities.
#[derive(Clone, Debug)]
pub struct SemanticCache {
    root: RuntimeNodeId,
    nodes: HashMap<RuntimeNodeId, CachedSemanticNode>,
    by_locator: HashMap<BackendLocator, RuntimeNodeId>,
    generation: u64,
    full_snapshot_count: u64,
}

impl SemanticCache {
    pub fn from_snapshot(mut root: SemanticNode) -> Result<Self, CacheError> {
        redact_password_values(&mut root);
        assign_new_ids(&mut root);
        Self::from_assigned_tree(root, 1, 1)
    }

    fn from_assigned_tree(
        root: SemanticNode,
        generation: u64,
        full_snapshot_count: u64,
    ) -> Result<Self, CacheError> {
        let root_id = root.runtime_id;
        let mut cache = Self {
            root: root_id,
            nodes: HashMap::new(),
            by_locator: HashMap::new(),
            generation,
            full_snapshot_count,
        };
        cache.insert_tree(root, None)?;
        cache.validate()?;
        Ok(cache)
    }

    pub fn root_id(&self) -> RuntimeNodeId {
        self.root
    }

    pub fn node(&self, id: RuntimeNodeId) -> Option<&CachedSemanticNode> {
        self.nodes.get(&id)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn full_snapshot_count(&self) -> u64 {
        self.full_snapshot_count
    }

    pub fn runtime_id(&self, locator: &BackendLocator) -> Option<RuntimeNodeId> {
        self.by_locator.get(locator).copied()
    }

    pub fn context(&self, id: RuntimeNodeId) -> Option<CacheNodeContext> {
        self.nodes.get(&id).map(|node| CacheNodeContext {
            parent: node.parent,
            children: node.children.clone(),
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Materialize a recursive presentation tree. The returned tree is derived
    /// and never becomes runtime canonical storage.
    pub fn materialize_tree(&self) -> Result<SemanticNode, CacheError> {
        self.materialize_subtree(self.root)
    }

    pub fn refresh_node(&mut self, mut fresh: SemanticNode) -> Result<RuntimeNodeId, CacheError> {
        redact_password_values(&mut fresh);
        let locator = fresh.backend_locator.clone();
        let id = self
            .runtime_id(&locator)
            .ok_or(CacheError::LocatorNotFound(locator))?;
        let current = self.nodes.get(&id).ok_or(CacheError::MissingRoot)?;
        let parent = current.parent;
        let children = current.children.clone();
        let index_in_parent = current.index_in_parent;
        let replacement = CachedSemanticNode::from_semantic(fresh, parent, children);
        self.nodes.insert(
            id,
            CachedSemanticNode {
                runtime_id: id,
                index_in_parent,
                truncations: Vec::new(),
                ..replacement
            },
        );
        self.generation = self.generation.saturating_add(1);
        self.validate()?;
        Ok(id)
    }

    pub fn replace_subtree(
        &mut self,
        locator: &BackendLocator,
        mut fresh: SemanticNode,
    ) -> Result<CacheMutationReport, CacheError> {
        redact_password_values(&mut fresh);
        let id = self
            .runtime_id(locator)
            .ok_or_else(|| CacheError::LocatorNotFound(locator.clone()))?;
        let old = self.materialize_subtree(id)?;
        let parent = self.nodes.get(&id).ok_or(CacheError::MissingRoot)?.parent;
        let old_by_id = self.subtree_locators(id)?;
        let reconciled = reconcile_subtree(&old, fresh);

        for old_id in self.subtree_ids(id)? {
            if let Some(removed) = self.nodes.remove(&old_id) {
                self.by_locator.remove(&removed.backend_locator);
            }
        }
        self.insert_tree(reconciled, parent)?;
        self.generation = self.generation.saturating_add(1);
        self.validate()?;

        let new_by_id = self.subtree_locators(id)?;
        let reconciled_runtime_ids: Vec<_> = new_by_id
            .iter()
            .filter_map(|(runtime_id, locator)| {
                old_by_id
                    .get(runtime_id)
                    .filter(|old| *old != locator)
                    .map(|_| *runtime_id)
            })
            .collect();
        Ok(CacheMutationReport {
            subtree_nodes: new_by_id.len(),
            locator_reconciled: reconciled_runtime_ids.len(),
            reconciled_runtime_ids,
            new_runtime_ids: new_by_id
                .keys()
                .filter(|runtime_id| !old_by_id.contains_key(runtime_id))
                .count(),
            removed_runtime_ids: old_by_id
                .keys()
                .filter(|runtime_id| !new_by_id.contains_key(runtime_id))
                .count(),
        })
    }

    pub fn full_refresh(&mut self, mut fresh: SemanticNode) -> Result<(), CacheError> {
        redact_password_values(&mut fresh);
        let old = self.materialize_tree()?;
        let reconciled = reconcile_subtree(&old, fresh);
        let generation = self.generation.saturating_add(1);
        let full_snapshot_count = self.full_snapshot_count.saturating_add(1);
        *self = Self::from_assigned_tree(reconciled, generation, full_snapshot_count)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), CacheError> {
        let root = self.nodes.get(&self.root).ok_or(CacheError::MissingRoot)?;
        if root.parent.is_some() {
            return Err(CacheError::BrokenParent(self.root));
        }
        if self.by_locator.len() != self.nodes.len() {
            return Err(CacheError::DuplicateLocator(root.backend_locator.clone()));
        }
        for (id, node) in &self.nodes {
            if self.by_locator.get(&node.backend_locator) != Some(id) {
                return Err(CacheError::DuplicateLocator(node.backend_locator.clone()));
            }
            if let Some(parent) = node.parent {
                let parent_node = self
                    .nodes
                    .get(&parent)
                    .ok_or(CacheError::BrokenParent(*id))?;
                if !parent_node.children.contains(id) {
                    return Err(CacheError::BrokenChild { parent, child: *id });
                }
            }
            let mut unique = HashSet::new();
            for child in &node.children {
                if !unique.insert(*child) {
                    return Err(CacheError::DuplicateChild {
                        parent: *id,
                        child: *child,
                    });
                }
                let child_node = self.nodes.get(child).ok_or(CacheError::BrokenChild {
                    parent: *id,
                    child: *child,
                })?;
                if child_node.parent != Some(*id) {
                    return Err(CacheError::BrokenChild {
                        parent: *id,
                        child: *child,
                    });
                }
            }
        }
        let mut reachable = HashSet::new();
        self.visit_reachable(self.root, &mut reachable)?;
        if reachable.len() != self.nodes.len() {
            return Err(CacheError::UnreachableNodes {
                count: self.nodes.len() - reachable.len(),
            });
        }
        Ok(())
    }

    fn visit_reachable(
        &self,
        id: RuntimeNodeId,
        seen: &mut HashSet<RuntimeNodeId>,
    ) -> Result<(), CacheError> {
        if !seen.insert(id) {
            return Err(CacheError::ParentCycle(id));
        }
        let node = self.nodes.get(&id).ok_or(CacheError::MissingRoot)?;
        for child in &node.children {
            self.visit_reachable(*child, seen)?;
        }
        Ok(())
    }

    fn insert_tree(
        &mut self,
        node: SemanticNode,
        parent: Option<RuntimeNodeId>,
    ) -> Result<(), CacheError> {
        let id = node.runtime_id;
        let child_ids: Vec<_> = node.children.iter().map(|child| child.runtime_id).collect();
        if self.by_locator.contains_key(&node.backend_locator) {
            return Err(CacheError::DuplicateLocator(node.backend_locator));
        }
        if self.nodes.contains_key(&id) {
            return Err(CacheError::DuplicateRuntimeId(id));
        }
        let children = node.children.clone();
        let cached = CachedSemanticNode::from_semantic(node, parent, child_ids);
        self.by_locator.insert(cached.backend_locator.clone(), id);
        self.nodes.insert(id, cached);
        for child in children {
            self.insert_tree(child, Some(id))?;
        }
        Ok(())
    }

    fn materialize_subtree(&self, id: RuntimeNodeId) -> Result<SemanticNode, CacheError> {
        let node = self.nodes.get(&id).ok_or(CacheError::MissingRoot)?;
        let children = node
            .children
            .iter()
            .map(|child| self.materialize_subtree(*child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(node.to_semantic(children))
    }

    fn subtree_ids(&self, id: RuntimeNodeId) -> Result<Vec<RuntimeNodeId>, CacheError> {
        let mut ids = vec![id];
        let node = self.nodes.get(&id).ok_or(CacheError::MissingRoot)?;
        for child in &node.children {
            ids.extend(self.subtree_ids(*child)?);
        }
        Ok(ids)
    }

    fn subtree_locators(
        &self,
        id: RuntimeNodeId,
    ) -> Result<HashMap<RuntimeNodeId, BackendLocator>, CacheError> {
        self.subtree_ids(id)?
            .into_iter()
            .map(|runtime_id| {
                self.nodes
                    .get(&runtime_id)
                    .map(|node| (runtime_id, node.backend_locator.clone()))
                    .ok_or(CacheError::MissingRoot)
            })
            .collect()
    }
}

impl CachedSemanticNode {
    fn from_semantic(
        mut node: SemanticNode,
        parent: Option<RuntimeNodeId>,
        children: Vec<RuntimeNodeId>,
    ) -> Self {
        if node.text_input_kind == Some(TextInputKind::Password) {
            node.value = None;
        }
        Self {
            runtime_id: node.runtime_id,
            backend_locator: node.backend_locator,
            parent,
            children,
            index_in_parent: node.index_in_parent,
            role: node.role,
            name: node.name,
            description: node.description,
            value: node.value,
            text_input_kind: node.text_input_kind,
            states: node.states,
            actions: node.actions,
            capabilities: node.capabilities,
            truncations: node.truncations,
            debug: node.debug,
        }
    }

    fn to_semantic(&self, children: Vec<SemanticNode>) -> SemanticNode {
        SemanticNode {
            runtime_id: self.runtime_id,
            backend_locator: self.backend_locator.clone(),
            index_in_parent: self.index_in_parent,
            role: self.role.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            value: self.value.clone(),
            text_input_kind: self.text_input_kind,
            states: self.states.clone(),
            actions: self.actions.clone(),
            capabilities: self.capabilities.clone(),
            children,
            truncations: self.truncations.clone(),
            debug: self.debug.clone(),
        }
    }
}

fn allocate_id() -> RuntimeNodeId {
    RuntimeNodeId::new(NEXT_CACHE_RUNTIME_ID.fetch_add(1, Ordering::Relaxed))
}

fn assign_new_ids(node: &mut SemanticNode) {
    node.runtime_id = allocate_id();
    for child in &mut node.children {
        assign_new_ids(child);
    }
}

fn redact_password_values(node: &mut SemanticNode) {
    if node.text_input_kind == Some(TextInputKind::Password) {
        node.value = None;
    }
    for child in &mut node.children {
        redact_password_values(child);
    }
}

fn fingerprint(node: &SemanticNode) -> (String, Option<String>, Option<TextInputKind>) {
    (
        node.role.to_string(),
        node.name.clone(),
        node.text_input_kind,
    )
}

fn reconcile_subtree(old: &SemanticNode, mut fresh: SemanticNode) -> SemanticNode {
    fresh.runtime_id = old.runtime_id;
    let old_children = &old.children;
    let fresh_copy = fresh.children.clone();
    let mut used = HashSet::new();
    let mut children = Vec::with_capacity(fresh.children.len());
    for mut child in std::mem::take(&mut fresh.children) {
        let exact = old_children.iter().enumerate().find(|(index, candidate)| {
            !used.contains(index) && candidate.backend_locator == child.backend_locator
        });
        let matched = exact.or_else(|| {
            let hint = fingerprint(&child);
            let old_matches: Vec<_> = old_children
                .iter()
                .enumerate()
                .filter(|(index, candidate)| {
                    !used.contains(index) && fingerprint(candidate) == hint
                })
                .collect();
            let new_count = fresh_copy
                .iter()
                .filter(|candidate| fingerprint(candidate) == hint)
                .count();
            if old_matches.len() == 1 && new_count == 1 {
                Some(old_matches[0])
            } else {
                None
            }
        });
        if let Some((index, candidate)) = matched {
            used.insert(index);
            child = reconcile_subtree(candidate, child);
        } else {
            assign_new_ids(&mut child);
        }
        children.push(child);
    }
    fresh.children = children;
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(path: &str, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", path),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
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

    fn tree(child_paths: &[(&str, &str)]) -> SemanticNode {
        let mut root = node("/root", SemanticRole::Application, "App");
        root.children = child_paths
            .iter()
            .enumerate()
            .map(|(index, (path, name))| {
                let mut child = node(path, SemanticRole::Button, name);
                child.index_in_parent = Some(index);
                child
            })
            .collect();
        root
    }

    #[test]
    fn arena_has_constant_time_id_locator_and_relationship_lookups() {
        let cache = SemanticCache::from_snapshot(tree(&[("/save", "Save")])).unwrap();
        let locator = BackendLocator::new(":1.2", "/save");
        let id = cache.runtime_id(&locator).unwrap();
        assert_eq!(cache.node(id).unwrap().backend_locator, locator);
        assert_eq!(cache.node(id).unwrap().parent, Some(cache.root_id()));
        assert_eq!(cache.node(cache.root_id()).unwrap().children, vec![id]);
    }

    #[test]
    fn same_locator_keeps_runtime_id_during_node_refresh() {
        let mut cache = SemanticCache::from_snapshot(tree(&[("/save", "Save")])).unwrap();
        let locator = BackendLocator::new(":1.2", "/save");
        let before = cache.runtime_id(&locator).unwrap();
        let mut fresh = node("/save", SemanticRole::Button, "Save now");
        fresh.description = Some("updated".to_owned());
        cache.refresh_node(fresh).unwrap();
        assert_eq!(cache.runtime_id(&locator), Some(before));
        assert_eq!(
            cache.node(before).unwrap().name.as_deref(),
            Some("Save now")
        );
    }

    #[test]
    fn unique_structural_churn_keeps_runtime_id_and_updates_locator() {
        let mut cache = SemanticCache::from_snapshot(tree(&[("/270", "Churn target")])).unwrap();
        let old = BackendLocator::new(":1.2", "/270");
        let before = cache.runtime_id(&old).unwrap();
        cache
            .replace_subtree(
                &BackendLocator::new(":1.2", "/root"),
                tree(&[("/278", "Churn target")]),
            )
            .unwrap();
        assert_eq!(
            cache.runtime_id(&BackendLocator::new(":1.2", "/278")),
            Some(before)
        );
        assert_eq!(cache.runtime_id(&old), None);
    }

    #[test]
    fn ambiguous_structural_churn_never_guesses_identity() {
        let mut cache =
            SemanticCache::from_snapshot(tree(&[("/old-a", "Duplicate"), ("/old-b", "Duplicate")]))
                .unwrap();
        let old_ids: HashSet<_> = ["/old-a", "/old-b"]
            .into_iter()
            .map(|path| {
                cache
                    .runtime_id(&BackendLocator::new(":1.2", path))
                    .unwrap()
            })
            .collect();
        cache
            .replace_subtree(
                &BackendLocator::new(":1.2", "/root"),
                tree(&[("/new-a", "Duplicate"), ("/new-b", "Duplicate")]),
            )
            .unwrap();
        for path in ["/new-a", "/new-b"] {
            assert!(
                !old_ids.contains(
                    &cache
                        .runtime_id(&BackendLocator::new(":1.2", path))
                        .unwrap()
                )
            );
        }
    }

    #[test]
    fn application_restart_creates_a_new_runtime_identity_session() {
        let first = SemanticCache::from_snapshot(tree(&[("/save", "Save")])).unwrap();
        let second = SemanticCache::from_snapshot(tree(&[("/save", "Save")])).unwrap();
        assert_ne!(first.root_id(), second.root_id());
    }

    #[test]
    fn subtree_add_and_remove_leaves_no_unreachable_nodes() {
        let mut cache = SemanticCache::from_snapshot(tree(&[("/a", "A")])).unwrap();
        cache
            .replace_subtree(
                &BackendLocator::new(":1.2", "/root"),
                tree(&[("/a", "A"), ("/b", "B")]),
            )
            .unwrap();
        assert_eq!(cache.node_count(), 3);
        cache
            .replace_subtree(&BackendLocator::new(":1.2", "/root"), tree(&[("/b", "B")]))
            .unwrap();
        assert_eq!(cache.node_count(), 2);
        assert!(
            cache
                .runtime_id(&BackendLocator::new(":1.2", "/a"))
                .is_none()
        );
        cache.validate().unwrap();
    }

    #[test]
    fn duplicate_locator_is_rejected() {
        assert!(matches!(
            SemanticCache::from_snapshot(tree(&[("/same", "A"), ("/same", "B")])),
            Err(CacheError::DuplicateLocator(_))
        ));
    }

    #[test]
    fn invariant_detects_broken_parent_duplicate_child_cycle_and_unreachable() {
        let cache = SemanticCache::from_snapshot(tree(&[("/a", "A")])).unwrap();
        let child = cache
            .runtime_id(&BackendLocator::new(":1.2", "/a"))
            .unwrap();
        let mut broken = cache.clone();
        broken.nodes.get_mut(&child).unwrap().parent = Some(RuntimeNodeId::new(u64::MAX));
        assert!(
            matches!(
                broken.validate(),
                Err(CacheError::BrokenParent(id)) if id == child
            ) || matches!(broken.validate(), Err(CacheError::BrokenChild { child: id, .. }) if id == child)
        );
        let mut duplicate = cache.clone();
        duplicate
            .nodes
            .get_mut(&duplicate.root)
            .unwrap()
            .children
            .push(child);
        assert!(matches!(
            duplicate.validate(),
            Err(CacheError::DuplicateChild { .. })
        ));
        let mut cycle = cache.clone();
        cycle
            .nodes
            .get_mut(&child)
            .unwrap()
            .children
            .push(cycle.root);
        cycle.nodes.get_mut(&cycle.root).unwrap().parent = Some(child);
        assert!(matches!(
            cycle.validate(),
            Err(CacheError::BrokenParent(_) | CacheError::ParentCycle(_))
        ));
        let mut unreachable = cache.clone();
        let unreachable_id = RuntimeNodeId::new(u64::MAX - 1);
        let mut detached = unreachable.nodes.get(&child).unwrap().clone();
        detached.runtime_id = unreachable_id;
        detached.backend_locator = BackendLocator::new(":1.2", "/detached");
        detached.parent = Some(unreachable_id);
        detached.children = vec![unreachable_id];
        unreachable
            .by_locator
            .insert(detached.backend_locator.clone(), unreachable_id);
        unreachable.nodes.insert(unreachable_id, detached);
        assert!(matches!(
            unreachable.validate(),
            Err(CacheError::UnreachableNodes { count: 1 })
        ));
    }

    #[test]
    fn incremental_password_refresh_drops_value_before_caching() {
        let mut password = node("/password", SemanticRole::TextInput, "Password");
        password.text_input_kind = Some(TextInputKind::Password);
        let mut root = node("/root", SemanticRole::Application, "App");
        root.children.push(password);
        let mut cache = SemanticCache::from_snapshot(root).unwrap();
        let password_id = cache
            .runtime_id(&BackendLocator::new(":1.2", "/password"))
            .unwrap();
        let mut refresh = node("/password", SemanticRole::TextInput, "Password");
        refresh.text_input_kind = Some(TextInputKind::Password);
        refresh.value = Some("incremental-secret-sentinel".to_owned());
        cache.refresh_node(refresh).unwrap();
        assert_eq!(cache.node(password_id).unwrap().value, None);
    }

    #[test]
    fn recursive_tree_is_a_derived_projection() {
        let cache = SemanticCache::from_snapshot(tree(&[("/a", "A"), ("/b", "B")])).unwrap();
        let projected = cache.materialize_tree().unwrap();
        assert_eq!(projected.children.len(), 2);
        assert_eq!(projected.children[1].name.as_deref(), Some("B"));
    }
}
