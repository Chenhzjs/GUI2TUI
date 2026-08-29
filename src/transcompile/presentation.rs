use std::collections::HashMap;

use clap::ValueEnum;

use crate::{
    semantic::{RuntimeNodeId, SemanticNode, SemanticRole, SemanticState, TextInputKind},
    tui::{
        action::{InteractionCapability, UiIntent},
        view_model::{TuiElementKind, TuiViewModel},
    },
};

use super::{
    analyze::RegionAnalysis,
    region::{ModalityPolicy, SemanticRegion, SemanticRegionKind},
    scene::{SceneBinding, SceneElement, SceneElementId, SceneElementKind, TuiScene},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum PresentationMode {
    Legacy,
    #[default]
    Transcompiled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStrategy {
    DirectWidget,
    LabeledField,
    Form,
    Selector,
    CommandList,
    CommandPalette,
    ReflowGroup,
    StatusLine,
    StructuredSummary,
    PreserveModality,
    Unsupported,
}

pub fn compile_scene(root: &SemanticNode, analysis: &RegionAnalysis) -> TuiScene {
    let nodes = index_by_runtime(root);
    let mut compiler = SceneCompiler {
        next_id: 0,
        nodes,
        elements: Vec::new(),
    };
    compiler.compile_region(&analysis.root);
    TuiScene::new(window_title(root), root, compiler.elements)
}

pub fn compile_legacy_scene(root: &SemanticNode) -> TuiScene {
    let legacy = TuiViewModel::from_snapshot(root);
    let elements = legacy
        .elements
        .into_iter()
        .enumerate()
        .map(|(index, element)| {
            let kind = match element.kind {
                TuiElementKind::Label { text } => SceneElementKind::Text { text },
                TuiElementKind::Group { label } => SceneElementKind::Group { label },
                TuiElementKind::Button { label } => SceneElementKind::Button { label },
                TuiElementKind::ToggleButton { label, pressed } => {
                    SceneElementKind::Toggle { label, pressed }
                }
                TuiElementKind::CheckBox { label, checked } => {
                    SceneElementKind::Checkbox { label, checked }
                }
                TuiElementKind::TextInput {
                    label,
                    display,
                    input_kind,
                } => SceneElementKind::Field {
                    label,
                    display,
                    input_kind,
                },
                TuiElementKind::ComboBox { label } => SceneElementKind::Selector { label },
                TuiElementKind::List { label } => SceneElementKind::Group { label },
                TuiElementKind::ListItem { label, selected } => {
                    SceneElementKind::SelectionItem { label, selected }
                }
                TuiElementKind::MenuBar => SceneElementKind::CommandHeader {
                    label: "Menu".to_owned(),
                },
                TuiElementKind::Menu { label } => SceneElementKind::Group { label },
                TuiElementKind::MenuItem { label, .. } => SceneElementKind::Command { path: label },
                TuiElementKind::Unsupported { role, label } => SceneElementKind::Unsupported {
                    label: label.map_or(role.clone(), |label| format!("{role} \"{label}\"")),
                },
            };
            let default_intent = intent_for_role(&element.semantic_role)
                .or_else(|| intent_for_capability(element.capability));
            let binding = default_intent.map(|default_intent| SceneBinding {
                runtime_id: element.runtime_id,
                backend_locator: element.backend_locator,
                semantic_role: element.semantic_role,
                actions: element.actions,
                capability: element.capability,
                default_intent,
            });
            SceneElement {
                id: SceneElementId::new(index as u64),
                kind,
                sources: vec![element.runtime_id],
                binding,
                strategy: PresentationStrategy::DirectWidget,
            }
        })
        .collect();
    TuiScene::new(legacy.title, root, elements)
}

struct SceneCompiler<'a> {
    next_id: u64,
    nodes: HashMap<RuntimeNodeId, &'a SemanticNode>,
    elements: Vec<SceneElement>,
}

impl SceneCompiler<'_> {
    fn id(&mut self) -> SceneElementId {
        let id = SceneElementId::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn push(
        &mut self,
        kind: SceneElementKind,
        sources: Vec<RuntimeNodeId>,
        binding: Option<SceneBinding>,
        strategy: PresentationStrategy,
    ) {
        let id = self.id();
        self.elements.push(SceneElement {
            id,
            kind,
            sources,
            binding,
            strategy,
        });
    }

    fn compile_region(&mut self, region: &SemanticRegion) {
        match region.kind {
            SemanticRegionKind::Control => self.compile_control(region, false),
            SemanticRegionKind::Field => self.compile_field(region),
            SemanticRegionKind::Form => {
                if let Some(label) = &region.label {
                    self.push(
                        SceneElementKind::Group {
                            label: label.clone(),
                        },
                        region.source_nodes.clone(),
                        None,
                        PresentationStrategy::Form,
                    );
                }
                for child in &region.children {
                    self.compile_region(child);
                }
            }
            SemanticRegionKind::Selection => {
                self.push(
                    SceneElementKind::Group {
                        label: region
                            .label
                            .clone()
                            .unwrap_or_else(|| "Selection".to_owned()),
                    },
                    region.source_nodes.clone(),
                    None,
                    PresentationStrategy::Selector,
                );
                for child in &region.children {
                    self.compile_selection_control(child);
                }
            }
            SemanticRegionKind::CommandSet => {
                if region.children.is_empty() {
                    return;
                }
                self.push(
                    SceneElementKind::CommandHeader {
                        label: region
                            .label
                            .clone()
                            .unwrap_or_else(|| "Commands".to_owned()),
                    },
                    region.source_nodes.clone(),
                    None,
                    PresentationStrategy::CommandList,
                );
                for child in &region.children {
                    self.compile_control(child, true);
                }
            }
            SemanticRegionKind::Status => {
                self.push(
                    SceneElementKind::Status {
                        text: region.label.clone().unwrap_or_else(|| "Status".to_owned()),
                    },
                    region.source_nodes.clone(),
                    None,
                    PresentationStrategy::StatusLine,
                );
                for child in &region.children {
                    self.compile_region(child);
                }
            }
            SemanticRegionKind::Content => {
                if let Some(text) = &region.label {
                    self.push(
                        SceneElementKind::Text { text: text.clone() },
                        region.source_nodes.clone(),
                        None,
                        PresentationStrategy::StructuredSummary,
                    );
                }
            }
            SemanticRegionKind::OpaqueContent => {
                let dimensions = region
                    .source_nodes
                    .first()
                    .and_then(|id| self.nodes.get(id))
                    .and_then(|node| node.debug.geometry.as_ref())
                    .map(|geometry| (geometry.width, geometry.height));
                debug_assert_eq!(region.modality, ModalityPolicy::FidelityPreferred);
                self.push(
                    SceneElementKind::OpaqueContent {
                        label: region
                            .label
                            .clone()
                            .unwrap_or_else(|| "Graphical content".to_owned()),
                        dimensions,
                    },
                    region.source_nodes.clone(),
                    None,
                    PresentationStrategy::PreserveModality,
                );
            }
            SemanticRegionKind::Navigation | SemanticRegionKind::Group => {
                if region.kind == SemanticRegionKind::Group
                    && let Some(label) = &region.label
                {
                    self.push(
                        SceneElementKind::Group {
                            label: label.clone(),
                        },
                        region.source_nodes.clone(),
                        None,
                        PresentationStrategy::ReflowGroup,
                    );
                }
                for child in &region.children {
                    self.compile_region(child);
                }
            }
            SemanticRegionKind::Unknown => {
                let label = region.label.clone().unwrap_or_else(|| {
                    region
                        .source_nodes
                        .first()
                        .and_then(|id| self.nodes.get(id))
                        .map_or_else(|| "Unknown".to_owned(), |node| node.role.to_string())
                });
                self.push(
                    SceneElementKind::Unsupported { label },
                    region.source_nodes.clone(),
                    None,
                    PresentationStrategy::Unsupported,
                );
                for child in &region.children {
                    self.compile_region(child);
                }
            }
        }
    }

    fn compile_field(&mut self, region: &SemanticRegion) {
        let Some(control) = region.children.first() else {
            return;
        };
        let Some(node) = control
            .source_nodes
            .first()
            .and_then(|id| self.nodes.get(id))
        else {
            return;
        };
        let binding = self.binding(node, control);
        let input_kind = node.text_input_kind.unwrap_or(TextInputKind::Plain);
        let display = if input_kind == TextInputKind::Password {
            "[password]".to_owned()
        } else {
            node.value.clone().unwrap_or_else(|| "[empty]".to_owned())
        };
        self.push(
            SceneElementKind::Field {
                label: region
                    .label
                    .clone()
                    .or_else(|| node.name.clone())
                    .unwrap_or_else(|| "Field".to_owned()),
                display,
                input_kind,
            },
            region.source_nodes.clone(),
            binding,
            PresentationStrategy::LabeledField,
        );
    }

    fn compile_selection_control(&mut self, region: &SemanticRegion) {
        let Some(node) = region
            .source_nodes
            .first()
            .and_then(|id| self.nodes.get(id))
        else {
            return;
        };
        let selected = node.states.contains(&SemanticState::Selected);
        self.push(
            SceneElementKind::SelectionItem {
                label: node_label(node).unwrap_or_else(|| "Item".to_owned()),
                selected,
            },
            region.source_nodes.clone(),
            self.binding(node, region),
            PresentationStrategy::Selector,
        );
    }

    fn compile_control(&mut self, region: &SemanticRegion, command: bool) {
        let Some(node) = region
            .source_nodes
            .first()
            .and_then(|id| self.nodes.get(id))
        else {
            return;
        };
        if node.role == SemanticRole::TextInput {
            let input_kind = node.text_input_kind.unwrap_or(TextInputKind::Plain);
            self.push(
                SceneElementKind::Field {
                    label: node.name.clone().unwrap_or_else(|| "Text input".to_owned()),
                    display: if input_kind == TextInputKind::Password {
                        "[password]".to_owned()
                    } else {
                        node.value.clone().unwrap_or_else(|| "[empty]".to_owned())
                    },
                    input_kind,
                },
                region.source_nodes.clone(),
                self.binding(node, region),
                PresentationStrategy::DirectWidget,
            );
            return;
        }
        if command {
            let mut path = region.command_path.clone();
            if let Some(name) = &node.name {
                path.push(name.clone());
            }
            self.push(
                SceneElementKind::Command {
                    path: path.join(" › "),
                },
                region.source_nodes.clone(),
                self.binding(node, region),
                PresentationStrategy::CommandList,
            );
            return;
        }
        let label = node_label(node).unwrap_or_else(|| node.role.to_string());
        let kind = match node.role {
            SemanticRole::Button => SceneElementKind::Button { label },
            SemanticRole::ToggleButton => SceneElementKind::Toggle {
                label,
                pressed: node.states.contains(&SemanticState::Pressed)
                    || node.states.contains(&SemanticState::Checked),
            },
            SemanticRole::CheckBox | SemanticRole::RadioButton => SceneElementKind::Checkbox {
                label,
                checked: node.states.contains(&SemanticState::Checked),
            },
            SemanticRole::ComboBox => SceneElementKind::Selector { label },
            SemanticRole::ListItem => SceneElementKind::SelectionItem {
                label,
                selected: node.states.contains(&SemanticState::Selected),
            },
            SemanticRole::MenuItem => SceneElementKind::Command {
                path: if region.command_path.is_empty() {
                    label
                } else {
                    format!("{} › {label}", region.command_path.join(" › "))
                },
            },
            _ => SceneElementKind::Unsupported { label },
        };
        self.push(
            kind,
            region.source_nodes.clone(),
            self.binding(node, region),
            PresentationStrategy::DirectWidget,
        );
    }

    fn binding(&self, node: &SemanticNode, region: &SemanticRegion) -> Option<SceneBinding> {
        let interaction = region
            .interactions
            .iter()
            .find(|interaction| interaction.source == node.runtime_id);
        let default_intent = interaction
            .map(|interaction| interaction.intent)
            .or_else(|| intent_for_role(&node.role))?;
        let capability = interaction.map_or(InteractionCapability::None, |interaction| {
            capability_for_intent(interaction.intent)
        });
        Some(SceneBinding {
            runtime_id: node.runtime_id,
            backend_locator: node.backend_locator.clone(),
            semantic_role: node.role.clone(),
            actions: node.actions.clone(),
            capability,
            default_intent,
        })
    }
}

fn index_by_runtime(root: &SemanticNode) -> HashMap<RuntimeNodeId, &SemanticNode> {
    fn visit<'a>(node: &'a SemanticNode, output: &mut HashMap<RuntimeNodeId, &'a SemanticNode>) {
        output.insert(node.runtime_id, node);
        for child in &node.children {
            visit(child, output);
        }
    }
    let mut output = HashMap::new();
    visit(root, &mut output);
    output
}

