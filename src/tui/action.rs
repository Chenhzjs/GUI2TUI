use thiserror::Error;

use crate::semantic::{SemanticAction, SemanticRole};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiIntent {
    FocusNext,
    FocusPrevious,
    Activate,
    Toggle,
    Refresh,
    ScrollLines(i16),
    ScrollPages(i16),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCapability {
    None,
    Activate,
    Toggle,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionResolutionError {
    #[error("the focused control exposes no AT-SPI actions")]
    NoActions,
    #[error("no compatible action was found; available actions: {available}")]
    NoCompatibleAction { available: String },
}

pub fn resolve_action<'a>(
    role: &SemanticRole,
    actions: &'a [SemanticAction],
    intent: UiIntent,
) -> Result<&'a SemanticAction, ActionResolutionError> {
    if actions.is_empty() {
        return Err(ActionResolutionError::NoActions);
    }

    let preferred = compatible_action_names(role, intent);

    preferred
        .iter()
        .find_map(|name| {
            actions
                .iter()
                .find(|action| action.name.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| ActionResolutionError::NoCompatibleAction {
            available: actions
                .iter()
                .map(|action| action.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })
}

pub fn interaction_capability(
    role: &SemanticRole,
    actions: &[SemanticAction],
) -> InteractionCapability {
    let intent = match role {
        SemanticRole::ToggleButton | SemanticRole::CheckBox => UiIntent::Toggle,
        SemanticRole::Button | SemanticRole::ListItem | SemanticRole::MenuItem => {
            UiIntent::Activate
        }
        _ => return InteractionCapability::None,
    };
    if resolve_action(role, actions, intent).is_err() {
        InteractionCapability::None
    } else if intent == UiIntent::Toggle {
        InteractionCapability::Toggle
    } else {
        InteractionCapability::Activate
    }
}

fn compatible_action_names(role: &SemanticRole, intent: UiIntent) -> &'static [&'static str] {
    match (role, intent) {
        (SemanticRole::Button, UiIntent::Activate | UiIntent::Toggle) => {
            &["click", "press", "activate"]
        }
        (SemanticRole::ToggleButton, UiIntent::Activate | UiIntent::Toggle) => {
            &["toggle", "click", "press", "activate"]
        }
        (SemanticRole::CheckBox, UiIntent::Activate | UiIntent::Toggle) => {
            &["toggle", "click", "press"]
        }
        // Qt 6 QListWidgetItem exposes Toggle, which selects the item.
        (SemanticRole::ListItem, UiIntent::Activate) => &["select", "toggle", "activate", "click"],
        (SemanticRole::MenuItem, UiIntent::Activate) => &["activate", "click", "press"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(names: &[&str]) -> Vec<SemanticAction> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| SemanticAction {
                index: index as i32,
                name: (*name).to_owned(),
                description: None,
                keybinding: None,
            })
            .collect()
    }

    #[test]
    fn resolves_button_action_by_semantic_preference_case_insensitively() {
        let available = actions(&["show-menu", "Click", "press"]);
        let resolved =
            resolve_action(&SemanticRole::Button, &available, UiIntent::Activate).unwrap();
        assert_eq!(resolved.index, 1);
        assert_eq!(resolved.name, "Click");
    }

    #[test]
    fn reports_missing_compatible_action() {
        let available = actions(&["show-menu"]);
        assert!(matches!(
            resolve_action(&SemanticRole::Button, &available, UiIntent::Activate),
            Err(ActionResolutionError::NoCompatibleAction { .. })
        ));
        assert_eq!(
            resolve_action(&SemanticRole::Button, &[], UiIntent::Activate),
            Err(ActionResolutionError::NoActions)
        );
    }

    #[test]
    fn resolver_is_role_aware_and_never_falls_back_to_action_zero() {
        let dangerous = actions(&["delete", "open", "properties"]);
        assert!(matches!(
            resolve_action(&SemanticRole::Button, &dangerous, UiIntent::Activate),
            Err(ActionResolutionError::NoCompatibleAction { .. })
        ));
        assert!(matches!(
            resolve_action(
                &SemanticRole::Unknown("custom".to_owned()),
                &dangerous,
                UiIntent::Activate
            ),
            Err(ActionResolutionError::NoCompatibleAction { .. })
        ));
    }

    #[test]
    fn capability_requires_a_role_compatible_advertised_action() {
        assert_eq!(
            interaction_capability(&SemanticRole::Button, &actions(&["Click"])),
            InteractionCapability::Activate
        );
        assert_eq!(
            interaction_capability(&SemanticRole::CheckBox, &[]),
            InteractionCapability::None
        );
        assert_eq!(
            interaction_capability(&SemanticRole::TextInput, &actions(&["Activate"])),
            InteractionCapability::None
        );
        assert_eq!(
            interaction_capability(&SemanticRole::ListItem, &actions(&["Toggle"])),
            InteractionCapability::Activate
        );
    }
}
