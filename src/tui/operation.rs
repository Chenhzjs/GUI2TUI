use thiserror::Error;

use crate::{
    semantic::{BackendLocator, RuntimeNodeId, SemanticAction, SemanticCache, SemanticCapability},
    transcompile::{ChoiceSelectionStrategy, TuiScene},
};

use super::action::{ActionResolutionError, UiIntent, resolve_action};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticOperation {
    ActivateNode(RuntimeNodeId),
    ToggleNode(RuntimeNodeId),
    SelectNode(RuntimeNodeId),
    OpenMenu(RuntimeNodeId),
    ClosePopup(RuntimeNodeId),
    ReplaceText { target: RuntimeNodeId, text: String },
}

impl SemanticOperation {
    pub fn from_intent(runtime_id: RuntimeNodeId, intent: UiIntent) -> Option<Self> {
        match intent {
            UiIntent::Activate => Some(Self::ActivateNode(runtime_id)),
            UiIntent::Toggle => Some(Self::ToggleNode(runtime_id)),
            UiIntent::Select => Some(Self::SelectNode(runtime_id)),
            UiIntent::OpenMenu => Some(Self::OpenMenu(runtime_id)),
            UiIntent::ClosePopup => Some(Self::ClosePopup(runtime_id)),
            _ => None,
        }
    }

    fn runtime_id(&self) -> RuntimeNodeId {
        match self {
            Self::ActivateNode(id)
            | Self::ToggleNode(id)
            | Self::SelectNode(id)
            | Self::OpenMenu(id)
            | Self::ClosePopup(id) => *id,
            Self::ReplaceText { target, .. } => *target,
        }
    }

    fn intent(&self) -> UiIntent {
        match self {
            Self::ActivateNode(_) => UiIntent::Activate,
            Self::ToggleNode(_) => UiIntent::Toggle,
            Self::SelectNode(_) => UiIntent::Select,
            Self::OpenMenu(_) => UiIntent::OpenMenu,
            Self::ClosePopup(_) => UiIntent::ClosePopup,
            Self::ReplaceText { .. } => UiIntent::CommitEdit,
        }
    }
}

