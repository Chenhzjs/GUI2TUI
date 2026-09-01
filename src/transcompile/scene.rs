use std::{collections::HashMap, fmt};

use crate::{
    semantic::{
        BackendLocator, RuntimeNodeId, SemanticAction, SemanticCapability, SemanticNode,
        SemanticRole, TextInputKind,
    },
    tui::action::{InteractionCapability, UiIntent},
};

use super::presentation::PresentationStrategy;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneElementId(u64);

impl SceneElementId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SceneElementId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneBinding {
    pub runtime_id: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub semantic_role: SemanticRole,
    pub actions: Vec<SemanticAction>,
    pub capability: InteractionCapability,
    pub default_intent: UiIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneElementKind {
    Text {
        text: String,
    },
    Group {
        label: String,
    },
    Field {
        label: String,
        display: String,
        input_kind: TextInputKind,
    },
    Button {
        label: String,
    },
    Toggle {
        label: String,
        pressed: bool,
    },
    Checkbox {
        label: String,
        checked: bool,
    },
    Selector {
        label: String,
    },
    DocumentSummary {
        title: String,
        blocks: usize,
        headings: usize,
        links: usize,
        forms: usize,
        completeness: String,
    },
    SelectionItem {
        label: String,
        selected: bool,
    },
    CommandHeader {
        label: String,
    },
    Command {
        path: String,
    },
    Status {
        text: String,
    },
    Hint {
        text: String,
    },
    Error {
        text: String,
    },
    OpaqueContent {
        label: String,
        dimensions: Option<(i32, i32)>,
    },
    Unsupported {
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneElement {
    pub id: SceneElementId,
    pub kind: SceneElementKind,
    pub sources: Vec<RuntimeNodeId>,
    pub binding: Option<SceneBinding>,
    pub strategy: PresentationStrategy,
}

impl SceneElement {
    pub fn is_focusable(&self) -> bool {
        self.binding.is_some()
            && matches!(
                self.kind,
                SceneElementKind::Field { .. }
                    | SceneElementKind::Button { .. }
                    | SceneElementKind::Toggle { .. }
                    | SceneElementKind::Checkbox { .. }
                    | SceneElementKind::Selector { .. }
                    | SceneElementKind::DocumentSummary { .. }
                    | SceneElementKind::SelectionItem { .. }
                    | SceneElementKind::Command { .. }
            )
    }

    pub fn capability(&self) -> InteractionCapability {
        self.binding
            .as_ref()
            .map_or(InteractionCapability::None, |binding| binding.capability)
    }

    pub fn label(&self) -> &str {
        match &self.kind {
            SceneElementKind::Text { text }
            | SceneElementKind::Status { text }
            | SceneElementKind::Hint { text }
            | SceneElementKind::Error { text } => text,
            SceneElementKind::Group { label }
            | SceneElementKind::Field { label, .. }
            | SceneElementKind::Button { label }
            | SceneElementKind::Toggle { label, .. }
            | SceneElementKind::Checkbox { label, .. }
            | SceneElementKind::Selector { label }
            | SceneElementKind::SelectionItem { label, .. }
            | SceneElementKind::CommandHeader { label }
            | SceneElementKind::OpaqueContent { label, .. }
            | SceneElementKind::Unsupported { label } => label,
            SceneElementKind::DocumentSummary { title, .. } => title,
            SceneElementKind::Command { path } => path,
        }
    }

    pub fn height_for_width(&self, width: u16) -> u16 {
        match self.kind {
            SceneElementKind::Field { .. } if width < 100 => 2,
            SceneElementKind::OpaqueContent { dimensions, .. } if dimensions.is_some() => 3,
            SceneElementKind::OpaqueContent { .. } => 2,
            SceneElementKind::DocumentSummary { .. } => 7,
            SceneElementKind::Group { .. } | SceneElementKind::CommandHeader { .. } => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneMetrics {
    pub elements: usize,
    pub interactive_elements: usize,
    pub commands: usize,
    pub opaque: usize,
    pub unsupported: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneNodeContext {
    pub parent_id: Option<RuntimeNodeId>,
    pub index_in_parent: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneNodeMetadata {
    pub backend_locator: BackendLocator,
    pub capabilities: Vec<SemanticCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuiScene {
    pub title: String,
    pub elements: Vec<SceneElement>,
    pub metrics: SceneMetrics,
    node_contexts: HashMap<RuntimeNodeId, SceneNodeContext>,
    node_metadata: HashMap<RuntimeNodeId, SceneNodeMetadata>,
    by_runtime: HashMap<RuntimeNodeId, SceneElementId>,
}

impl TuiScene {
    pub fn new(title: String, root: &SemanticNode, elements: Vec<SceneElement>) -> Self {
        let mut node_contexts = HashMap::new();
        let mut node_metadata = HashMap::new();
        index_nodes(root, None, &mut node_contexts, &mut node_metadata);
        let by_runtime = elements
            .iter()
            .filter_map(|element| {
                element
                    .binding
                    .as_ref()
                    .map(|binding| (binding.runtime_id, element.id))
            })
            .collect();
        let metrics = SceneMetrics {
            elements: elements.len(),
            interactive_elements: elements
                .iter()
                .filter(|element| element.is_focusable())
                .count(),
            commands: elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::Command { .. }))
                .count(),
            opaque: elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::OpaqueContent { .. }))
                .count(),
            unsupported: elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::Unsupported { .. }))
                .count(),
        };
        Self {
            title,
            elements,
            metrics,
            node_contexts,
            node_metadata,
            by_runtime,
        }
    }

    pub fn element(&self, id: SceneElementId) -> Option<&SceneElement> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn element_mut(&mut self, id: SceneElementId) -> Option<&mut SceneElement> {
        self.elements.iter_mut().find(|element| element.id == id)
    }

    pub fn focusable_ids(&self) -> Vec<SceneElementId> {
        self.elements
            .iter()
            .filter(|element| element.is_focusable())
            .map(|element| element.id)
            .collect()
    }

    pub fn scene_id_for_locator(&self, locator: &BackendLocator) -> Option<SceneElementId> {
        self.elements.iter().find_map(|element| {
            element.binding.as_ref().and_then(|binding| {
                (&binding.backend_locator == locator && element.is_focusable())
                    .then_some(element.id)
            })
        })
    }

    pub fn scene_id_for_runtime(&self, runtime_id: RuntimeNodeId) -> Option<SceneElementId> {
        self.by_runtime.get(&runtime_id).copied()
    }

    pub fn row_span(&self, id: SceneElementId, width: u16) -> Option<(u16, u16)> {
        let mut top = 0_u16;
        for element in &self.elements {
            let height = element.height_for_width(width);
            if element.id == id {
                return Some((top, height));
            }
            top = top.saturating_add(height);
        }
        None
    }

    pub fn content_height(&self, width: u16) -> u16 {
        self.elements.iter().fold(0_u16, |height, element| {
            height.saturating_add(element.height_for_width(width))
        })
    }

    pub fn node_context(&self, id: RuntimeNodeId) -> Option<SceneNodeContext> {
        self.node_contexts.get(&id).copied()
    }

    pub fn node_metadata(&self, id: RuntimeNodeId) -> Option<&SceneNodeMetadata> {
        self.node_metadata.get(&id)
    }

    pub fn commands(&self) -> impl Iterator<Item = &SceneElement> {
        self.elements
            .iter()
            .filter(|element| matches!(element.kind, SceneElementKind::Command { .. }))
    }

    pub fn replace_elements(&mut self, elements: Vec<SceneElement>) {
        self.elements = elements;
        self.by_runtime = self
            .elements
            .iter()
            .filter_map(|element| {
                element
                    .binding
                    .as_ref()
                    .map(|binding| (binding.runtime_id, element.id))
            })
            .collect();
        self.metrics = SceneMetrics {
            elements: self.elements.len(),
            interactive_elements: self
                .elements
                .iter()
                .filter(|element| element.is_focusable())
                .count(),
            commands: self
                .elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::Command { .. }))
                .count(),
            opaque: self
                .elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::OpaqueContent { .. }))
                .count(),
            unsupported: self
                .elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::Unsupported { .. }))
                .count(),
        };
    }
}

