use crate::semantic::RuntimeNodeId;

use super::{ContentBlockId, ContentCache, SemanticContentModel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentSearchResult {
    pub block_id: ContentBlockId,
    pub source: RuntimeNodeId,
    pub range: Option<(usize, usize)>,
    pub preview: String,
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
            }],
            navigation: ContentNavigationIndex::default(),
            completeness: ContentCompleteness::Complete,
        };
        let cache = ContentCache::new(Default::default());
        let results = search_indexed_content(&model, &cache, "navigation");
        assert_eq!(results[0].block_id, block_id);
    }
}
