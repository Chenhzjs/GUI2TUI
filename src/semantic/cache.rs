use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
};

use thiserror::Error;

use super::{BackendLocator, RuntimeNodeId, SemanticNode, TextInputKind};

static NEXT_CACHE_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheNodeContext {
    pub parent: Option<RuntimeNodeId>,
    pub children: Vec<RuntimeNodeId>,
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
    #[error("semantic cache contains a parent cycle at {0}")]
    ParentCycle(RuntimeNodeId),
}

/// Single-owner live semantic tree with session-stable runtime identities.
#[derive(Clone, Debug)]
pub struct SemanticCache {
    root: SemanticNode,
    by_locator: HashMap<BackendLocator, RuntimeNodeId>,
    contexts: HashMap<RuntimeNodeId, CacheNodeContext>,
    generation: u64,
    full_snapshot_count: u64,
}

impl SemanticCache {
    pub fn from_snapshot(mut root: SemanticNode) -> Result<Self, CacheError> {
        redact_password_values(&mut root);
        assign_new_ids(&mut root);
        let mut cache = Self {
            root,
            by_locator: HashMap::new(),
            contexts: HashMap::new(),
            generation: 1,
            full_snapshot_count: 1,
        };
        cache.reindex()?;
        Ok(cache)
    }

    pub fn root(&self) -> &SemanticNode {
        &self.root
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

    pub fn context(&self, id: RuntimeNodeId) -> Option<&CacheNodeContext> {
        self.contexts.get(&id)
    }

    pub fn node_count(&self) -> usize {
        self.by_locator.len()
    }

    pub fn refresh_node(&mut self, mut fresh: SemanticNode) -> Result<RuntimeNodeId, CacheError> {
        redact_password_values(&mut fresh);
        let locator = fresh.backend_locator.clone();
        let id = self
            .runtime_id(&locator)
            .ok_or_else(|| CacheError::LocatorNotFound(locator.clone()))?;
        let target = find_mut(&mut self.root, id).ok_or(CacheError::MissingRoot)?;
        let children = std::mem::take(&mut target.children);
        let runtime_id = target.runtime_id;
        let index_in_parent = target.index_in_parent;
        *target = fresh;
        target.runtime_id = runtime_id;
        target.index_in_parent = index_in_parent;
        target.children = children;
        target.truncations.clear();
        self.generation = self.generation.saturating_add(1);
        self.reindex()?;
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
        let old_by_id: HashMap<_, _> = self
            .by_locator
            .iter()
            .map(|(locator, id)| (*id, locator.clone()))
            .collect();
        replace_at(&mut self.root, id, fresh).ok_or(CacheError::MissingRoot)?;
        self.generation = self.generation.saturating_add(1);
        self.reindex()?;
        let new_by_id: HashMap<_, _> = self
            .by_locator
            .iter()
            .map(|(locator, id)| (*id, locator.clone()))
            .collect();
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
            subtree_nodes: subtree_size_by_id(&self.root, id),
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
        self.root = reconcile_subtree(&self.root, fresh);
        self.generation = self.generation.saturating_add(1);
        self.full_snapshot_count = self.full_snapshot_count.saturating_add(1);
        self.reindex()
    }

    pub fn validate(&self) -> Result<(), CacheError> {
        let mut ids = HashSet::new();
        let mut locators = HashSet::new();
        validate_node(&self.root, &mut ids, &mut locators)?;
        if ids.is_empty() {
            return Err(CacheError::MissingRoot);
        }
        for (id, context) in &self.contexts {
            let mut seen = HashSet::new();
            let mut cursor = Some(*id);
            while let Some(current) = cursor {
                if !seen.insert(current) {
                    return Err(CacheError::ParentCycle(current));
                }
                cursor = self.contexts.get(&current).and_then(|entry| entry.parent);
            }
            if let Some(parent) = context.parent
                && !self.contexts.contains_key(&parent)
            {
                return Err(CacheError::BrokenParent(*id));
            }
        }
        Ok(())
    }

    fn reindex(&mut self) -> Result<(), CacheError> {
        let mut by_locator = HashMap::new();
        let mut contexts = HashMap::new();
        index_node(&self.root, None, &mut by_locator, &mut contexts)?;
        self.by_locator = by_locator;
        self.contexts = contexts;
        self.validate()
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

fn replace_at(node: &mut SemanticNode, target: RuntimeNodeId, fresh: SemanticNode) -> Option<()> {
    if node.runtime_id == target {
        *node = reconcile_subtree(node, fresh);
        return Some(());
    }
    node.children
        .iter_mut()
        .find_map(|child| replace_at(child, target, fresh.clone()))
}

fn find_mut(node: &mut SemanticNode, id: RuntimeNodeId) -> Option<&mut SemanticNode> {
    if node.runtime_id == id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_mut(child, id))
}

fn subtree_size_by_id(node: &SemanticNode, id: RuntimeNodeId) -> usize {
    if node.runtime_id == id {
        return count_nodes(node);
    }
    node.children
        .iter()
        .map(|child| subtree_size_by_id(child, id))
        .find(|count| *count > 0)
        .unwrap_or(0)
}

fn count_nodes(node: &SemanticNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

fn index_node(
    node: &SemanticNode,
    parent: Option<RuntimeNodeId>,
    by_locator: &mut HashMap<BackendLocator, RuntimeNodeId>,
    contexts: &mut HashMap<RuntimeNodeId, CacheNodeContext>,
) -> Result<(), CacheError> {
    if by_locator
        .insert(node.backend_locator.clone(), node.runtime_id)
        .is_some()
    {
        return Err(CacheError::DuplicateLocator(node.backend_locator.clone()));
    }
    if contexts
        .insert(
            node.runtime_id,
            CacheNodeContext {
                parent,
                children: node.children.iter().map(|child| child.runtime_id).collect(),
            },
        )
        .is_some()
    {
        return Err(CacheError::DuplicateRuntimeId(node.runtime_id));
    }
    for child in &node.children {
        index_node(child, Some(node.runtime_id), by_locator, contexts)?;
    }
    Ok(())
}

fn validate_node(
    node: &SemanticNode,
    ids: &mut HashSet<RuntimeNodeId>,
    locators: &mut HashSet<BackendLocator>,
) -> Result<(), CacheError> {
    if !ids.insert(node.runtime_id) {
        return Err(CacheError::DuplicateRuntimeId(node.runtime_id));
    }
    if !locators.insert(node.backend_locator.clone()) {
        return Err(CacheError::DuplicateLocator(node.backend_locator.clone()));
    }
    for child in &node.children {
        validate_node(child, ids, locators)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{DebugInfo, SemanticRole};

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
            .map(|(path, name)| node(path, SemanticRole::Button, name))
            .collect();
        root
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
        assert_ne!(first.root().runtime_id, second.root().runtime_id);
    }

    #[test]
    fn subtree_add_and_remove_reindexes_children_and_stale_nodes() {
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
        let result = SemanticCache::from_snapshot(tree(&[("/same", "A"), ("/same", "B")]));
        assert!(matches!(result, Err(CacheError::DuplicateLocator(_))));
    }

    #[test]
    fn broken_parent_invariant_is_detected() {
        let mut cache = SemanticCache::from_snapshot(tree(&[("/a", "A")])).unwrap();
        let child = cache
            .runtime_id(&BackendLocator::new(":1.2", "/a"))
            .unwrap();
        cache.contexts.get_mut(&child).unwrap().parent = Some(RuntimeNodeId::new(u64::MAX));
        assert!(matches!(cache.validate(), Err(CacheError::BrokenParent(id)) if id == child));
    }

    #[test]
    fn parent_cycle_invariant_is_detected() {
        let mut cache = SemanticCache::from_snapshot(tree(&[("/a", "A")])).unwrap();
        let root = cache.root.runtime_id;
        let child = cache
            .runtime_id(&BackendLocator::new(":1.2", "/a"))
            .unwrap();
        cache.contexts.get_mut(&root).unwrap().parent = Some(child);
        assert!(matches!(cache.validate(), Err(CacheError::ParentCycle(_))));
    }

    #[test]
    fn duplicate_runtime_identity_is_rejected() {
        let mut root = tree(&[("/a", "A"), ("/b", "B")]);
        root.runtime_id = RuntimeNodeId::new(10);
        root.children[0].runtime_id = RuntimeNodeId::new(11);
        root.children[1].runtime_id = RuntimeNodeId::new(11);
        let mut ids = HashSet::new();
        let mut locators = HashSet::new();
        assert!(matches!(
            validate_node(&root, &mut ids, &mut locators),
            Err(CacheError::DuplicateRuntimeId(_))
        ));
    }

    #[test]
    fn incremental_password_refresh_drops_value_before_caching() {
        let mut password = node("/password", SemanticRole::TextInput, "Password");
        password.text_input_kind = Some(TextInputKind::Password);
        let mut root = node("/root", SemanticRole::Application, "App");
        root.children.push(password);
        let mut cache = SemanticCache::from_snapshot(root).unwrap();

        let mut refresh = node("/password", SemanticRole::TextInput, "Password");
        refresh.text_input_kind = Some(TextInputKind::Password);
        refresh.value = Some("incremental-secret-sentinel".to_owned());
        cache.refresh_node(refresh).unwrap();

        assert_eq!(cache.root().children[0].value, None);
    }
}