fn index_nodes(
    node: &SemanticNode,
    parent_id: Option<RuntimeNodeId>,
    contexts: &mut HashMap<RuntimeNodeId, SceneNodeContext>,
    metadata: &mut HashMap<RuntimeNodeId, SceneNodeMetadata>,
) {
    contexts.insert(
        node.runtime_id,
        SceneNodeContext {
            parent_id,
            index_in_parent: node.index_in_parent,
        },
    );
    metadata.insert(
        node.runtime_id,
        SceneNodeMetadata {
            backend_locator: node.backend_locator.clone(),
            capabilities: node.capabilities.clone(),
        },
    );
    for child in &node.children {
        index_nodes(child, Some(node.runtime_id), contexts, metadata);
    }
}

pub fn format_scene(scene: &TuiScene) -> String {
    let mut output = format!("Scene title={:?}\n", scene.title);
    for element in &scene.elements {
        output.push_str(&format!(
            "  Element {} strategy={:?} kind={:?} sources={:?}",
            element.id,
            element.strategy,
            element.kind,
            element
                .sources
                .iter()
                .map(|id| id.get())
                .collect::<Vec<_>>()
        ));
        if let Some(binding) = &element.binding {
            output.push_str(&format!(
                " binding=runtime:{} intent={:?} capability={:?}",
                binding.runtime_id, binding.default_intent, binding.capability
            ));
        }
        output.push('\n');
    }
    output.push_str(&format!(
        "Metrics elements={} interactive={} commands={} opaque={} unsupported={}\n",
        scene.metrics.elements,
        scene.metrics.interactive_elements,
        scene.metrics.commands,
        scene.metrics.opaque,
        scene.metrics.unsupported,
    ));
    output
}
