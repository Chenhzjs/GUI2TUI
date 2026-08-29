use std::collections::HashSet;

use super::{
    CachedSemanticNode, RelationState, RuntimeNodeId, SemanticCache, SemanticRelation,
    SemanticRelationKind, SemanticRole,
};

pub const LARGE_TREE_RELATION_CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionCompleteness {
    Complete,
    PartialRealized,
    Unknown,
}

pub fn collection_completeness(node: &CachedSemanticNode) -> CollectionCompleteness {
    if node.states.iter().any(
        |state| matches!(state, super::SemanticState::Other(value) if value == "manages-descendants"),
    ) {
        CollectionCompleteness::PartialRealized
    } else if matches!(
        node.role,
        SemanticRole::List | SemanticRole::Menu | SemanticRole::TabList
    ) {
        CollectionCompleteness::Complete
    } else if matches!(node.role, SemanticRole::Tree | SemanticRole::Table) {
        CollectionCompleteness::Unknown
    } else {
        CollectionCompleteness::Complete
    }
}

pub struct RelationalSemanticGraph<'a> {
    cache: &'a SemanticCache,
}

impl<'a> RelationalSemanticGraph<'a> {
    pub fn new(cache: &'a SemanticCache) -> Self {
        Self { cache }
    }

    pub fn relations(&self, id: RuntimeNodeId) -> Option<&[SemanticRelation]> {
        match self.cache.relation_state(id)? {
            RelationState::Known(relations) => Some(relations),
            RelationState::Unknown | RelationState::Unavailable => None,
        }
    }

    pub fn node(&self, id: RuntimeNodeId) -> Option<&CachedSemanticNode> {
        self.cache.node(id)
    }

    pub fn targets(&self, id: RuntimeNodeId, kind: SemanticRelationKind) -> Vec<RuntimeNodeId> {
        self.relations(id)
            .into_iter()
            .flatten()
            .filter(|relation| relation.kind == kind)
            .flat_map(|relation| relation.targets.iter())
            .filter_map(|target| target.runtime_id)
            .collect()
    }

    pub fn labels_for(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::LabelledBy)
    }

    pub fn descriptions_for(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::DescribedBy)
    }

    pub fn errors_for(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::ErrorMessage)
    }

    pub fn memberships_of(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::MemberOf)
    }

    pub fn controlled_nodes(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::ControllerFor)
    }

    pub fn controllers_for(&self, id: RuntimeNodeId) -> Vec<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::ControlledBy)
    }

    pub fn popup_owner(&self, id: RuntimeNodeId) -> Option<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::PopupFor)
            .into_iter()
            .next()
    }

    pub fn parent_window(&self, id: RuntimeNodeId) -> Option<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::SubwindowOf)
            .into_iter()
            .next()
    }

    pub fn logical_next(&self, id: RuntimeNodeId) -> Option<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::FlowsTo)
            .into_iter()
            .next()
    }

    pub fn logical_previous(&self, id: RuntimeNodeId) -> Option<RuntimeNodeId> {
        self.targets(id, SemanticRelationKind::FlowsFrom)
            .into_iter()
            .next()
    }
}

pub fn targeted_relation_candidates(
    cache: &SemanticCache,
    scene_sources: impl IntoIterator<Item = RuntimeNodeId>,
) -> Vec<RuntimeNodeId> {
    let limit = if cache.node_count() <= 512 {
        cache.node_count()
    } else {
        LARGE_TREE_RELATION_CANDIDATE_LIMIT
    };
    let mut output = Vec::with_capacity(limit.min(cache.node_count()));
    let mut seen = HashSet::new();
    for id in scene_sources {
        if output.len() < limit && cache.node(id).is_some() && seen.insert(id) {
            output.push(id);
        }
    }
    let priorities = [
        SemanticRole::Dialog,
        SemanticRole::Window,
        SemanticRole::TextInput,
        SemanticRole::RadioButton,
        SemanticRole::ComboBox,
        SemanticRole::Menu,
        SemanticRole::MenuItem,
        SemanticRole::Label,
    ];
    for role in priorities {
        for node in cache.nodes().filter(|node| node.role == role) {
            if output.len() == limit {
                return output;
            }
            if seen.insert(node.runtime_id) {
                output.push(node.runtime_id);
            }
        }
    }
    output
}

pub fn format_relations(cache: &SemanticCache, only: Option<RuntimeNodeId>) -> String {
    let mut nodes: Vec<_> = cache
        .nodes()
        .filter(|node| only.is_none_or(|id| node.runtime_id == id))
        .collect();
    nodes.sort_by_key(|node| node.runtime_id);
    let mut output = String::new();
    for node in nodes {
        let Some(state) = cache.relation_state(node.runtime_id) else {
            continue;
        };
        if matches!(state, RelationState::Unknown) && only.is_none() {
            continue;
        }
        if matches!(state, RelationState::Known(relations) if relations.is_empty())
            && only.is_none()
        {
            continue;
        }
        output.push_str(&format!(
            "{} {:?} [RuntimeNodeId={}] locator={}\n",
            node.role, node.name, node.runtime_id, node.backend_locator
        ));
        match state {
            RelationState::Unknown => output.push_str("  Relations: UNKNOWN (not enriched)\n"),
            RelationState::Unavailable => output.push_str("  Relations: UNAVAILABLE\n"),
            RelationState::Known(relations) if relations.is_empty() => {
                output.push_str("  Relations: none exposed\n")
            }
            RelationState::Known(relations) => {
                for relation in relations {
                    output.push_str(&format!("  {}:\n", relation.kind));
                    for target in &relation.targets {
                        if let Some(id) = target.runtime_id
                            && let Some(target_node) = cache.node(id)
                        {
                            output.push_str(&format!(
                                "    -> {} {:?} [{}]\n",
                                target_node.role, target_node.name, id
                            ));
                        } else {
                            output.push_str(&format!("    -> unresolved {}\n", target.locator));
                        }
                    }
                }
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode, SemanticState};

    use super::*;

    #[test]
    fn manages_descendants_never_claims_complete_collection() {
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/list"),
            index_in_parent: None,
            role: SemanticRole::List,
            name: None,
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![SemanticState::Other("manages-descendants".to_owned())],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        };
        let cache = SemanticCache::from_snapshot(root).unwrap();
        assert_eq!(
            collection_completeness(cache.node(cache.root_id()).unwrap()),
            CollectionCompleteness::PartialRealized
        );
    }
}
