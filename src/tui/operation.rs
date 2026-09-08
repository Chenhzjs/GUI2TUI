use thiserror::Error;

use crate::{
    semantic::{
        BackendLocator, RuntimeNodeId, SemanticAction, SemanticCache, SemanticCapability,
        SemanticRole, SemanticState,
    },
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
    ReplaceText {
        target: RuntimeNodeId,
        text: String,
    },
    ReplaceComplexText {
        target: RuntimeNodeId,
        expected: String,
        text: String,
    },
    AdjustValue {
        target: RuntimeNodeId,
        increase: bool,
    },
}

impl SemanticOperation {
    pub fn from_intent(runtime_id: RuntimeNodeId, intent: UiIntent) -> Option<Self> {
        match intent {
            UiIntent::Activate => Some(Self::ActivateNode(runtime_id)),
            UiIntent::Toggle => Some(Self::ToggleNode(runtime_id)),
            UiIntent::Select => Some(Self::SelectNode(runtime_id)),
            UiIntent::OpenMenu => Some(Self::OpenMenu(runtime_id)),
            UiIntent::ClosePopup => Some(Self::ClosePopup(runtime_id)),
            UiIntent::IncreaseValue => Some(Self::AdjustValue {
                target: runtime_id,
                increase: true,
            }),
            UiIntent::DecreaseValue => Some(Self::AdjustValue {
                target: runtime_id,
                increase: false,
            }),
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
            Self::ReplaceText { target, .. }
            | Self::ReplaceComplexText { target, .. }
            | Self::AdjustValue { target, .. } => *target,
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
            Self::ReplaceComplexText { .. } => UiIntent::BeginExternalEdit,
            Self::AdjustValue { increase, .. } => {
                if *increase {
                    UiIntent::IncreaseValue
                } else {
                    UiIntent::DecreaseValue
                }
            }
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
        SemanticOperation::SelectNode(_)
            | SemanticOperation::ReplaceText { .. }
            | SemanticOperation::ReplaceComplexText { .. }
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
    SelectCurrentItem {
        collection_locator: BackendLocator,
        target_locator: BackendLocator,
        target_role: SemanticRole,
        action: Option<SemanticAction>,
    },
    SelectCurrentTableRow {
        table_locator: BackendLocator,
        target_cell_locator: BackendLocator,
    },
    SetTextContents {
        locator: BackendLocator,
        text: String,
    },
    SetComplexTextContents {
        locator: BackendLocator,
        expected: String,
        text: String,
    },
    AdjustValue {
        locator: BackendLocator,
        increase: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionStrategy {
    NodeAction {
        collection_locator: BackendLocator,
        target_locator: BackendLocator,
        action: SemanticAction,
    },
    ParentSelection {
        collection_locator: BackendLocator,
        target_locator: BackendLocator,
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
            if node.role == SemanticRole::ListItem {
                let parent = node.parent.and_then(|id| cache.node(id)).ok_or_else(|| {
                    OperationResolutionError::NoCompatibleOperation(
                        "selection target has no current collection".to_owned(),
                    )
                })?;
                if parent.role != SemanticRole::List
                    || is_multiselectable(&parent.states)
                    || is_multiselectable(&node.states)
                    || !is_current_available_target(&node.states)
                {
                    return Err(OperationResolutionError::NoCompatibleOperation(
                        "choice target is not a current single-selection list member".to_owned(),
                    ));
                }
                Ok(BackendOperation::SelectCurrentItem {
                    collection_locator: parent.backend_locator.clone(),
                    target_locator: node.backend_locator.clone(),
                    target_role: node.role.clone(),
                    action: Some(action.clone()),
                })
            } else {
                Ok(BackendOperation::InvokeAction {
                    locator: node.backend_locator.clone(),
                    action: action.clone(),
                })
            }
        }
        ChoiceSelectionStrategy::ParentSelection { parent, child } => {
            let collection = cache
                .node(*parent)
                .ok_or(OperationResolutionError::NodeNotFound(*parent))?;
            let target = cache
                .node(*child)
                .ok_or(OperationResolutionError::NodeNotFound(*child))?;
            let required_capability = if target.role == SemanticRole::ListItem {
                SemanticCapability::SelectCurrentChild
            } else {
                SemanticCapability::SelectChildren
            };
            if target.parent != Some(*parent)
                || (target.role == SemanticRole::ListItem && collection.role != SemanticRole::List)
                || !collection.capabilities.contains(&required_capability)
                || is_multiselectable(&collection.states)
                || is_multiselectable(&target.states)
                || !is_current_available_target(&target.states)
            {
                return Err(OperationResolutionError::NoCompatibleOperation(
                    "choice target is no longer a current single-selection member".to_owned(),
                ));
            }
            Ok(BackendOperation::SelectCurrentItem {
                collection_locator: collection.backend_locator.clone(),
                target_locator: target.backend_locator.clone(),
                target_role: target.role.clone(),
                action: None,
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

    if let SemanticOperation::ReplaceComplexText { expected, text, .. } = &operation {
        let metadata = scene
            .node_metadata(runtime_id)
            .ok_or(OperationResolutionError::NodeNotFound(runtime_id))?;
        if !metadata
            .capabilities
            .contains(&SemanticCapability::EditComplexText)
        {
            return Err(OperationResolutionError::NoCompatibleOperation(
                "the text target is no longer qualified as complete writable plain text".to_owned(),
            ));
        }
        return Ok(BackendOperation::SetComplexTextContents {
            locator: binding.backend_locator.clone(),
            expected: expected.clone(),
            text: text.clone(),
        });
    }

    if let SemanticOperation::AdjustValue { increase, .. } = operation {
        if binding.capability != super::action::InteractionCapability::AdjustValue {
            return Err(OperationResolutionError::NoCompatibleOperation(
                "the focused control is not a qualified bounded Value".to_owned(),
            ));
        }
        return Ok(BackendOperation::AdjustValue {
            locator: binding.backend_locator.clone(),
            increase,
        });
    }

    if matches!(operation, SemanticOperation::SelectNode(_)) {
        return match resolve_selection_strategy(scene, runtime_id) {
            SelectionStrategy::NodeAction {
                collection_locator,
                target_locator,
                action,
            } => Ok(BackendOperation::SelectCurrentItem {
                collection_locator,
                target_locator,
                target_role: binding.semantic_role.clone(),
                action: Some(action),
            }),
            SelectionStrategy::ParentSelection {
                collection_locator,
                target_locator,
            } => Ok(BackendOperation::SelectCurrentItem {
                collection_locator,
                target_locator,
                target_role: binding.semantic_role.clone(),
                action: None,
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

    let Some(context) = scene.node_context(runtime_id) else {
        return SelectionStrategy::Unsupported;
    };
    let Some(parent_id) = context.parent_id else {
        return SelectionStrategy::Unsupported;
    };
    let Some(parent) = scene.node_metadata(parent_id) else {
        return SelectionStrategy::Unsupported;
    };
    let Some(target) = scene.node_metadata(runtime_id) else {
        return SelectionStrategy::Unsupported;
    };
    if binding.semantic_role != SemanticRole::ListItem
        || parent.role != SemanticRole::List
        || is_multiselectable(&parent.states)
        || is_multiselectable(&target.states)
        || !is_current_available_target(&target.states)
    {
        return SelectionStrategy::Unsupported;
    }
    if let Ok(action) = resolve_action(&binding.semantic_role, &binding.actions, UiIntent::Select) {
        return SelectionStrategy::NodeAction {
            collection_locator: parent.backend_locator.clone(),
            target_locator: binding.backend_locator.clone(),
            action: action.clone(),
        };
    }
    if parent
        .capabilities
        .contains(&SemanticCapability::SelectCurrentChild)
    {
        SelectionStrategy::ParentSelection {
            collection_locator: parent.backend_locator.clone(),
            target_locator: binding.backend_locator.clone(),
        }
    } else {
        SelectionStrategy::Unsupported
    }
}

fn is_multiselectable(states: &[SemanticState]) -> bool {
    states
        .iter()
        .any(|state| matches!(state, SemanticState::Other(value) if value == "multiselectable"))
}

fn is_current_available_target(states: &[SemanticState]) -> bool {
    let enabled = states.iter().any(|state| {
        matches!(state, SemanticState::Enabled)
            || matches!(state, SemanticState::Other(value) if value == "sensitive")
    });
    let showing = states
        .iter()
        .any(|state| matches!(state, SemanticState::Other(value) if value == "showing"));
    let visible = states
        .iter()
        .any(|state| matches!(state, SemanticState::Other(value) if value == "visible"));
    enabled && (showing || visible)
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
    fn gtk_style_selection_carries_exact_target_and_never_a_child_index() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut list = node(1, SemanticRole::List, "Items");
        list.capabilities
            .push(SemanticCapability::SelectCurrentChild);
        let mut alpha = node(2, SemanticRole::ListItem, "Alpha");
        alpha.index_in_parent = Some(7);
        alpha.states = vec![
            SemanticState::Enabled,
            SemanticState::Other("selectable".to_owned()),
            SemanticState::Other("showing".to_owned()),
        ];
        alpha.actions.push(action("listitem.scroll-to"));
        list.children.push(alpha);
        root.children.push(list);

        let scene = compile_legacy_scene(&root);
        assert_eq!(
            resolve_selection_strategy(&scene, RuntimeNodeId::new(2)),
            SelectionStrategy::ParentSelection {
                collection_locator: BackendLocator::new(":1.2", "/node/1"),
                target_locator: BackendLocator::new(":1.2", "/node/2"),
            }
        );
    }

    #[test]
    fn qt_style_toggle_action_resolves_to_select_not_toggle_semantics() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut list = node(2, SemanticRole::List, "Items");
        let mut item = node(1, SemanticRole::ListItem, "Beta");
        item.index_in_parent = Some(1);
        item.states = vec![
            SemanticState::Enabled,
            SemanticState::Other("selectable".to_owned()),
            SemanticState::Other("showing".to_owned()),
        ];
        item.actions.push(action("Toggle"));
        list.children.push(item);
        root.children.push(list);
        let scene = compile_legacy_scene(&root);

        assert_eq!(
            SemanticOperation::from_intent(RuntimeNodeId::new(1), UiIntent::Select),
            Some(SemanticOperation::SelectNode(RuntimeNodeId::new(1)))
        );
        assert!(matches!(
            resolve_selection_strategy(&scene, RuntimeNodeId::new(1)),
            SelectionStrategy::NodeAction { action, .. } if action.name == "Toggle"
        ));
    }

    #[test]
    fn multiselect_toggle_never_becomes_single_select() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut list = node(2, SemanticRole::List, "Items");
        list.states = vec![SemanticState::Other("multiselectable".to_owned())];
        let mut item = node(1, SemanticRole::ListItem, "Beta");
        item.states = vec![
            SemanticState::Enabled,
            SemanticState::Other("selectable".to_owned()),
            SemanticState::Other("showing".to_owned()),
        ];
        item.actions.push(action("Toggle"));
        list.children.push(item);
        root.children.push(list);
        let scene = compile_legacy_scene(&root);

        assert_eq!(
            resolve_selection_strategy(&scene, RuntimeNodeId::new(1)),
            SelectionStrategy::Unsupported
        );
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

    #[test]
    fn qualified_value_maps_to_increment_operation_without_an_action() {
        let mut root = node(0, SemanticRole::Window, "Demo");
        let mut slider = node(1, SemanticRole::Slider, "Probe value");
        slider.value = Some("4".to_owned());
        slider.capabilities.push(SemanticCapability::Value);
        root.children.push(slider);
        let scene = compile_legacy_scene(&root);

        assert_eq!(
            resolve_backend_operation(
                &scene,
                SemanticOperation::AdjustValue {
                    target: RuntimeNodeId::new(1),
                    increase: true,
                },
            ),
            Ok(BackendOperation::AdjustValue {
                locator: BackendLocator::new(":1.2", "/node/1"),
                increase: true,
            })
        );
    }
}
