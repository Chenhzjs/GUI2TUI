use std::collections::HashSet;

use super::{
    CachedSemanticNode, RelationState, RuntimeNodeId, SemanticCache, SemanticRelation,
    SemanticRelationKind, SemanticRole,
};

pub const LARGE_TREE_RELATION_CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationPriorityReason {
    Background,
    CurrentWindow,
    RelationSensitiveRole,
    VisibleScene,
    ActiveScope,
    Focused,
    OnDemand,
}

#[derive(Clone, Debug, Default)]
pub struct RelationPriorityContext {
    pub focused: Option<RuntimeNodeId>,
    pub active_scope: HashSet<RuntimeNodeId>,
    pub visible_scene: HashSet<RuntimeNodeId>,
    pub current_window: HashSet<RuntimeNodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationCandidate {
    pub runtime_id: RuntimeNodeId,
    pub reason: RelationPriorityReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationSchedule {
    pub budget: usize,
    pub candidates: Vec<RelationCandidate>,
    pub deferred: usize,
}

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

pub fn schedule_relation_candidates(
    cache: &SemanticCache,
    context: &RelationPriorityContext,
    budget: usize,
) -> RelationSchedule {
    let mut candidates: Vec<_> = cache
        .nodes()
        .filter(|node| {
            matches!(
                cache.relation_state(node.runtime_id),
                Some(RelationState::Unknown)
            )
        })
        .map(|node| {
            let reason = if context.focused == Some(node.runtime_id) {
                RelationPriorityReason::Focused
            } else if context.active_scope.contains(&node.runtime_id) {
                RelationPriorityReason::ActiveScope
            } else if context.visible_scene.contains(&node.runtime_id) {
                RelationPriorityReason::VisibleScene
            } else if relation_sensitive_role(&node.role) {
                RelationPriorityReason::RelationSensitiveRole
            } else if context.current_window.contains(&node.runtime_id) {
                RelationPriorityReason::CurrentWindow
            } else {
                RelationPriorityReason::Background
            };
            RelationCandidate {
                runtime_id: node.runtime_id,
                reason,
            }
        })
        .collect();
    // HashMap iteration never determines the result. Equal-priority candidates
    // use the stable session-local ID as a deterministic tie breaker.
    candidates.sort_by(|left, right| {
        right
            .reason
            .cmp(&left.reason)
            .then_with(|| left.runtime_id.cmp(&right.runtime_id))
    });
    let total = candidates.len();
    candidates.truncate(budget.min(total));
    RelationSchedule {
        budget,
        deferred: total.saturating_sub(candidates.len()),
        candidates,
    }
}

pub fn schedule_on_demand_relations(
    cache: &SemanticCache,
    requested: impl IntoIterator<Item = RuntimeNodeId>,
    budget: usize,
) -> RelationSchedule {
    let mut seen = HashSet::new();
    let candidates: Vec<_> = requested
        .into_iter()
        .filter(|id| seen.insert(*id))
        .filter(|id| matches!(cache.relation_state(*id), Some(RelationState::Unknown)))
        .take(budget)
        .map(|runtime_id| RelationCandidate {
            runtime_id,
            reason: RelationPriorityReason::OnDemand,
        })
        .collect();
    RelationSchedule {
        budget,
        deferred: 0,
        candidates,
    }
}

fn relation_sensitive_role(role: &SemanticRole) -> bool {
    matches!(
        role,
        SemanticRole::Dialog
            | SemanticRole::Window
            | SemanticRole::TextInput
            | SemanticRole::RadioButton
            | SemanticRole::ComboBox
            | SemanticRole::Menu
            | SemanticRole::MenuItem
            | SemanticRole::Label
    )
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

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
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

    fn id_named(cache: &SemanticCache, name: &str) -> RuntimeNodeId {
        cache
            .nodes()
            .find(|node| node.name.as_deref() == Some(name))
            .unwrap()
            .runtime_id
    }

    #[test]
    fn priority_context_not_tree_order_selects_the_relation_budget() {
        let mut root = node(1, SemanticRole::Application, "app");
        // Background is deliberately first in traversal order.
        let mut background = node(2, SemanticRole::Window, "background");
        background
            .children
            .push(node(3, SemanticRole::Label, "background label"));
        let mut active = node(4, SemanticRole::Window, "active");
        active
            .children
            .push(node(5, SemanticRole::Button, "visible"));
        active
            .children
            .push(node(6, SemanticRole::TextInput, "focused"));
        root.children = vec![background, active];
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let focused = id_named(&cache, "focused");
        let visible = id_named(&cache, "visible");
        let active_root = id_named(&cache, "active");
        let context = RelationPriorityContext {
            focused: Some(focused),
            active_scope: [active_root].into_iter().collect(),
            visible_scene: [visible].into_iter().collect(),
            current_window: [active_root, visible, focused].into_iter().collect(),
        };
        let schedule = schedule_relation_candidates(&cache, &context, 3);
        assert_eq!(schedule.candidates[0].runtime_id, focused);
        assert_eq!(
            schedule.candidates[0].reason,
            RelationPriorityReason::Focused
        );
        assert_eq!(schedule.candidates[1].runtime_id, active_root);
        assert_eq!(
            schedule.candidates[1].reason,
            RelationPriorityReason::ActiveScope
        );
        assert_eq!(schedule.candidates[2].runtime_id, visible);
        assert_eq!(
            schedule.candidates[2].reason,
            RelationPriorityReason::VisibleScene
        );
        assert!(schedule.deferred > 0);
    }

    #[test]
    fn on_demand_fetch_bypasses_startup_omission_and_dialog_reprioritizes() {
        let mut root = node(1, SemanticRole::Application, "app");
        root.children.push(node(2, SemanticRole::Label, "omitted"));
        root.children.push(node(3, SemanticRole::Dialog, "dialog"));
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let omitted = id_named(&cache, "omitted");
        let dialog = id_named(&cache, "dialog");
        let startup = schedule_relation_candidates(&cache, &RelationPriorityContext::default(), 1);
        assert!(!startup.candidates.is_empty());
        let demand = schedule_on_demand_relations(&cache, [omitted], 1);
        assert_eq!(demand.candidates[0].runtime_id, omitted);
        assert_eq!(
            demand.candidates[0].reason,
            RelationPriorityReason::OnDemand
        );

        let reprioritized = schedule_relation_candidates(
            &cache,
            &RelationPriorityContext {
                active_scope: [dialog].into_iter().collect(),
                ..Default::default()
            },
            1,
        );
        assert_eq!(reprioritized.candidates[0].runtime_id, dialog);
        assert_eq!(
            reprioritized.candidates[0].reason,
            RelationPriorityReason::ActiveScope
        );
    }
}