/// Resolve an operation directly against the canonical semantic runtime.
/// This is used for contextual owner operations (for example closing a
/// ComboBox popup) whose owner is intentionally not interactive in the active
/// popup scene.
pub fn resolve_cached_node_operation(
    cache: &SemanticCache,
    operation: SemanticOperation,
) -> Result<BackendOperation, OperationResolutionError> {
    let runtime_id = operation.runtime_id();
    let node = cache
        .node(runtime_id)
        .ok_or(OperationResolutionError::NodeNotFound(runtime_id))?;
    if matches!(
        operation,
        SemanticOperation::SelectNode(_) | SemanticOperation::ReplaceText { .. }
    ) {
        return Err(OperationResolutionError::NoCompatibleOperation(
            "operation requires scene relationship context".to_owned(),
        ));
    }
    let action = resolve_action(&node.role, &node.actions, operation.intent())
        .map_err(action_error)?
        .clone();
    Ok(BackendOperation::InvokeAction {
        locator: node.backend_locator.clone(),
        action,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendOperation {
    InvokeAction {
        locator: BackendLocator,
        action: SemanticAction,
    },
    SelectChild {
        container_locator: BackendLocator,
        child_index: usize,
    },
    SetTextContents {
        locator: BackendLocator,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionStrategy {
    NodeAction {
        action: SemanticAction,
    },
    ParentSelection {
        container_locator: BackendLocator,
        child_index: usize,
    },
    Unsupported,
}

pub fn resolve_choice_backend_operation(
    cache: &SemanticCache,
    strategy: &ChoiceSelectionStrategy,
) -> Result<BackendOperation, OperationResolutionError> {
    match strategy {
        ChoiceSelectionStrategy::ChildSemanticAction { child, action } => {
            let node = cache
                .node(*child)
                .ok_or(OperationResolutionError::NodeNotFound(*child))?;
            if !node.actions.iter().any(|advertised| advertised == action)
                || action.name.trim().is_empty()
            {
                return Err(OperationResolutionError::NoCompatibleOperation(
                    "choice action is no longer safely advertised".to_owned(),
                ));
            }
            Ok(BackendOperation::InvokeAction {
                locator: node.backend_locator.clone(),
                action: action.clone(),
            })
        }
        ChoiceSelectionStrategy::ParentSelection {
            parent,
            child_index,
        } => {
            let node = cache
                .node(*parent)
                .ok_or(OperationResolutionError::NodeNotFound(*parent))?;
            if !node
                .capabilities
                .contains(&SemanticCapability::SelectChildren)
            {
                return Err(OperationResolutionError::NoCompatibleOperation(
                    "choice parent no longer exposes Selection".to_owned(),
                ));
            }
            Ok(BackendOperation::SelectChild {
                container_locator: node.backend_locator.clone(),
                child_index: *child_index,
            })
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OperationResolutionError {
    #[error("semantic node {0} is not present in the current TUI snapshot")]
    NodeNotFound(RuntimeNodeId),
    #[error("no compatible semantic operation is available: {0}")]
    NoCompatibleOperation(String),
}

pub fn resolve_backend_operation(
    scene: &TuiScene,
    operation: SemanticOperation,
) -> Result<BackendOperation, OperationResolutionError> {
    let runtime_id = operation.runtime_id();
    let scene_id = scene
        .scene_id_for_runtime(runtime_id)
        .ok_or(OperationResolutionError::NodeNotFound(runtime_id))?;
    let element = scene
        .element(scene_id)
        .ok_or(OperationResolutionError::NodeNotFound(runtime_id))?;
    let binding = element
        .binding
        .as_ref()
        .ok_or(OperationResolutionError::NodeNotFound(runtime_id))?;

    if let SemanticOperation::ReplaceText { text, .. } = &operation {
        if binding.capability != super::action::InteractionCapability::EditText {
            return Err(OperationResolutionError::NoCompatibleOperation(
                "the text input is not a plain editable AT-SPI control".to_owned(),
            ));
        }
        return Ok(BackendOperation::SetTextContents {
            locator: binding.backend_locator.clone(),
            text: text.clone(),
        });
    }

    if matches!(operation, SemanticOperation::SelectNode(_)) {
        return match resolve_selection_strategy(scene, runtime_id) {
            SelectionStrategy::NodeAction { action } => Ok(BackendOperation::InvokeAction {
                locator: binding.backend_locator.clone(),
                action,
            }),
            SelectionStrategy::ParentSelection {
                container_locator,
                child_index,
            } => Ok(BackendOperation::SelectChild {
                container_locator,
                child_index,
            }),
            SelectionStrategy::Unsupported => Err(OperationResolutionError::NoCompatibleOperation(
                "the list item has neither a compatible action nor a selectable parent".to_owned(),
            )),
        };
    }

    let action = resolve_action(&binding.semantic_role, &binding.actions, operation.intent())
        .map_err(action_error)?
        .clone();
    Ok(BackendOperation::InvokeAction {
        locator: binding.backend_locator.clone(),
        action,
    })
}

pub fn resolve_selection_strategy(
    scene: &TuiScene,
    runtime_id: RuntimeNodeId,
) -> SelectionStrategy {
    let Some(scene_id) = scene.scene_id_for_runtime(runtime_id) else {
        return SelectionStrategy::Unsupported;
    };
    let Some(element) = scene.element(scene_id) else {
        return SelectionStrategy::Unsupported;
    };
    let Some(binding) = element.binding.as_ref() else {
        return SelectionStrategy::Unsupported;
    };

    if let Ok(action) = resolve_action(&binding.semantic_role, &binding.actions, UiIntent::Select) {
        return SelectionStrategy::NodeAction {
            action: action.clone(),
        };
    }

    let Some(context) = scene.node_context(runtime_id) else {
        return SelectionStrategy::Unsupported;
    };
    let (Some(parent_id), Some(child_index)) = (context.parent_id, context.index_in_parent) else {
        return SelectionStrategy::Unsupported;
    };
    let Some(parent) = scene.node_metadata(parent_id) else {
        return SelectionStrategy::Unsupported;
    };
    if parent
        .capabilities
        .contains(&SemanticCapability::SelectChildren)
    {
        SelectionStrategy::ParentSelection {
            container_locator: parent.backend_locator.clone(),
            child_index,
        }
    } else {
        SelectionStrategy::Unsupported
    }
}

fn action_error(error: ActionResolutionError) -> OperationResolutionError {
    OperationResolutionError::NoCompatibleOperation(error.to_string())
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        DebugInfo, SemanticNode, SemanticRole, SemanticState, TextInputKind, TreeTruncation,
    };

    use super::*;
    use crate::transcompile::compile_legacy_scene;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None::<TextInputKind>,
            states: Vec::<SemanticState>::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::<TreeTruncation>::new(),
            debug: DebugInfo::default(),
        }
    }

    fn action(name: &str) -> SemanticAction {
        SemanticAction {
            index: 0,
            name: name.to_owned(),
            description: None,
            keybinding: None,
        }
    }

    #[test]
    fn nested_selection_uses_parent_container_and_original_child_index() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut list = node(1, SemanticRole::List, "Items");
        list.capabilities.push(SemanticCapability::SelectChildren);
        let mut alpha = node(2, SemanticRole::ListItem, "Alpha");
        alpha.index_in_parent = Some(7);
        alpha.actions.push(action("listitem.scroll-to"));
        list.children.push(alpha);
        root.children.push(list);

        let scene = compile_legacy_scene(&root);
        assert_eq!(
            resolve_selection_strategy(&scene, RuntimeNodeId::new(2)),
            SelectionStrategy::ParentSelection {
                container_locator: BackendLocator::new(":1.2", "/node/1"),
                child_index: 7,
            }
        );
    }

    #[test]
    fn toggle_action_resolves_to_select_not_toggle_semantics() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut item = node(1, SemanticRole::ListItem, "Beta");
        item.index_in_parent = Some(1);
        item.actions.push(action("Toggle"));
        root.children.push(item);
        let scene = compile_legacy_scene(&root);

        assert_eq!(
            SemanticOperation::from_intent(RuntimeNodeId::new(1), UiIntent::Select),
            Some(SemanticOperation::SelectNode(RuntimeNodeId::new(1)))
        );
        assert!(matches!(
            resolve_selection_strategy(&scene, RuntimeNodeId::new(1)),
            SelectionStrategy::NodeAction { action } if action.name == "Toggle"
        ));
    }

    #[test]
    fn unsafe_actions_never_become_a_backend_operation() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut button = node(1, SemanticRole::Button, "Danger");
        button.actions.push(action("delete"));
        root.children.push(button);
        let scene = compile_legacy_scene(&root);

        assert!(matches!(
            resolve_backend_operation(
                &scene,
                SemanticOperation::ActivateNode(RuntimeNodeId::new(1))
            ),
            Err(OperationResolutionError::NoCompatibleOperation(_))
        ));
    }

    #[test]
    fn replace_text_maps_to_an_explicit_backend_operation() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut input = node(1, SemanticRole::TextInput, "Username");
        input.capabilities.push(SemanticCapability::EditText);
        root.children.push(input);
        let scene = compile_legacy_scene(&root);
        assert_eq!(
            resolve_backend_operation(
                &scene,
                SemanticOperation::ReplaceText {
                    target: RuntimeNodeId::new(1),
                    text: "updated".to_owned(),
                }
            ),
            Ok(BackendOperation::SetTextContents {
                locator: BackendLocator::new(":1.2", "/node/1"),
                text: "updated".to_owned(),
            })
        );
    }
}
