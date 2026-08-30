use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use crate::{
    backend::{AtspiBackend, BackendError, DocumentProbe},
    events::NormalizedEvent,
    semantic::{RuntimeNodeId, SemanticCache, TextInputKind},
};

use super::{
    ContentBlockId, ContentBlockKind, ContentCache, ContentCacheBudget, ContentCacheMetrics,
    ContentCatalog, ContentMetadata, ContentRangeKey, ContentSearchResult, ContentSearchSession,
    LoadedContentRange, SearchBudget, SearchSessionId, SearchState, SemanticContentModel,
    TextContentState, analyze_virtual_collections, search::match_text, search_indexed_content,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCapabilityStatus {
    Unsupported,
    Declared,
    Verified,
    Quarantined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextProbeOutcome {
    Verified,
    Unsupported,
    Failed,
}

impl TextCapabilityStatus {
    pub fn after_probe(self, outcome: TextProbeOutcome) -> Self {
        if self == Self::Quarantined {
            return Self::Quarantined;
        }
        match outcome {
            TextProbeOutcome::Verified => Self::Verified,
            TextProbeOutcome::Unsupported => Self::Unsupported,
            TextProbeOutcome::Failed => Self::Quarantined,
        }
    }

    pub fn should_probe(self) -> bool {
        matches!(self, Self::Declared | Self::Verified)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentPatchMetrics {
    pub local_invalidations: u64,
    pub local_patches: u64,
    pub catalog_rebuilds: u64,
    pub preserved_block_ids: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterializationBudget {
    pub visible_blocks: usize,
    pub lookahead_blocks: usize,
    pub paragraph_ranges_per_source: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MaterializationPriority {
    Background,
    ExplicitSearch,
    Lookahead,
    VisibleViewport,
    ActiveTask,
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
    tables: Vec<super::SemanticTableModel>,
    text_capabilities: HashMap<RuntimeNodeId, TextCapabilityStatus>,
    next_search_session_id: u64,
    patch_metrics: ContentPatchMetrics,
    pending_sources: HashSet<RuntimeNodeId>,
    structural_rebuild_required: bool,
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
            tables: analyze_tables(semantic),
            text_capabilities: declared_text_capabilities(semantic),
            next_search_session_id: 1,
            patch_metrics: ContentPatchMetrics::default(),
            pending_sources: HashSet::new(),
            structural_rebuild_required: false,
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

    pub fn table(&self, owner: RuntimeNodeId) -> Option<&super::SemanticTableModel> {
        self.tables.iter().find(|table| table.owner == owner)
    }

    pub fn text_capability(&self, source: RuntimeNodeId) -> TextCapabilityStatus {
        self.text_capabilities
            .get(&source)
            .copied()
            .unwrap_or(TextCapabilityStatus::Unsupported)
    }

    pub fn patch_metrics(&self) -> ContentPatchMetrics {
        self.patch_metrics
    }

    pub fn cache_metrics(&self) -> super::ContentCacheMetrics {
        self.cache.metrics()
    }

    pub fn rebuild_semantics(&mut self, semantic: &SemanticCache) {
        if !self.structural_rebuild_required && !self.pending_sources.is_empty() {
            let sources = std::mem::take(&mut self.pending_sources);
            let mut safe_local_patch = true;
            for source in &sources {
                let Some(node) = semantic.node(*source) else {
                    safe_local_patch = false;
                    break;
                };
                for model_index in 0..self.catalog.models().count() {
                    let ids = self
                        .catalog
                        .model_mut(model_index)
                        .map(|model| model.blocks.blocks_for_source(*source).to_vec())
                        .unwrap_or_default();
                    for id in ids {
                        let Some(model) = self.catalog.model_mut(model_index) else {
                            continue;
                        };
                        let Some(block) = model.blocks.get_mut(id) else {
                            continue;
                        };
                        block.label = node.name.clone().or_else(|| node.description.clone());
                        if matches!(block.text, TextContentState::Summary(_)) {
                            block.text = block
                                .label
                                .clone()
                                .or_else(|| node.value.clone())
                                .map(TextContentState::Summary)
                                .unwrap_or(TextContentState::Unavailable);
                        }
                    }
                }
            }
            if safe_local_patch {
                self.patch_metrics.local_patches += 1;
                return;
            }
        }
        self.pending_sources.clear();
        self.structural_rebuild_required = false;
        let previous_ids: std::collections::HashSet<_> = self
            .catalog
            .models()
            .flat_map(|model| model.blocks.ids())
            .collect();
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
        self.tables = analyze_tables(semantic);
        self.text_capabilities
            .retain(|source, _| semantic.node(*source).is_some());
        for (source, status) in declared_text_capabilities(semantic) {
            self.text_capabilities.entry(source).or_insert(status);
        }
        self.patch_metrics.catalog_rebuilds += 1;
        self.patch_metrics.preserved_block_ids += self
            .catalog
            .models()
            .flat_map(|model| model.blocks.ids())
            .filter(|id| previous_ids.contains(id))
            .count() as u64;
    }

    pub fn invalidate_event(&mut self, semantic: &SemanticCache, event: &NormalizedEvent) -> usize {
        let source_id = semantic.runtime_id(event.source());
        let invalidated = source_id.map_or(0, |source| self.cache.invalidate_source(source));
        if let Some(source) = source_id {
            self.pending_sources.insert(source);
        }
        if matches!(
            event,
            NormalizedEvent::ChildrenChanged { .. }
                | NormalizedEvent::WindowCreated { .. }
                | NormalizedEvent::WindowDestroyed { .. }
                | NormalizedEvent::CacheAdded { .. }
                | NormalizedEvent::CacheRemoved { .. }
        ) {
            self.structural_rebuild_required = true;
        }
        if invalidated > 0 {
            self.patch_metrics.local_invalidations += 1;
        }
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
            .filter(|block| self.text_capability(block.source).should_probe())
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
            let read_result = backend
                .read_content_paragraphs(
                    &node.backend_locator,
                    0,
                    budget.paragraph_ranges_per_source,
                )
                .await;
            match read_result {
                Ok((_count, ranges)) => {
                    let status = self
                        .text_capability(source)
                        .after_probe(TextProbeOutcome::Verified);
                    self.text_capabilities.insert(source, status);
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
                    let status = self
                        .text_capability(source)
                        .after_probe(TextProbeOutcome::Unsupported);
                    self.text_capabilities.insert(source, status);
                    report.unavailable_blocks += 1;
                }
                Err(error) => {
                    // A declared Text interface that fails a bounded read is
                    // not retried automatically in this application runtime.
                    // This is generic observed-behaviour quarantine, not a
                    // toolkit allow/deny list.
                    let status = self
                        .text_capability(source)
                        .after_probe(TextProbeOutcome::Failed);
                    self.text_capabilities.insert(source, status);
                    tracing::warn!(source = %source, %error, "quarantined unreliable Text capability");
                    report.unavailable_blocks += 1;
                }
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

    pub fn begin_progressive_search(
        &mut self,
        root: RuntimeNodeId,
        query: String,
    ) -> Option<ContentSearchSession> {
        let model = self.catalog.get(root)?;
        let id = SearchSessionId::new(self.next_search_session_id);
        self.next_search_session_id = self.next_search_session_id.saturating_add(1);
        Some(ContentSearchSession::new(id, model, query))
    }

    pub async fn progressive_search_step(
        &mut self,
        backend: &AtspiBackend,
        semantic: &SemanticCache,
        session: &mut ContentSearchSession,
        budget: SearchBudget,
    ) {
        if !session.is_running() {
            return;
        }
        let Some(model) = self.catalog.get(session.root) else {
            session.state = SearchState::Cancelled;
            return;
        };
        let mut block_budget = budget.blocks_per_tick;
        let mut rpc_budget = budget.text_rpcs_per_tick;
        while block_budget > 0 && session.cursor < session.order.len() {
            if !session.is_running() {
                break;
            }
            let id = session.order[session.cursor];
            block_budget -= 1;
            let Some(block) = model.block(id).cloned() else {
                session.cursor += 1;
                continue;
            };
            if semantic
                .node(block.source)
                .is_some_and(|node| node.text_input_kind == Some(TextInputKind::Password))
            {
                session.cursor += 1;
                session.progress.scanned_blocks += 1;
                continue;
            }
            let mut searchable = block
                .label
                .clone()
                .or_else(|| block.text.visible_text().map(str::to_owned));
            let mut block_complete = true;
            let loaded = self
                .cache
                .ranges_for_source(block.source)
                .map(|range| range.text.clone())
                .collect::<Vec<_>>();
            if !loaded.is_empty() {
                searchable = Some(loaded.join("\n"));
            } else if matches!(block.text, TextContentState::Unknown)
                && rpc_budget > 0
                && self.text_capability(block.source).should_probe()
            {
                rpc_budget -= 1;
                session.progress.text_rpcs += 1;
                let Some(node) = semantic.node(block.source) else {
                    continue;
                };
                let offset = session
                    .source_offsets
                    .get(&block.source)
                    .copied()
                    .unwrap_or(0);
                match backend
                    .read_content_paragraphs(&node.backend_locator, offset, 1)
                    .await
                {
                    Ok((character_count, ranges)) => {
                        let status = self
                            .text_capability(block.source)
                            .after_probe(TextProbeOutcome::Verified);
                        self.text_capabilities.insert(block.source, status);
                        if let Some(range) = ranges.into_iter().next() {
                            searchable = Some(range.text.clone());
                            self.cache.insert(LoadedContentRange {
                                block_id: block.id,
                                key: ContentRangeKey {
                                    source: block.source,
                                    start: range.start,
                                    end: range.end,
                                },
                                text: range.text,
                            });
                            block_complete = range.end >= character_count;
                            session.source_offsets.insert(block.source, range.end);
                        }
                    }
                    Err(BackendError::ContentTextUnsupported(_)) => {
                        let status = self
                            .text_capability(block.source)
                            .after_probe(TextProbeOutcome::Unsupported);
                        self.text_capabilities.insert(block.source, status);
                    }
                    Err(error) => {
                        let status = self
                            .text_capability(block.source)
                            .after_probe(TextProbeOutcome::Failed);
                        self.text_capabilities.insert(block.source, status);
                        tracing::warn!(source = %block.source, %error, "quarantined Text source during progressive search");
                    }
                }
            }
            if let Some(text) = searchable
                && let Some(result) = match_text(block.id, block.source, &text, &session.query)
            {
                session.results.push(result);
            }
            if block_complete {
                session.cursor += 1;
                session.progress.scanned_blocks += 1;
            } else if rpc_budget == 0 {
                break;
            }
        }
        if session.cursor >= session.order.len() {
            session.state = SearchState::Complete;
        }
    }

    pub fn displayed_block_text(&self, root: RuntimeNodeId, id: ContentBlockId) -> Option<String> {
        self.catalog
            .get(root)
            .and_then(|model| model.block(id))
            .map(|block| reader_text(block, &self.cache))
    }
}

fn declared_text_capabilities(
    semantic: &SemanticCache,
) -> HashMap<RuntimeNodeId, TextCapabilityStatus> {
    semantic
        .nodes()
        .filter(|node| {
            node.text_input_kind != Some(TextInputKind::Password)
                && node
                    .debug
                    .interfaces
                    .iter()
                    .any(|interface| interface == "Text")
        })
        .map(|node| (node.runtime_id, TextCapabilityStatus::Declared))
        .collect()
}

fn analyze_tables(semantic: &SemanticCache) -> Vec<super::SemanticTableModel> {
    semantic
        .nodes()
        .filter(|node| node.role == crate::semantic::SemanticRole::Table)
        .filter_map(|node| super::SemanticTableModel::analyze(semantic, node.runtime_id))
        .collect()
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
        let block_id = runtime
            .catalog()
            .models()
            .next()
            .unwrap()
            .blocks
            .blocks_for_source(source_id)[0];
        runtime.rebuild_semantics(&semantic);
        assert_eq!(runtime.patch_metrics().local_patches, 1);
        assert_eq!(runtime.patch_metrics().catalog_rebuilds, 0);
        assert_eq!(
            runtime
                .catalog()
                .models()
                .next()
                .unwrap()
                .blocks
                .blocks_for_source(source_id),
            &[block_id]
        );
    }

    #[test]
    fn text_capability_trust_is_fail_closed_and_quarantine_does_not_retry() {
        assert_eq!(
            TextCapabilityStatus::Declared.after_probe(TextProbeOutcome::Verified),
            TextCapabilityStatus::Verified
        );
        let quarantined = TextCapabilityStatus::Declared.after_probe(TextProbeOutcome::Failed);
        assert_eq!(quarantined, TextCapabilityStatus::Quarantined);
        assert!(!quarantined.should_probe());
        assert_eq!(
            quarantined.after_probe(TextProbeOutcome::Verified),
            TextCapabilityStatus::Quarantined
        );
    }
}
