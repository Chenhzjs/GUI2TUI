pub mod analyze;
pub mod presentation;
pub mod region;
pub mod scene;

pub use analyze::{RegionAnalysis, RegionMetrics, analyze_regions, format_regions};
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
