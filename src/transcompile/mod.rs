pub mod analyze;
pub mod choice;
pub mod command;
pub mod content;
pub mod presentation;
pub mod region;
pub mod scene;
pub mod scope;
pub mod spatial;

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
pub use spatial::{
    CompositionKind, CoordinateSpace, GeometryTrust, InteractionPurpose, LayoutAnalysis,
    LayoutDemand, LayoutImportance, LayoutMetrics, LayoutNode, LayoutReachabilityAudit,
    NormalizedBounds, PresentationCoverageAudit, PresentationObligation, PresentationPriority,
    RegionPresentation, RegionPresentationContext, RegionPresentationKind, ResponsiveComposition,
    SpatialBounds, SpatialEvidence, SpatialEvidenceIndex, SpatialProbeBudget, SpatialProbeMetrics,
    SpatialRegion, SpatialRegionId, SpatialRegionKind, SpatialRelation, SpatialTopology,
    TerminalWidthClass, TopologyRelationKind, TuiLayoutPlan, VisibilityGuarantee,
    audit_layout_reachability, audit_presentation_coverage, format_layout_plan,
    format_presentation_coverage, format_spatial_evidence, infer_layout,
    infer_layout_with_presentations, realize_responsive_layout, refine_layout_demands_from_scene,
    region_focus_order,
};
