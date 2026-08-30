use std::collections::HashMap;

use crate::semantic::RuntimeNodeId;

use super::{ContentBlockId, ContentCache, SemanticContentModel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentSearchResult {
    pub block_id: ContentBlockId,
    pub source: RuntimeNodeId,
    pub range: Option<(usize, usize)>,
    pub preview: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SearchSessionId(u64);

impl SearchSessionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchState {
    Running,
    Cancelled,
    Complete,
    Failed(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchProgress {
    pub scanned_blocks: usize,
    pub total_blocks: Option<usize>,
    pub text_rpcs: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchBudget {
    pub blocks_per_tick: usize,
    pub text_rpcs_per_tick: usize,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            blocks_per_tick: 4,
            text_rpcs_per_tick: 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContentSearchSession {
    pub id: SearchSessionId,
    pub root: RuntimeNodeId,
    pub query: String,
    pub state: SearchState,
    pub cursor: usize,
    pub order: Vec<ContentBlockId>,
    pub results: Vec<ContentSearchResult>,
    pub progress: SearchProgress,
    pub source_offsets: HashMap<RuntimeNodeId, i32>,
}

impl ContentSearchSession {
    pub fn new(id: SearchSessionId, model: &SemanticContentModel, query: String) -> Self {
        let order = model.reading_order();
        let total_blocks =
            (model.completeness == super::ContentCompleteness::Complete).then_some(order.len());
        Self {
            id,
            root: model.root,
            query,
            state: SearchState::Running,
            cursor: 0,
            order,
            results: Vec::new(),
            progress: SearchProgress {
                total_blocks,
                ..Default::default()
            },
            source_offsets: HashMap::new(),
        }
    }

    pub fn cancel(&mut self) {
        if self.state == SearchState::Running {
            self.state = SearchState::Cancelled;
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == SearchState::Running
    }

    pub fn invalidate_source(&mut self, source: RuntimeNodeId) {
        self.results.retain(|result| result.source != source);
    }
}

pub fn search_indexed_content(
    model: &SemanticContentModel,
    cache: &ContentCache,
    query: &str,
) -> Vec<ContentSearchResult> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut results = Vec::new();
    for block in &model.blocks {
        let mut candidates = Vec::new();
        if let Some(label) = &block.label {
            candidates.push(label.as_str());
        }
        if let Some(text) = block.text.visible_text() {
            candidates.push(text);
        }
        for range in cache.ranges_for_source(block.source) {
            candidates.push(range.text.as_str());
        }
        if let Some((text, start)) = candidates
            .into_iter()
            .find_map(|text| text.to_lowercase().find(&query).map(|start| (text, start)))
        {
            results.push(ContentSearchResult {
                block_id: block.id,
                source: block.source,
                range: Some((start, start + query.len())),
                preview: safe_preview(text, start, query.len()),
            });
        }
    }
    results
}

fn safe_preview(text: &str, byte_start: usize, query_bytes: usize) -> String {
    let start = text[..byte_start]
        .char_indices()
        .rev()
        .nth(30)
        .map_or(0, |(index, _)| index);
    let after = (byte_start + query_bytes).min(text.len());
    let end = text[after..]
        .char_indices()
        .nth(50)
        .map_or(text.len(), |(index, _)| after + index);
    text[start..end].replace(['\n', '\r'], " ")
}

pub(crate) fn match_text(
    block_id: ContentBlockId,
    source: RuntimeNodeId,
    text: &str,
    query: &str,
) -> Option<ContentSearchResult> {
    let query = query.trim().to_lowercase();
    let start = text.to_lowercase().find(&query)?;
    Some(ContentSearchResult {
        block_id,
        source,
        range: Some((start, start + query.len())),
        preview: safe_preview(text, start, query.len()),
    })
}

#[cfg(test)]
mod tests {
    use crate::content::{
        ContentBlock, ContentBlockKind, ContentCompleteness, ContentKind, ContentMetadata,
        ContentNavigationIndex, TextContentState,
    };

    use super::*;

    #[test]
    fn semantic_labels_and_loaded_text_are_searchable_by_block_identity() {
        let source = RuntimeNodeId::new(4);
        let block_id = ContentBlockId::new(2);
        let model = SemanticContentModel {
            root: RuntimeNodeId::new(1),
            kind: ContentKind::Document,
            metadata: ContentMetadata::default(),
            roots: vec![block_id],
            blocks: vec![ContentBlock {
                id: block_id,
                source,
                kind: ContentBlockKind::Paragraph,
                label: Some("Semantic navigation".to_owned()),
                text: TextContentState::Unknown,
                children: Vec::new(),
                interactive_sources: Vec::new(),
            }]
            .into(),
            navigation: ContentNavigationIndex::default(),
            completeness: ContentCompleteness::Complete,
            scope_class: crate::content::ContentScopeClass::Primary,
        };
        let cache = ContentCache::new(Default::default());
        let results = search_indexed_content(&model, &cache, "navigation");
        assert_eq!(results[0].block_id, block_id);
    }

    #[test]
    fn progressive_search_is_explicit_streaming_and_cancel_aware() {
        let source = RuntimeNodeId::new(4);
        let block_id = ContentBlockId::new(4);
        let model = SemanticContentModel {
            root: RuntimeNodeId::new(1),
            kind: ContentKind::Document,
            metadata: ContentMetadata::default(),
            roots: vec![block_id],
            blocks: vec![ContentBlock {
                id: block_id,
                source,
                kind: ContentBlockKind::Paragraph,
                label: Some("Streaming semantic result".to_owned()),
                text: TextContentState::Unknown,
                children: Vec::new(),
                interactive_sources: Vec::new(),
            }]
            .into(),
            navigation: ContentNavigationIndex::default(),
            completeness: ContentCompleteness::Unknown,
            scope_class: crate::content::ContentScopeClass::Primary,
        };
        let mut session =
            ContentSearchSession::new(SearchSessionId::new(1), &model, "semantic".to_owned());
        assert_eq!(session.progress.total_blocks, None);
        assert!(session.results.is_empty());
        session
            .results
            .push(match_text(block_id, source, "semantic", "semantic").unwrap());
        assert_eq!(session.results.len(), 1);
        session.cancel();
        assert!(!session.is_running());
    }

    #[test]
    fn mutation_invalidates_only_results_from_affected_source() {
        let model = SemanticContentModel {
            root: RuntimeNodeId::new(1),
            kind: ContentKind::Document,
            metadata: ContentMetadata::default(),
            roots: Vec::new(),
            blocks: Vec::new().into(),
            navigation: ContentNavigationIndex::default(),
            completeness: ContentCompleteness::Complete,
            scope_class: crate::content::ContentScopeClass::Primary,
        };
        let mut session =
            ContentSearchSession::new(SearchSessionId::new(2), &model, "x".to_owned());
        for source in [RuntimeNodeId::new(2), RuntimeNodeId::new(3)] {
            session.results.push(ContentSearchResult {
                block_id: ContentBlockId::new(source.get()),
                source,
                range: None,
                preview: "x".to_owned(),
            });
        }
        session.invalidate_source(RuntimeNodeId::new(2));
        assert_eq!(session.results.len(), 1);
        assert_eq!(session.results[0].source, RuntimeNodeId::new(3));
    }
}
