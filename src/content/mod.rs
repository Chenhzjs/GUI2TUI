mod cache;
mod model;
mod runtime;
mod search;
mod virtual_collection;

pub use cache::{
    ContentCache, ContentCacheBudget, ContentCacheMetrics, ContentRangeKey, LoadedContentRange,
    TextCursorError, TextRangeCursor,
};
pub use model::{
    ContentBlock, ContentBlockId, ContentBlockKind, ContentCatalog, ContentCompleteness,
    ContentKind, ContentMetadata, ContentNavigationIndex, ContentSummary, OpaqueContentKind,
    SemanticContentModel, TextContentState, format_content_model, format_outline,
};
pub use runtime::{ContentRuntime, MaterializationBudget, MaterializationReport, ReaderBlock};
pub use search::{ContentSearchResult, search_indexed_content};
pub use virtual_collection::{
    VirtualCollectionModel, analyze_virtual_collections, format_virtual_collections,
};