fn window_title(root: &SemanticNode) -> String {
    fn find(node: &SemanticNode) -> Option<String> {
        if matches!(node.role, SemanticRole::Window | SemanticRole::Dialog) && node.name.is_some() {
            return node.name.clone();
        }
        node.children.iter().find_map(find)
    }
    find(root)
        .or_else(|| root.name.clone())
        .unwrap_or_else(|| "Accessible application".to_owned())
}

fn node_label(node: &SemanticNode) -> Option<String> {
    node.name.clone().or_else(|| {
        let mut labels = Vec::new();
        collect_text_labels(node, &mut labels);
        labels.sort();
        labels.dedup();
        (labels.len() == 1).then(|| labels[0].clone())
    })
}

fn collect_text_labels(node: &SemanticNode, labels: &mut Vec<String>) {
    for child in &node.children {
        if matches!(child.role, SemanticRole::Label | SemanticRole::Text) {
            if let Some(label) = child.name.clone().or_else(|| child.value.clone()) {
                labels.push(label);
            }
        } else if !matches!(
            child.role,
            SemanticRole::Button
                | SemanticRole::ToggleButton
                | SemanticRole::CheckBox
                | SemanticRole::RadioButton
                | SemanticRole::TextInput
                | SemanticRole::ComboBox
                | SemanticRole::ListItem
                | SemanticRole::MenuItem
        ) {
            collect_text_labels(child, labels);
        }
    }
}

