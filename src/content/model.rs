use std::{collections::HashMap, fmt, ops::Index};

use crate::semantic::{
    CachedSemanticNode, RuntimeNodeId, SemanticCache, SemanticRole, TextInputKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentBlockId(u64);

impl ContentBlockId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ContentBlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    Document,
    RichText,
    StructuredContent,
    Hypertext,
    UnknownContent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentCompleteness {
    Complete,
    PartialRealized,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentScopeClass {
    Primary,
    ActiveTransient,
    BackgroundSecondary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentMetadata {
    pub title: Option<String>,
    pub locale: Option<String>,
    pub current_page: Option<i32>,
    pub page_count: Option<i32>,
    pub attributes: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextContentState {
    Unknown,
    Summary(String),
    Loaded(String),
    Unavailable,
}

impl TextContentState {
    pub fn visible_text(&self) -> Option<&str> {
        match self {
            Self::Summary(text) | Self::Loaded(text) => Some(text),
            Self::Unknown | Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpaqueContentKind {
    Image,
    Audio,
    Video,
    Canvas,
    Graphical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentBlockKind {
    Heading { level: Option<u8> },
    Paragraph,
    Text,
    Link,
    List,
    ListItem,
    Quote,
    Landmark,
    FormAnchor,
    TableAnchor,
    Comment,
    OpaqueContent(OpaqueContentKind),
    Group,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentBlock {
    pub id: ContentBlockId,
    pub source: RuntimeNodeId,
    pub kind: ContentBlockKind,
    pub label: Option<String>,
    pub text: TextContentState,
    pub children: Vec<ContentBlockId>,
    pub interactive_sources: Vec<RuntimeNodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentArena {
    by_id: HashMap<ContentBlockId, ContentBlock>,
    order: Vec<ContentBlockId>,
    by_source: HashMap<RuntimeNodeId, Vec<ContentBlockId>>,
}

impl ContentArena {
    pub fn from_blocks(blocks: impl IntoIterator<Item = ContentBlock>) -> Self {
        let blocks = blocks.into_iter();
        let (minimum, _) = blocks.size_hint();
        let mut arena = Self {
            by_id: HashMap::with_capacity(minimum),
            order: Vec::with_capacity(minimum),
            by_source: HashMap::with_capacity(minimum),
        };
        for block in blocks {
            arena.insert(block);
        }
        arena
    }

    pub fn insert(&mut self, block: ContentBlock) {
        let id = block.id;
        let source = block.source;
        if let Some(previous) = self.by_id.insert(id, block) {
            if previous.source != source
                && let Some(ids) = self.by_source.get_mut(&previous.source)
            {
                ids.retain(|candidate| *candidate != id);
            }
        } else {
            self.order.push(id);
        }
        let ids = self.by_source.entry(source).or_default();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    pub fn get(&self, id: ContentBlockId) -> Option<&ContentBlock> {
        self.by_id.get(&id)
    }

    pub fn get_mut(&mut self, id: ContentBlockId) -> Option<&mut ContentBlock> {
        self.by_id.get_mut(&id)
    }

    pub fn blocks_for_source(&self, source: RuntimeNodeId) -> &[ContentBlockId] {
        self.by_source
            .get(&source)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn iter(&self) -> impl Iterator<Item = &ContentBlock> {
        self.order.iter().filter_map(|id| self.by_id.get(id))
    }

    pub fn ids(&self) -> impl Iterator<Item = ContentBlockId> + '_ {
        self.order.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

impl From<Vec<ContentBlock>> for ContentArena {
    fn from(blocks: Vec<ContentBlock>) -> Self {
        Self::from_blocks(blocks)
    }
}

impl<'a> IntoIterator for &'a ContentArena {
    type Item = &'a ContentBlock;
    type IntoIter = Box<dyn Iterator<Item = &'a ContentBlock> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl Index<usize> for ContentArena {
    type Output = ContentBlock;

    fn index(&self, index: usize) -> &Self::Output {
        self.by_id
            .get(&self.order[index])
            .expect("content arena order references a missing block")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentNavigationIndex {
    pub headings: Vec<ContentBlockId>,
    pub links: Vec<ContentBlockId>,
    pub form_fields: Vec<RuntimeNodeId>,
    pub lists: Vec<ContentBlockId>,
    pub tables: Vec<ContentBlockId>,
    pub landmarks: Vec<ContentBlockId>,
    pub comments: Vec<ContentBlockId>,
    pub opaque: Vec<ContentBlockId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticContentModel {
    pub root: RuntimeNodeId,
    pub kind: ContentKind,
    pub metadata: ContentMetadata,
    pub roots: Vec<ContentBlockId>,
    pub blocks: ContentArena,
    pub navigation: ContentNavigationIndex,
    pub completeness: ContentCompleteness,
    pub scope_class: ContentScopeClass,
}

impl SemanticContentModel {
    pub fn block(&self, id: ContentBlockId) -> Option<&ContentBlock> {
        self.blocks.get(id)
    }

    pub fn ordered_blocks(&self) -> impl Iterator<Item = &ContentBlock> {
        self.blocks.iter()
    }

    pub fn reading_order(&self) -> Vec<ContentBlockId> {
        let mut output = Vec::new();
        for root in &self.roots {
            self.append_reading_order(*root, &mut output);
        }
        output
    }

    fn append_reading_order(&self, id: ContentBlockId, output: &mut Vec<ContentBlockId>) {
        let Some(block) = self.block(id) else { return };
        output.push(id);
        for child in &block.children {
            self.append_reading_order(*child, output);
        }
    }

    pub fn source_nodes(&self) -> impl Iterator<Item = RuntimeNodeId> + '_ {
        self.blocks.iter().map(|block| block.source)
    }

    pub fn summary(&self) -> ContentSummary {
        ContentSummary {
            blocks: self.blocks.len(),
            headings: self.navigation.headings.len(),
            links: self.navigation.links.len(),
            forms: self.navigation.form_fields.len(),
            lists: self.navigation.lists.len(),
            tables: self.navigation.tables.len(),
            opaque: self.navigation.opaque.len(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentSummary {
    pub blocks: usize,
    pub headings: usize,
    pub links: usize,
    pub forms: usize,
    pub lists: usize,
    pub tables: usize,
    pub opaque: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ContentCatalog {
    models: Vec<SemanticContentModel>,
    by_root: HashMap<RuntimeNodeId, usize>,
    owning_root: HashMap<RuntimeNodeId, RuntimeNodeId>,
}

impl ContentCatalog {
    pub fn analyze(cache: &SemanticCache) -> Self {
        let mut candidates: Vec<_> = cache
            .nodes()
            .filter(|node| is_content_root(node))
            .map(|node| node.runtime_id)
            .collect();
        candidates.sort();
        let all_candidates: std::collections::HashSet<_> = candidates.iter().copied().collect();
        candidates.retain(|candidate| {
            !ancestors(cache, *candidate).any(|ancestor| all_candidates.contains(&ancestor))
        });
        // Application chrome and transient panels can expose secondary Document
        // objects in the same interaction scope. Keep the richest visible
        // content root per Window/Dialog scope; independent windows and modal
        // dialogs retain their own Reader.
        let mut best_by_scope: HashMap<RuntimeNodeId, (RuntimeNodeId, usize)> = HashMap::new();
        for candidate in &candidates {
            let scope = nearest_content_scope(cache, *candidate).unwrap_or(*candidate);
            let score = descendant_count(cache, *candidate);
            let entry = best_by_scope.entry(scope).or_insert((*candidate, score));
            if score > entry.1 {
                *entry = (*candidate, score);
            }
        }
        let selected: std::collections::HashSet<_> =
            best_by_scope.values().map(|(root, _)| *root).collect();
        candidates.retain(|candidate| selected.contains(candidate));

        let primary = candidates.iter().copied().max_by_key(|candidate| {
            let scope = nearest_content_scope(cache, *candidate).unwrap_or(*candidate);
            let active = cache
                .node(scope)
                .is_some_and(|node| has_state(node, "active") || has_state(node, "focused"));
            (active, descendant_count(cache, *candidate))
        });

        let mut models = Vec::new();
        let mut by_root = HashMap::new();
        let mut owning_root = HashMap::new();
        for root in candidates {
            let scope = nearest_content_scope(cache, root).unwrap_or(root);
            let scope_class = if Some(root) == primary {
                ContentScopeClass::Primary
            } else if cache.node(scope).is_some_and(|node| {
                node.role == SemanticRole::Dialog
                    && (has_state(node, "active") || has_state(node, "focused"))
            }) {
                ContentScopeClass::ActiveTransient
            } else {
                ContentScopeClass::BackgroundSecondary
            };
            let model = analyze_model(cache, root, scope_class);
            let index = models.len();
            for source in model.source_nodes() {
                owning_root.entry(source).or_insert(root);
            }
            owning_root.insert(root, root);
            by_root.insert(root, index);
            models.push(model);
        }
        Self {
            models,
            by_root,
            owning_root,
        }
    }

    pub fn models(&self) -> impl Iterator<Item = &SemanticContentModel> {
        self.models.iter()
    }

    pub fn visible_models(&self) -> impl Iterator<Item = &SemanticContentModel> {
        self.models
            .iter()
            .filter(|model| model.scope_class != ContentScopeClass::BackgroundSecondary)
    }

    pub fn get(&self, root: RuntimeNodeId) -> Option<&SemanticContentModel> {
        self.by_root
            .get(&root)
            .and_then(|index| self.models.get(*index))
    }

    pub(crate) fn by_root_index(&self, root: RuntimeNodeId) -> Option<usize> {
        self.by_root.get(&root).copied()
    }

    pub(crate) fn model_mut(&mut self, index: usize) -> Option<&mut SemanticContentModel> {
        self.models.get_mut(index)
    }

    pub fn owning_root(&self, source: RuntimeNodeId) -> Option<RuntimeNodeId> {
        self.owning_root.get(&source).copied()
    }

    pub fn is_content_source(&self, source: RuntimeNodeId) -> bool {
        self.owning_root.contains_key(&source)
    }
}

fn nearest_content_scope(cache: &SemanticCache, id: RuntimeNodeId) -> Option<RuntimeNodeId> {
    ancestors(cache, id).find(|ancestor| {
        cache
            .node(*ancestor)
            .is_some_and(|node| matches!(node.role, SemanticRole::Window | SemanticRole::Dialog))
    })
}

fn descendant_count(cache: &SemanticCache, root: RuntimeNodeId) -> usize {
    let mut count = 0;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = cache.node(id) else { continue };
        count += 1;
        stack.extend(node.children.iter().copied());
    }
    count
}

fn is_content_root(node: &CachedSemanticNode) -> bool {
    let active_or_standalone = node.parent.is_none() || has_state(node, "showing");
    active_or_standalone
        && (node.role == SemanticRole::Document
            || (matches!(node.role, SemanticRole::Text | SemanticRole::TextInput)
                && node.text_input_kind != Some(TextInputKind::Password)
                && has_interface(node, "Text")
                && (node.role == SemanticRole::Text
                    || (has_state(node, "multi-line") && has_state(node, "read-only")))
                && (is_readable_text(node)
                    || node.children.is_empty()
                    || node.name.as_deref().is_some_and(|name| name.len() > 80))))
}

fn ancestors(cache: &SemanticCache, id: RuntimeNodeId) -> impl Iterator<Item = RuntimeNodeId> + '_ {
    std::iter::successors(cache.node(id).and_then(|node| node.parent), |parent| {
        cache.node(*parent).and_then(|node| node.parent)
    })
}

fn analyze_model(
    cache: &SemanticCache,
    root: RuntimeNodeId,
    scope_class: ContentScopeClass,
) -> SemanticContentModel {
    let root_node = cache.node(root).expect("content root must exist");
    let kind = if root_node.role == SemanticRole::Document {
        if has_interface(root_node, "Hypertext") {
            ContentKind::Hypertext
        } else {
            ContentKind::Document
        }
    } else if has_interface(root_node, "Text") {
        ContentKind::RichText
    } else {
        ContentKind::UnknownContent
    };
    let completeness = if has_state(root_node, "manages-descendants") {
        ContentCompleteness::PartialRealized
    } else if !root_node.truncations.is_empty() {
        ContentCompleteness::Unknown
    } else {
        ContentCompleteness::Complete
    };
    let mut builder = ModelBuilder {
        cache,
        next_id: 1,
        blocks: Vec::new(),
        navigation: ContentNavigationIndex::default(),
    };
    let mut roots = Vec::new();
    for child in &root_node.children {
        if let Some(block) = builder.visit(*child) {
            roots.push(block);
        }
    }
    if roots.is_empty()
        && let Some(block) = builder.visit(root)
    {
        roots.push(block);
    }
    SemanticContentModel {
        root,
        kind,
        metadata: ContentMetadata {
            title: root_node.name.clone(),
            ..Default::default()
        },
        roots,
        blocks: ContentArena::from_blocks(builder.blocks),
        navigation: builder.navigation,
        completeness,
        scope_class,
    }
}

struct ModelBuilder<'a> {
    cache: &'a SemanticCache,
    next_id: u64,
    blocks: Vec<ContentBlock>,
    navigation: ContentNavigationIndex,
}

impl ModelBuilder<'_> {
    fn visit(&mut self, id: RuntimeNodeId) -> Option<ContentBlockId> {
        let node = self.cache.node(id)?;
        if node.text_input_kind == Some(TextInputKind::Password) {
            return None;
        }
        if is_ui_chrome_role(&node.role) {
            return None;
        }
        let kind = block_kind(node);
        let mut child_blocks = Vec::new();
        for child in &node.children {
            if let Some(block) = self.visit(*child) {
                child_blocks.push(block);
            }
        }
        let interactive_sources = if is_control(&node.role) && !is_readable_text(node) {
            vec![id]
        } else {
            Vec::new()
        };
        let label = node.name.clone().or_else(|| node.description.clone());
        let text = if matches!(
            kind,
            ContentBlockKind::Heading { .. }
                | ContentBlockKind::Paragraph
                | ContentBlockKind::Text
                | ContentBlockKind::Link
                | ContentBlockKind::ListItem
                | ContentBlockKind::Quote
        ) {
            if is_readable_text(node) {
                TextContentState::Unknown
            } else {
                label
                    .clone()
                    .or_else(|| node.value.clone())
                    .map(TextContentState::Summary)
                    .unwrap_or_else(|| {
                        if has_interface(node, "Text") {
                            TextContentState::Unknown
                        } else {
                            TextContentState::Unavailable
                        }
                    })
            }
        } else {
            TextContentState::Unavailable
        };
        let semantically_empty = label.is_none()
            && child_blocks.is_empty()
            && interactive_sources.is_empty()
            && matches!(kind, ContentBlockKind::Group | ContentBlockKind::Unknown);
        if semantically_empty {
            return None;
        }
        // A base semantic block is owned by exactly one SemanticCache node.
        // RuntimeNodeId is cache-session stable, so deriving the block ID from
        // it preserves Reader/Outline/Search positions across unrelated local
        // content rebuilds without guessing across ambiguous replacements.
        let block_id = ContentBlockId::new(id.get());
        self.next_id = self.next_id.max(id.get().saturating_add(1));
        match kind {
            ContentBlockKind::Heading { .. } => self.navigation.headings.push(block_id),
            ContentBlockKind::Link => self.navigation.links.push(block_id),
            ContentBlockKind::List => self.navigation.lists.push(block_id),
            ContentBlockKind::TableAnchor => self.navigation.tables.push(block_id),
            ContentBlockKind::Landmark => self.navigation.landmarks.push(block_id),
            ContentBlockKind::Comment => self.navigation.comments.push(block_id),
            ContentBlockKind::OpaqueContent(_) => self.navigation.opaque.push(block_id),
            ContentBlockKind::FormAnchor => {
                self.navigation.form_fields.extend(descendants_matching(
                    self.cache,
                    id,
                    |candidate| is_control(&candidate.role),
                ));
                if is_control(&node.role) {
                    self.navigation.form_fields.push(id);
                }
                self.navigation.form_fields.sort();
                self.navigation.form_fields.dedup();
            }
            _ => {}
        }
        self.blocks.push(ContentBlock {
            id: block_id,
            source: id,
            kind,
            label,
            text,
            children: child_blocks,
            interactive_sources,
        });
        Some(block_id)
    }
}

fn block_kind(node: &CachedSemanticNode) -> ContentBlockKind {
    match node.role {
        SemanticRole::Heading => ContentBlockKind::Heading {
            level: heading_level(node),
        },
        SemanticRole::Paragraph => ContentBlockKind::Paragraph,
        SemanticRole::Text | SemanticRole::Label => ContentBlockKind::Text,
        SemanticRole::Link => ContentBlockKind::Link,
        SemanticRole::List => ContentBlockKind::List,
        SemanticRole::ListItem => ContentBlockKind::ListItem,
        SemanticRole::Quote => ContentBlockKind::Quote,
        SemanticRole::Landmark => ContentBlockKind::Landmark,
        SemanticRole::TextInput if is_readable_text(node) => ContentBlockKind::Text,
        SemanticRole::Form | SemanticRole::TextInput => ContentBlockKind::FormAnchor,
        SemanticRole::Table => ContentBlockKind::TableAnchor,
        SemanticRole::Comment => ContentBlockKind::Comment,
        SemanticRole::Image => ContentBlockKind::OpaqueContent(OpaqueContentKind::Image),
        SemanticRole::Audio => ContentBlockKind::OpaqueContent(OpaqueContentKind::Audio),
        SemanticRole::Video => ContentBlockKind::OpaqueContent(OpaqueContentKind::Video),
        SemanticRole::Unknown(ref value)
            if matches!(value.as_str(), "canvas" | "drawing area" | "animation") =>
        {
            ContentBlockKind::OpaqueContent(OpaqueContentKind::Graphical)
        }
        SemanticRole::Container | SemanticRole::Document => ContentBlockKind::Group,
        _ if is_control(&node.role) => ContentBlockKind::FormAnchor,
        _ => ContentBlockKind::Unknown,
    }
}

fn is_readable_text(node: &CachedSemanticNode) -> bool {
    node.role == SemanticRole::TextInput
        && node.text_input_kind != Some(TextInputKind::Password)
        && has_state(node, "multi-line")
        && has_state(node, "read-only")
        && has_interface(node, "Text")
}

fn heading_level(node: &CachedSemanticNode) -> Option<u8> {
    node.debug.interfaces.iter().find_map(|value| {
        value
            .strip_prefix("heading-level=")
            .and_then(|level| level.parse::<u8>().ok())
    })
}

fn is_ui_chrome_role(role: &SemanticRole) -> bool {
    matches!(
        role,
        SemanticRole::MenuBar
            | SemanticRole::Menu
            | SemanticRole::MenuItem
            | SemanticRole::StatusBar
            | SemanticRole::Window
            | SemanticRole::Dialog
    )
}

fn is_control(role: &SemanticRole) -> bool {
    matches!(
        role,
        SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::RadioButton
            | SemanticRole::TextInput
            | SemanticRole::ComboBox
            | SemanticRole::ListItem
    )
}

fn has_interface(node: &CachedSemanticNode, interface: &str) -> bool {
    node.debug.interfaces.iter().any(|value| value == interface)
}

fn has_state(node: &CachedSemanticNode, state: &str) -> bool {
    node.states
        .iter()
        .any(|candidate| candidate.to_string() == state)
}

fn descendants_matching(
    cache: &SemanticCache,
    root: RuntimeNodeId,
    predicate: impl Fn(&CachedSemanticNode) -> bool + Copy,
) -> Vec<RuntimeNodeId> {
    let mut result = Vec::new();
    let mut stack = cache
        .node(root)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    while let Some(id) = stack.pop() {
        if let Some(node) = cache.node(id) {
            if predicate(node) {
                result.push(id);
            }
            stack.extend(node.children.iter().copied());
        }
    }
    result
}

pub fn format_content_model(model: &SemanticContentModel, with_text: bool) -> String {
    let mut output = format!(
        "Content root={} kind={:?} scope={:?} completeness={:?} title={:?} blocks={} headings={} links={} forms={}\n",
        model.root,
        model.kind,
        model.scope_class,
        model.completeness,
        model.metadata.title,
        model.blocks.len(),
        model.navigation.headings.len(),
        model.navigation.links.len(),
        model.navigation.form_fields.len(),
    );
    for block in model.blocks.iter() {
        let text = if with_text {
            block
                .text
                .visible_text()
                .map(|value| format!(" text={value:?}"))
                .unwrap_or_default()
        } else {
            format!(
                " text={:?}",
                match block.text {
                    TextContentState::Unknown => "unloaded",
                    TextContentState::Summary(_) => "summary",
                    TextContentState::Loaded(_) => "loaded",
                    TextContentState::Unavailable => "unavailable",
                }
            )
        };
        output.push_str(&format!(
            "  block={} source={} kind={:?} label={:?}{} children={:?} controls={:?}\n",
            block.id,
            block.source,
            block.kind,
            block.label,
            text,
            block.children,
            block.interactive_sources,
        ));
    }
    output
}

pub fn format_outline(model: &SemanticContentModel) -> String {
    let mut output = format!(
        "Outline root={} title={:?} headings={}\n",
        model.root,
        model.metadata.title,
        model.navigation.headings.len()
    );
    for id in &model.navigation.headings {
        if let Some(block) = model.block(*id) {
            let level = match block.kind {
                ContentBlockKind::Heading { level: Some(level) } => format!(" level={level}"),
                _ => String::new(),
            };
            output.push_str(&format!(
                "  block={} source={}{} label={:?}\n",
                block.id, block.source, level, block.label
            ));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode, SemanticState, TreeTruncation};

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
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
            truncations: Vec::<TreeTruncation>::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn document_detection_indexes_structure_and_preserves_controls_and_opaque_content() {
        let mut document = node(1, SemanticRole::Document, "Article");
        document.children = vec![
            node(2, SemanticRole::Heading, "Introduction"),
            node(3, SemanticRole::Paragraph, "Body text"),
            node(4, SemanticRole::Link, "AT-SPI"),
            node(5, SemanticRole::TextInput, "Email"),
            node(6, SemanticRole::Image, "Architecture"),
        ];
        let cache = SemanticCache::from_snapshot(document).unwrap();
        let catalog = ContentCatalog::analyze(&cache);
        let model = catalog.models().next().unwrap();
        assert_eq!(model.navigation.headings.len(), 1);
        assert_eq!(model.navigation.links.len(), 1);
        assert_eq!(model.navigation.form_fields.len(), 1);
        assert_eq!(model.navigation.opaque.len(), 1);
        assert!(model.blocks.iter().any(|block| {
            matches!(
                block.kind,
                ContentBlockKind::OpaqueContent(OpaqueContentKind::Image)
            )
        }));
    }

    #[test]
    fn password_is_never_a_content_block() {
        let mut document = node(1, SemanticRole::Document, "Article");
        let mut password = node(2, SemanticRole::TextInput, "Password");
        password.text_input_kind = Some(TextInputKind::Password);
        password.value = Some("must-not-survive".to_owned());
        document.children.push(password);
        let cache = SemanticCache::from_snapshot(document).unwrap();
        let model = ContentCatalog::analyze(&cache)
            .models()
            .next()
            .unwrap()
            .clone();
        assert!(
            model
                .blocks
                .iter()
                .all(|block| block.source != RuntimeNodeId::new(2))
        );
        assert!(!format_content_model(&model, true).contains("must-not-survive"));
    }

    #[test]
    fn content_block_identity_is_session_stable_from_runtime_source() {
        let mut document = node(99, SemanticRole::Document, "Article");
        document
            .children
            .push(node(42, SemanticRole::Paragraph, "Body"));
        let cache = SemanticCache::from_snapshot(document).unwrap();
        let model = ContentCatalog::analyze(&cache)
            .models()
            .next()
            .unwrap()
            .clone();
        assert_eq!(model.blocks[0].id.get(), model.blocks[0].source.get());
    }

    #[test]
    fn arena_indexes_blocks_by_id_and_source() {
        let block = ContentBlock {
            id: ContentBlockId::new(7),
            source: RuntimeNodeId::new(9),
            kind: ContentBlockKind::Paragraph,
            label: Some("Indexed".to_owned()),
            text: TextContentState::Unknown,
            children: Vec::new(),
            interactive_sources: Vec::new(),
        };
        let arena = ContentArena::from_blocks([block]);
        assert_eq!(
            arena.get(ContentBlockId::new(7)).unwrap().label.as_deref(),
            Some("Indexed")
        );
        assert_eq!(
            arena.blocks_for_source(RuntimeNodeId::new(9)),
            &[ContentBlockId::new(7)]
        );
    }

    #[test]
    fn generic_scope_policy_hides_background_secondary_document() {
        let mut app = node(1, SemanticRole::Application, "Browser");
        let mut primary_window = node(2, SemanticRole::Window, "Primary");
        primary_window
            .states
            .push(SemanticState::Other("active".to_owned()));
        let mut primary = node(3, SemanticRole::Document, "Article");
        primary
            .states
            .push(SemanticState::Other("showing".to_owned()));
        primary
            .children
            .push(node(4, SemanticRole::Paragraph, "Body"));
        primary_window.children.push(primary);
        let mut secondary_window = node(5, SemanticRole::Window, "Auxiliary");
        let mut secondary = node(6, SemanticRole::Document, "Suggestions");
        secondary
            .states
            .push(SemanticState::Other("showing".to_owned()));
        secondary
            .children
            .push(node(7, SemanticRole::Paragraph, "Suggestion"));
        secondary_window.children.push(secondary);
        app.children.extend([primary_window, secondary_window]);
        let cache = SemanticCache::from_snapshot(app).unwrap();
        let catalog = ContentCatalog::analyze(&cache);
        assert_eq!(catalog.models().count(), 2);
        assert_eq!(catalog.visible_models().count(), 1);
        assert_eq!(
            catalog
                .visible_models()
                .next()
                .unwrap()
                .metadata
                .title
                .as_deref(),
            Some("Article")
        );
    }

    #[test]
    fn read_only_multiline_text_is_content_without_a_document_role() {
        let mut text = node(7, SemanticRole::TextInput, "Rich article");
        text.value = Some("First paragraph.\n\nSecond paragraph.".to_owned());
        text.states
            .push(SemanticState::Other("multi-line".to_owned()));
        text.states
            .push(SemanticState::Other("read-only".to_owned()));
        text.debug.interfaces = vec!["Text".to_owned(), "EditableText".to_owned()];
        let cache = SemanticCache::from_snapshot(text).unwrap();
        let catalog = ContentCatalog::analyze(&cache);
        let model = catalog.models().next().unwrap();
        assert_eq!(model.kind, ContentKind::RichText);
        assert!(matches!(model.blocks[0].kind, ContentBlockKind::Text));
        assert!(model.navigation.form_fields.is_empty());
    }
}
