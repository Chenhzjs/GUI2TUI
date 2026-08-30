pub mod analyze;
pub mod choice;
pub mod command;
pub mod content;
pub mod presentation;
pub mod region;
pub mod scene;
pub mod scope;

pub use analyze::{
    RegionAnalysis, RegionMetrics, analyze_regions, analyze_regions_with_graph, format_regions,
};
pub use choice::{
    ChoiceCatalog, ChoiceOption, ChoiceOptions, ChoiceSelectionStrategy, DisclosureRequirement,
    DismissBehavior, SemanticChoice, format_choices,
};
pub use command::{
    CommandEntry, CommandGroup, CommandHierarchy, RankedCommand, ReachabilityAudit,
    SemanticCommand, UnreachableCommand, format_commands,
};
pub use content::{
    ContentCompressionMetrics, ContentReachabilityAudit, audit_content_reachability,
    compress_content_scene, format_content_reachability,
};
pub use presentation::{
    PresentationMode, PresentationStrategy, compile_legacy_scene, compile_scene,
};
pub use region::{
    ModalityPolicy, RegionConfidence, RegionId, RegionInteraction, SemanticRegion,
    SemanticRegionKind,
};
pub use scene::{
    SceneBinding, SceneElement, SceneElementId, SceneElementKind, SceneMetrics, TuiScene,
    format_scene,
};
pub use scope::{
    InteractionScope, InteractionScopeId, InteractionScopeKind, InteractionScopes, format_scopes,
};