fn intent_for_capability(capability: InteractionCapability) -> Option<UiIntent> {
    match capability {
        InteractionCapability::None => None,
        InteractionCapability::Activate => Some(UiIntent::Activate),
        InteractionCapability::Toggle => Some(UiIntent::Toggle),
        InteractionCapability::Select => Some(UiIntent::Select),
        InteractionCapability::OpenMenu => Some(UiIntent::OpenMenu),
        InteractionCapability::EditText => Some(UiIntent::BeginEdit),
    }
}

fn intent_for_role(role: &SemanticRole) -> Option<UiIntent> {
    match role {
        SemanticRole::TextInput => Some(UiIntent::BeginEdit),
        SemanticRole::Button => Some(UiIntent::Activate),
        SemanticRole::ToggleButton | SemanticRole::CheckBox | SemanticRole::RadioButton => {
            Some(UiIntent::Toggle)
        }
        SemanticRole::ListItem => Some(UiIntent::Select),
        SemanticRole::MenuItem => Some(UiIntent::Activate),
        SemanticRole::ComboBox => Some(UiIntent::Activate),
        _ => None,
    }
}

fn capability_for_intent(intent: UiIntent) -> InteractionCapability {
    match intent {
        UiIntent::Activate => InteractionCapability::Activate,
        UiIntent::Toggle => InteractionCapability::Toggle,
        UiIntent::Select => InteractionCapability::Select,
        UiIntent::OpenMenu => InteractionCapability::OpenMenu,
        UiIntent::BeginEdit => InteractionCapability::EditText,
        _ => InteractionCapability::None,
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticAction, SemanticCapability};

    use super::*;
    use crate::transcompile::{SemanticRegionKind, analyze_regions};

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
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn field_scene_has_unique_identity_and_preserves_runtime_binding() {
        let mut root = node(0, SemanticRole::Window, "Profile");
        let label = node(1, SemanticRole::Label, "Username");
        let mut input = node(2, SemanticRole::TextInput, "Username");
        input.text_input_kind = Some(TextInputKind::Plain);
        input.value = Some("alice".to_owned());
        input.capabilities.push(SemanticCapability::EditText);
        root.children = vec![label, input];
        let analysis = analyze_regions(&root);
        let scene = compile_scene(&root, &analysis);
        let field = scene
            .elements
            .iter()
            .find(|element| matches!(element.kind, SceneElementKind::Field { .. }))
            .unwrap();
        assert_eq!(
            field.binding.as_ref().unwrap().runtime_id,
            RuntimeNodeId::new(2)
        );
        assert_eq!(
            field.sources,
            vec![RuntimeNodeId::new(1), RuntimeNodeId::new(2)]
        );
    }

    #[test]
    fn form_compilation_removes_layout_nodes_and_assigns_unique_scene_ids() {
        let mut root = node(0, SemanticRole::Window, "Profile");
        let mut username = node(2, SemanticRole::TextInput, "Username");
        username.text_input_kind = Some(TextInputKind::Plain);
        let mut password = node(4, SemanticRole::TextInput, "Password");
        password.text_input_kind = Some(TextInputKind::Password);
        root.children = vec![
            node(1, SemanticRole::Label, "Username"),
            username,
            node(3, SemanticRole::Label, "Password"),
            password,
        ];

        let analysis = analyze_regions(&root);
        assert_eq!(analysis.root.kind, SemanticRegionKind::Form);
        let scene = compile_scene(&root, &analysis);
        assert_eq!(
            scene
                .elements
                .iter()
                .filter(|element| matches!(element.kind, SceneElementKind::Field { .. }))
                .count(),
            2
        );
        let mut ids = scene
            .elements
            .iter()
            .map(|element| element.id)
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), scene.elements.len());
    }

    #[test]
    fn command_set_compiles_to_palette_source_commands() {
        let mut root = node(0, SemanticRole::MenuBar, "Commands");
        let mut command = node(1, SemanticRole::MenuItem, "Save");
        command.actions.push(SemanticAction {
            index: 0,
            name: "Press".to_owned(),
            description: None,
            keybinding: None,
        });
        root.children.push(command);
        let analysis = analyze_regions(&root);
        assert_eq!(analysis.root.kind, SemanticRegionKind::CommandSet);
        let scene = compile_scene(&root, &analysis);
        assert_eq!(scene.metrics.commands, 1);
        assert_eq!(scene.commands().count(), 1);
    }

    #[test]
    fn opaque_region_compiles_to_preserve_modality_block() {
        let mut root = node(
            0,
            SemanticRole::Unknown("drawing area".to_owned()),
            "Preview",
        );
        root.debug.geometry = Some(crate::semantic::Geometry {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        });
        let scene = compile_scene(&root, &analyze_regions(&root));
        assert!(matches!(
            scene.elements[0].kind,
            SceneElementKind::OpaqueContent {
                dimensions: Some((640, 480)),
                ..
            }
        ));
        assert_eq!(
            scene.elements[0].strategy,
            PresentationStrategy::PreserveModality
        );
    }
}
