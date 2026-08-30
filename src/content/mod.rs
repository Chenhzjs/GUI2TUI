mod cache;
mod model;
mod runtime;
mod search;
mod table;
mod virtual_collection;

pub use cache::{
    ContentCache, ContentCacheBudget, ContentCacheMetrics, ContentRangeKey, LoadedContentRange,
    TextCursorError, TextRangeCursor,
};
pub use model::{
    ContentArena, ContentBlock, ContentBlockId, ContentBlockKind, ContentCatalog,
    ContentCompleteness, ContentKind, ContentMetadata, ContentNavigationIndex, ContentScopeClass,
    ContentSummary, OpaqueContentKind, SemanticContentModel, TextContentState,
    format_content_model, format_outline,
};
pub use runtime::{
    ContentPatchMetrics, ContentRuntime, MaterializationBudget, MaterializationPriority,
    MaterializationReport, ReaderBlock, TextCapabilityStatus, TextProbeOutcome,
};
pub use search::{
    ContentSearchResult, ContentSearchSession, SearchBudget, SearchProgress, SearchSessionId,
    SearchState, search_indexed_content,
};
pub use table::{SemanticTableCell, SemanticTableModel, TablePosition};
pub use virtual_collection::{
    VirtualCollectionModel, analyze_virtual_collections, format_virtual_collections,
};
