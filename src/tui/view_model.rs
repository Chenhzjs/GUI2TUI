use std::collections::HashMap;

use crate::semantic::{
    BackendLocator, RuntimeNodeId, SemanticAction, SemanticCapability, SemanticNode, SemanticRole,
    SemanticState, TextInputKind,
};

use super::action::{InteractionCapability, interaction_capability};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TuiElementKind {
    Label { text: String },
    Group { label: String },
    Button { label: String },
    ToggleButton { label: String, pressed: bool },
    CheckBox { label: String, checked: bool },
    TextInput { label: String, display: String },
    ComboBox { label: String },
    List { label: String },
    ListItem { label: String, selected: bool },
    MenuBar,
    Menu { label: String },
    MenuItem { label: String, opens_menu: bool },
    Unsupported { role: String, label: Option<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeContext {
    pub parent_id: Option<RuntimeNodeId>,
    pub index_in_parent: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeMetadata {
    pub backend_locator: BackendLocator,
    pub capabilities: Vec<SemanticCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuiElement {
    pub runtime_id: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub semantic_role: SemanticRole,
    pub kind: TuiElementKind,
    pub actions: Vec<SemanticAction>,
    pub capability: InteractionCapability,
}

impl TuiElement {
    pub fn is_focusable(&self) -> bool {
        matches!(
            self.kind,
            TuiElementKind::Button { .. }
                | TuiElementKind::ToggleButton { .. }
                | TuiElementKind::CheckBox { .. }
                | TuiElementKind::TextInput { .. }
                | TuiElementKind::ComboBox { .. }
                | TuiElementKind::ListItem { .. }
                | TuiElementKind::MenuItem { .. }
        )
    }

    pub fn height(&self) -> u16 {
        if matches!(self.kind, TuiElementKind::TextInput { .. }) {
            2
        } else {
            1
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuiViewModel {
    pub title: String,
    pub elements: Vec<TuiElement>,
    node_contexts: HashMap<RuntimeNodeId, NodeContext>,
    node_metadata: HashMap<RuntimeNodeId, NodeMetadata>,
}

impl TuiViewModel {
    pub fn from_snapshot(root: &SemanticNode) -> Self {
        let mut title = root
            .name
            .clone()
            .unwrap_or_else(|| "Accessible application".to_owned());
        let mut elements = Vec::new();
        let mut node_contexts = HashMap::new();
        let mut node_metadata = HashMap::new();
        index_nodes(root, None, &mut node_contexts, &mut node_metadata);
        map_node(root, &[], &mut title, &mut elements);
        remove_redundant_input_labels(&mut elements);
        Self {
            title,
            elements,
            node_contexts,
            node_metadata,
        }
    }

    pub fn element(&self, id: RuntimeNodeId) -> Option<&TuiElement> {
        self.elements
            .iter()
            .find(|element| element.runtime_id == id)
    }

    pub fn focusable_ids(&self) -> Vec<RuntimeNodeId> {
        self.elements
            .iter()
            .filter(|element| element.is_focusable())
            .map(|element| element.runtime_id)
            .collect()
    }

    pub fn runtime_id_for_locator(&self, locator: &BackendLocator) -> Option<RuntimeNodeId> {
        self.elements
            .iter()
            .find(|element| &element.backend_locator == locator && element.is_focusable())
            .map(|element| element.runtime_id)
    }

    pub fn row_span(&self, id: RuntimeNodeId) -> Option<(u16, u16)> {
        let mut top = 0_u16;
        for element in &self.elements {
            let height = element.height();
            if element.runtime_id == id {
                return Some((top, height));
            }
            top = top.saturating_add(height);
        }
        None
    }

    pub fn content_height(&self) -> u16 {
        self.elements.iter().fold(0_u16, |height, element| {
            height.saturating_add(element.height())
        })
    }

    pub fn node_context(&self, id: RuntimeNodeId) -> Option<NodeContext> {
        self.node_contexts.get(&id).copied()
    }

    pub fn node_metadata(&self, id: RuntimeNodeId) -> Option<&NodeMetadata> {
        self.node_metadata.get(&id)
    }
}

fn index_nodes(
    node: &SemanticNode,
    parent_id: Option<RuntimeNodeId>,
    contexts: &mut HashMap<RuntimeNodeId, NodeContext>,
    metadata: &mut HashMap<RuntimeNodeId, NodeMetadata>,
) {
    contexts.insert(
        node.runtime_id,
        NodeContext {
            parent_id,
            index_in_parent: node.index_in_parent,
        },
    );
    metadata.insert(
        node.runtime_id,
        NodeMetadata {
            backend_locator: node.backend_locator.clone(),
            capabilities: node.capabilities.clone(),
        },
    );
    for child in &node.children {
        index_nodes(child, Some(node.runtime_id), contexts, metadata);
    }
}

fn map_node(
    node: &SemanticNode,
    parent_capabilities: &[SemanticCapability],
    title: &mut String,
    output: &mut Vec<TuiElement>,
) {
    let terminal_leaf = matches!(
        node.role,
        SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::RadioButton
            | SemanticRole::TextInput
            | SemanticRole::ListItem
    );
    let kind = match &node.role {
        SemanticRole::Application => None,
        SemanticRole::Window => {
            if let Some(name) = &node.name {
                *title = name.clone();
            }
            None
        }
        SemanticRole::Container => node.name.as_ref().map(|label| TuiElementKind::Group {
            label: label.clone(),
        }),
        SemanticRole::Label | SemanticRole::Text => {
            visible_text(node).map(|text| TuiElementKind::Label { text })
        }
        SemanticRole::Button => Some(TuiElementKind::Button {
            label: node.name.clone().unwrap_or_else(|| "Button".to_owned()),
        }),
        SemanticRole::ToggleButton => Some(TuiElementKind::ToggleButton {
            label: node
                .name
                .clone()
                .unwrap_or_else(|| "Toggle button".to_owned()),
            pressed: node.states.contains(&SemanticState::Pressed)
                || node.states.contains(&SemanticState::Checked),
        }),
        SemanticRole::CheckBox => Some(TuiElementKind::CheckBox {
            label: node.name.clone().unwrap_or_else(|| "Checkbox".to_owned()),
            checked: node.states.contains(&SemanticState::Checked),
        }),
        SemanticRole::TextInput => Some(TuiElementKind::TextInput {
            label: node.name.clone().unwrap_or_else(|| "Text input".to_owned()),
            display: if node.text_input_kind == Some(TextInputKind::Password) {
                "[password]".to_owned()
            } else {
                node.value.clone().unwrap_or_else(|| "[empty]".to_owned())
            },
        }),
        SemanticRole::ComboBox => Some(TuiElementKind::ComboBox {
            label: node.name.clone().unwrap_or_else(|| "Combo box".to_owned()),
        }),
        SemanticRole::List => Some(TuiElementKind::List {
            label: node.name.clone().unwrap_or_else(|| "List".to_owned()),
        }),
        SemanticRole::ListItem => Some(TuiElementKind::ListItem {
            label: node
                .name
                .clone()
                .or_else(|| first_descendant_text(node))
                .unwrap_or_else(|| "List item".to_owned()),
            selected: node.states.contains(&SemanticState::Selected),
        }),
        SemanticRole::MenuBar => Some(TuiElementKind::MenuBar),
        SemanticRole::Menu => Some(TuiElementKind::Menu {
            label: node.name.clone().unwrap_or_else(|| "Menu".to_owned()),
        }),
        SemanticRole::MenuItem => {
            let capability = interaction_capability(&node.role, &node.actions, parent_capabilities);
            Some(TuiElementKind::MenuItem {
                label: node.name.clone().unwrap_or_else(|| "Menu item".to_owned()),
                opens_menu: capability == InteractionCapability::OpenMenu,
            })
        }
        role => Some(TuiElementKind::Unsupported {
            role: role.to_string(),
            label: node.name.clone(),
        }),
    };

    if let Some(kind) = kind {
        let capability = interaction_capability(&node.role, &node.actions, parent_capabilities);
        output.push(TuiElement {
            runtime_id: node.runtime_id,
            backend_locator: node.backend_locator.clone(),
            semantic_role: node.role.clone(),
            kind,
            actions: node.actions.clone(),
            capability,
        });
    }

    if !terminal_leaf {
        for child in &node.children {
            map_node(child, &node.capabilities, title, output);
        }
    }
}

fn visible_text(node: &SemanticNode) -> Option<String> {
    node.name.clone().or_else(|| node.value.clone())
}

fn first_descendant_text(node: &SemanticNode) -> Option<String> {
    node.children.iter().find_map(|child| {
        if matches!(child.role, SemanticRole::Label | SemanticRole::Text) {
            visible_text(child)
        } else {
            first_descendant_text(child)
        }
    })
}

fn remove_redundant_input_labels(elements: &mut Vec<TuiElement>) {
    let mut index = 0;
    while index + 1 < elements.len() {
        let redundant = matches!(
            (&elements[index].kind, &elements[index + 1].kind),
            (
                TuiElementKind::Label { text },
                TuiElementKind::TextInput { label, .. }
            ) if text == label
        );
        if redundant {
            elements.remove(index);
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{DebugInfo, SemanticState};

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
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
    fn maps_supported_widgets_and_focusability() {
        let mut root = node(0, SemanticRole::Window, "Settings");
        let label = node(1, SemanticRole::Label, "Status: idle");
        let mut checkbox = node(2, SemanticRole::CheckBox, "Enable feature");
        checkbox.states.push(SemanticState::Checked);
        let mut button = node(3, SemanticRole::Button, "Apply");
        button.actions.push(SemanticAction {
            index: 0,
            name: "Click".to_owned(),
            description: None,
            keybinding: None,
        });
        root.children = vec![label, checkbox, button];

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(view.title, "Settings");
        assert!(matches!(
            view.elements[0].kind,
            TuiElementKind::Label { .. }
        ));
        assert!(matches!(
            view.elements[1].kind,
            TuiElementKind::CheckBox { checked: true, .. }
        ));
        assert!(!view.elements[0].is_focusable());
        assert!(view.elements[1].is_focusable());
        assert!(view.elements[2].is_focusable());
        assert_eq!(view.elements[1].capability, InteractionCapability::None);
        assert_eq!(view.elements[2].capability, InteractionCapability::Activate);
    }

    #[test]
    fn indexes_child_parent_and_original_backend_position() {
        let mut root = node(0, SemanticRole::Window, "Settings");
        let mut list = node(1, SemanticRole::List, "Items");
        let mut item = node(2, SemanticRole::ListItem, "Beta");
        item.index_in_parent = Some(4);
        list.children.push(item);
        root.children.push(list);

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(
            view.node_context(RuntimeNodeId::new(2)),
            Some(NodeContext {
                parent_id: Some(RuntimeNodeId::new(1)),
                index_in_parent: Some(4),
            })
        );
    }

    #[test]
    fn maps_menu_items_without_conflating_open_and_activate() {
        let mut root = node(0, SemanticRole::Window, "Settings");
        let mut opener = node(1, SemanticRole::MenuItem, "Tools");
        opener.actions.push(SemanticAction {
            index: 0,
            name: "ShowMenu".to_owned(),
            description: None,
            keybinding: None,
        });
        let mut leaf = node(2, SemanticRole::MenuItem, "Activate Demo");
        leaf.actions.push(SemanticAction {
            index: 0,
            name: "Press".to_owned(),
            description: None,
            keybinding: None,
        });
        root.children = vec![opener, leaf];

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(view.elements[0].capability, InteractionCapability::OpenMenu);
        assert_eq!(view.elements[1].capability, InteractionCapability::Activate);
        assert!(matches!(
            view.elements[0].kind,
            TuiElementKind::MenuItem {
                opens_menu: true,
                ..
            }
        ));
    }

    #[test]
    fn password_is_redacted_before_reaching_renderer() {
        let mut root = node(0, SemanticRole::Window, "Login");
        let mut password = node(1, SemanticRole::TextInput, "Password");
        password.text_input_kind = Some(TextInputKind::Password);
        password.value = Some("must-never-render".to_owned());
        root.children.push(password);

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(
            view.elements[0].kind,
            TuiElementKind::TextInput {
                label: "Password".to_owned(),
                display: "[password]".to_owned(),
            }
        );
        assert!(!format!("{view:?}").contains("must-never-render"));
    }

    #[test]
    fn plain_text_input_retains_value_without_using_atspi_sensitive_state() {
        let mut root = node(0, SemanticRole::Window, "Login");
        let mut input = node(1, SemanticRole::TextInput, "Username");
        input.text_input_kind = Some(TextInputKind::Plain);
        input.value = Some("alice".to_owned());
        input
            .states
            .push(SemanticState::Other("sensitive".to_owned()));
        root.children.push(input);

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(
            view.elements[0].kind,
            TuiElementKind::TextInput {
                label: "Username".to_owned(),
                display: "alice".to_owned(),
            }
        );
    }

    #[test]
    fn control_child_labels_are_not_duplicated() {
        let mut root = node(0, SemanticRole::Window, "Fixture");
        let mut button = node(1, SemanticRole::Button, "Activate safely");
        button
            .children
            .push(node(2, SemanticRole::Label, "Activate safely"));
        root.children.push(button);

        let view = TuiViewModel::from_snapshot(&root);
        assert_eq!(view.elements.len(), 1);
        assert!(matches!(
            &view.elements[0].kind,
            TuiElementKind::Button { label } if label == "Activate safely"
        ));
    }

    #[test]
    fn unsupported_roles_remain_as_read_only_summaries() {
        let mut root = node(0, SemanticRole::Window, "Files");
        root.children.push(node(1, SemanticRole::Tree, "Folders"));

        let view = TuiViewModel::from_snapshot(&root);
        assert!(matches!(
            &view.elements[0].kind,
            TuiElementKind::Unsupported { role, label }
                if role == "Tree" && label.as_deref() == Some("Folders")
        ));
        assert!(!view.elements[0].is_focusable());
    }
}
