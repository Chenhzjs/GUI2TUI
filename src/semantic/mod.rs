pub mod cache;
pub mod graph;
mod node;
mod relation;
mod snapshot;

pub use cache::{
    CacheError, CacheMutationReport, CacheNodeContext, CachedSemanticNode, SelectionRead,
    SemanticCache,
};
pub use graph::{
    CollectionCompleteness, LARGE_TREE_RELATION_CANDIDATE_LIMIT, RelationCandidate,
    RelationPriorityContext, RelationPriorityReason, RelationSchedule, RelationalSemanticGraph,
    collection_completeness, format_relations, schedule_on_demand_relations,
    schedule_relation_candidates, targeted_relation_candidates,
};
pub use node::{
    BackendLocator, BackendLocatorError, DebugInfo, Geometry, RuntimeNodeId, SemanticAction,
    SemanticCapability, SemanticNode, SemanticRole, SemanticState, TextInputKind, TreeTruncation,
};
pub use relation::{
    BackendRelation, RelationState, SemanticRelation, SemanticRelationKind, SemanticRelationTarget,
};
pub(crate) use snapshot::RuntimeIdAllocator;

/// Preview read provenance. An absent value is never proof of an empty field.
pub fn value_read_status(
    value: Option<&str>,
    kind: Option<TextInputKind>,
    interfaces: &[String],
    truncations: &[TreeTruncation],
) -> &'static str {
    if kind == Some(TextInputKind::Password) {
        return "redacted";
    }
    if truncations.iter().any(|t| {
        matches!(
            t,
            TreeTruncation::ReadFailure {
                operation: "read value",
                ..
            } | TreeTruncation::OperationTimeout {
                operation: "read value",
                ..
            }
        )
    }) {
        return "unavailable";
    }
    if let Some(value) = value {
        return if value.is_empty() {
            "known_empty"
        } else {
            "known"
        };
    }
    if interfaces.iter().any(|i| i == "Value")
        || (interfaces.iter().any(|i| i == "Text")
            && interfaces.iter().any(|i| i == "EditableText"))
    {
        "not_requested"
    } else {
        "not_exposed"
    }
}
