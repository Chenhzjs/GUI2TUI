use std::time::Instant;

use crate::{
    backend::{AtspiBackend, BackendError, DocumentProbe},
    events::NormalizedEvent,
    semantic::{RuntimeNodeId, SemanticCache, TextInputKind},
};

use super::{
    ContentBlockId, ContentBlockKind, ContentCache, ContentCacheBudget, ContentCacheMetrics,
    ContentCatalog, ContentMetadata, ContentRangeKey, ContentSearchResult, LoadedContentRange,
    SemanticContentModel, TextContentState, analyze_virtual_collections, search_indexed_content,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterializationBudget {
    pub visible_blocks: usize,
    pub lookahead_blocks: usize,
    pub paragraph_ranges_per_source: usize,
}

impl Default for MaterializationBudget {
    fn default() -> Self {
        Self {
            visible_blocks: 12,
            lookahead_blocks: 6,
            paragraph_ranges_per_source: 16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderBlock {
    pub id: ContentBlockId,
    pub source: RuntimeNodeId,
    pub kind: ContentBlockKind,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaterializationReport {
    pub requested_blocks: usize,
    pub backend_text_rpcs: usize,
    pub loaded_ranges: usize,
    pub unavailable_blocks: usize,
    pub duration_micros: u128,
    pub cache: ContentCacheMetrics,
}

pub struct ContentRuntime {
    catalog: ContentCatalog,
    cache: ContentCache,
    next_dynamic_block_id: u64,
    virtual_collections: Vec<super::VirtualCollectionModel>,
}

impl ContentRuntime {
    pub fn new(semantic: &SemanticCache, budget: ContentCacheBudget) -> Self {
        let catalog = ContentCatalog::analyze(semantic);
        let next_dynamic_block_id = catalog
            .models()
            .flat_map(|model| model.blocks.iter())
            .map(|block| block.id.get())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            catalog,
            cache: ContentCache::new(budget),
            next_dynamic_block_id,
            virtual_collections: analyze_virtual_collections(semantic),
        }
    }

    pub fn catalog(&self) -> &ContentCatalog {
        &self.catalog
    }

    pub fn model(&self, root: RuntimeNodeId) -> Option<&SemanticContentModel> {
        self.catalog.get(root)
    }

    pub fn cache(&self) -> &ContentCache {
        &self.cache
    }

    pub fn virtual_collections(&self) -> &[super::VirtualCollectionModel] {
        &self.virtual_collections
    }

    pub fn rebuild_semantics(&mut self, semantic: &SemanticCache) {
        let replacement = ContentCatalog::analyze(semantic);
        let live_sources: std::collections::HashSet<_> = replacement
            .models()
            .flat_map(SemanticContentModel::source_nodes)
            .collect();
        let stale: Vec<_> = self
            .cache
            .all_ranges()
            .map(|range| range.key.source)
            .filter(|source| !live_sources.contains(source))
            .collect();
        for source in stale {
            self.cache.invalidate_source(source);
        }
        self.catalog = replacement;
        self.virtual_collections = analyze_virtual_collections(semantic);
    }

    pub fn invalidate_event(&mut self, semantic: &SemanticCache, event: &NormalizedEvent) -> usize {
        let invalidated = semantic
            .runtime_id(event.source())
            .map_or(0, |source| self.cache.invalidate_source(source));
        if let NormalizedEvent::ActiveDescendantChanged {
            container,
            descendant,
        } = event
            && let Some(owner) = semantic.runtime_id(container)
            && let Some(model) = self
                .virtual_collections
                .iter_mut()
                .find(|model| model.owner == owner)
        {
            model.apply_active_descendant(
                descendant
                    .as_ref()
                    .and_then(|locator| semantic.runtime_id(locator)),
            );
        }
        invalidated
    }

    pub async fn enrich_document_metadata(
        &mut self,
        backend: &AtspiBackend,
        semantic: &SemanticCache,
        root: RuntimeNodeId,
    ) -> Result<DocumentProbe, BackendError> {
        let node = semantic.node(root).ok_or_else(|| {
            BackendError::SemanticCache(format!("content root {root} disappeared"))
        })?;
        let probe = backend.probe_document(&node.backend_locator).await?;
        if let Some(model_index) = self.catalog.by_root_index(root)
            && let Some(model) = self.catalog.model_mut(model_index)
        {
            model.metadata = ContentMetadata {
                title: model.metadata.title.clone(),
                locale: probe.locale.clone(),
                current_page: probe.current_page,
                page_count: probe.page_count,
                attributes: probe.attributes.clone(),
            };
        }
        Ok(probe)
    }

    pub async fn materialize_viewport(
        &mut self,
        backend: &AtspiBackend,
        semantic: &SemanticCache,
        root: RuntimeNodeId,
        position: ContentBlockId,
        budget: MaterializationBudget,
    ) -> Result<(Vec<ReaderBlock>, MaterializationReport), BackendError> {
        let started = Instant::now();
        let model = self.catalog.get(root).ok_or_else(|| {
            BackendError::SemanticCache(format!("content root {root} disappeared"))
        })?;
        let order = model.reading_order();
        let start = order.iter().position(|id| *id == position).unwrap_or(0);
        let end = (start + budget.visible_blocks + budget.lookahead_blocks).min(order.len());
        let requested = order[start..end].to_vec();
        let pending: Vec<_> = requested
            .iter()
            .filter_map(|id| model.block(*id))
            .filter(|block| matches!(block.text, TextContentState::Unknown))
            .map(|block| (block.id, block.source))
            .collect();
        let mut report = MaterializationReport {
            requested_blocks: requested.len(),
            ..Default::default()
        };
        for (base_id, source) in pending {
            let Some(node) = semantic.node(source) else {
                report.unavailable_blocks += 1;
                continue;
            };
            if node.text_input_kind == Some(TextInputKind::Password) {
                report.unavailable_blocks += 1;
                continue;
            }
            report.backend_text_rpcs += 1;
            let semantic_block = self
                .catalog
                .get(root)
                .and_then(|model| model.block(base_id))
                .is_some_and(|block| {
                    matches!(
                        block.kind,
                        ContentBlockKind::Heading { .. }
                            | ContentBlockKind::Paragraph
                            | ContentBlockKind::Link
                            | ContentBlockKind::ListItem
                            | ContentBlockKind::Quote
                    )
                });
            let read_result = if semantic_block {
                backend
                    .read_semantic_text_block(&node.backend_locator)
                    .await
                    .map(|range| (range.end, vec![range]))
            } else {
                backend
                    .read_content_paragraphs(
                        &node.backend_locator,
                        0,
                        budget.paragraph_ranges_per_source,
                    )
                    .await
            };
            match read_result {
                Ok((_count, ranges)) => {
                    for range in ranges {
                        let block_id = if range.start == 0 {
                            base_id
                        } else {
                            let id = ContentBlockId::new(self.next_dynamic_block_id);
                            self.next_dynamic_block_id =
                                self.next_dynamic_block_id.saturating_add(1);
                            id
                        };
                        if self.cache.insert(LoadedContentRange {
                            block_id,
                            key: ContentRangeKey {
                                source,
                                start: range.start,
                                end: range.end,
                            },
                            text: range.text,
                        }) {
                            report.loaded_ranges += 1;
                        }
                    }
                }
                Err(BackendError::ContentTextUnsupported(_))
                | Err(BackendError::NonAdvancingTextRange { .. }) => {
                    report.unavailable_blocks += 1;
                }
                Err(error) => return Err(error),
            }
        }
        let model = self.catalog.get(root).ok_or_else(|| {
            BackendError::SemanticCache(format!(
                "content root {root} disappeared during materialization"
            ))
        })?;
        let blocks = requested
            .iter()
            .take(budget.visible_blocks)
            .filter_map(|id| model.block(*id))
            .map(|block| ReaderBlock {
                id: block.id,
                source: block.source,
                kind: block.kind.clone(),
                text: reader_text(block, &self.cache),
            })
            .collect();
        report.duration_micros = started.elapsed().as_micros();
        report.cache = self.cache.metrics();
        Ok((blocks, report))
    }

    pub fn search(&self, root: RuntimeNodeId, query: &str) -> Vec<ContentSearchResult> {
        self.catalog.get(root).map_or_else(Vec::new, |model| {
            search_indexed_content(model, &self.cache, query)
        })
    }

    pub fn displayed_block_text(&self, root: RuntimeNodeId, id: ContentBlockId) -> Option<String> {
        self.catalog
            .get(root)
            .and_then(|model| model.block(id))
            .map(|block| reader_text(block, &self.cache))
    }
}

fn reader_text(block: &super::ContentBlock, cache: &ContentCache) -> String {
    let loaded = cache
        .ranges_for_source(block.source)
        .map(|range| range.text.trim_end().to_owned())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if !loaded.is_empty() {
        return loaded;
    }
    block
        .text
        .visible_text()
        .or(block.label.as_deref())
        .unwrap_or("[content unavailable]")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode, SemanticRole};

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.5", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: (!name.is_empty()).then(|| name.to_owned()),
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

    #[test]
    fn text_mutation_invalidates_only_affected_content_source() {
        let mut document = node(1, SemanticRole::Document, "Doc");
        document
            .children
            .push(node(2, SemanticRole::Paragraph, "Alpha"));
        let semantic = SemanticCache::from_snapshot(document).unwrap();
        let source = semantic
            .nodes()
            .find(|node| node.name.as_deref() == Some("Alpha"))
            .unwrap();
        let locator = source.backend_locator.clone();
        let source_id = source.runtime_id;
        let mut runtime = ContentRuntime::new(&semantic, ContentCacheBudget::default());
        runtime.cache.insert(LoadedContentRange {
            block_id: ContentBlockId::new(1),
            key: ContentRangeKey {
                source: source_id,
                start: 0,
                end: 5,
            },
            text: "Alpha".to_owned(),
        });
        assert_eq!(
            runtime.invalidate_event(
                &semantic,
                &NormalizedEvent::TextChanged {
                    locator,
                    change: "insert".to_owned(),
                    start: 0,
                    length: 1,
                }
            ),
            1
        );
        assert_eq!(runtime.cache.metrics().ranges, 0);
    }
}
