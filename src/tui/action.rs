use thiserror::Error;

use crate::semantic::{SemanticAction, SemanticCapability, SemanticRole};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiIntent {
    RegionNext,
    RegionPrevious,
    SubregionNext,
    SubregionPrevious,
    FocusNext,
    FocusPrevious,
    Activate,
    Toggle,
    Select,
    BeginChoice,
    OpenMenu,
    ClosePopup,
    OpenCommandPalette,
    BeginRead,
    OpenOutline,
    OpenContentSearch,
    BeginEdit,
    CommitEdit,
    CancelEdit,
    IncreaseValue,
    DecreaseValue,
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
    Select,
    Choose,
    OpenMenu,
    EditText,
    AdjustValue,
    BrowseContent,
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
    capabilities: &[SemanticCapability],
    parent_capabilities: &[SemanticCapability],
) -> InteractionCapability {
    if *role == SemanticRole::TextInput && capabilities.contains(&SemanticCapability::EditText) {
        return InteractionCapability::EditText;
    }
    if *role == SemanticRole::Slider && capabilities.contains(&SemanticCapability::Value) {
        return InteractionCapability::AdjustValue;
    }
    let intent = match role {
        SemanticRole::ToggleButton | SemanticRole::CheckBox | SemanticRole::RadioButton => {
            UiIntent::Toggle
        }
        SemanticRole::Button => UiIntent::Activate,
        SemanticRole::ListItem => {
            if resolve_action(role, actions, UiIntent::Select).is_ok()
                || parent_capabilities.contains(&SemanticCapability::SelectChildren)
            {
                return InteractionCapability::Select;
            }
            return InteractionCapability::None;
        }
        SemanticRole::MenuItem => {
            if resolve_action(role, actions, UiIntent::OpenMenu).is_ok() {
                return InteractionCapability::OpenMenu;
            }
            UiIntent::Activate
        }
        // Choice interactivity depends on exposed named options and their safe
        // selection strategies. It is assigned by the ChoiceCatalog, not by role.
        SemanticRole::ComboBox => return InteractionCapability::None,
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
        (SemanticRole::RadioButton, UiIntent::Activate | UiIntent::Toggle) => {
            &["toggle", "click", "press"]
        }
        // Qt 6 QListWidgetItem exposes Toggle, but the user operation is Select.
        (SemanticRole::ListItem, UiIntent::Select) => &["select", "toggle", "activate", "click"],
        (SemanticRole::MenuItem, UiIntent::OpenMenu) => &["showmenu", "show-menu"],
        (SemanticRole::MenuItem, UiIntent::Activate) => &["activate", "click", "press"],
        // `click`/`press` are accepted only after the relational analyzer has
        // identified the unique action-bearing disclosure child of a ComboBox.
        (SemanticRole::ComboBox, UiIntent::OpenMenu) => {
            &["showmenu", "show-menu", "click", "press"]
        }
        // Opening actions are not assumed to be reversible. Qt advertises
        // both `ShowMenu` and `Press` as "Open the combo box selection popup";
        // invoking either after a selection can reselect an item or leave the
        // popup open. Until a backend advertises an explicit, verified close
        // semantic, closing remains unavailable rather than guessed.
        (SemanticRole::ComboBox, UiIntent::ClosePopup) => &[],
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
            interaction_capability(&SemanticRole::Button, &actions(&["Click"]), &[], &[]),
            InteractionCapability::Activate
        );
        assert_eq!(
            interaction_capability(&SemanticRole::CheckBox, &[], &[], &[]),
            InteractionCapability::None
        );
        assert_eq!(
            interaction_capability(&SemanticRole::TextInput, &actions(&["Activate"]), &[], &[]),
            InteractionCapability::None
        );
        assert_eq!(
            interaction_capability(&SemanticRole::ListItem, &actions(&["Toggle"]), &[], &[]),
            InteractionCapability::Select
        );
        assert_eq!(
            interaction_capability(&SemanticRole::RadioButton, &actions(&["Toggle"]), &[], &[]),
            InteractionCapability::Toggle
        );
    }

    #[test]
    fn menu_open_and_leaf_activation_are_distinct() {
        let submenu = actions(&["ShowMenu"]);
        assert!(resolve_action(&SemanticRole::MenuItem, &submenu, UiIntent::Activate).is_err());
        assert_eq!(
            resolve_action(&SemanticRole::MenuItem, &submenu, UiIntent::OpenMenu)
                .unwrap()
                .name,
            "ShowMenu"
        );

        let leaf = actions(&["Press"]);
        assert!(resolve_action(&SemanticRole::MenuItem, &leaf, UiIntent::OpenMenu).is_err());
        assert_eq!(
            resolve_action(&SemanticRole::MenuItem, &leaf, UiIntent::Activate)
                .unwrap()
                .name,
            "Press"
        );
    }

    #[test]
    fn combo_role_alone_never_claims_choice_or_disclosure_capability() {
        assert_eq!(
            interaction_capability(
                &SemanticRole::ComboBox,
                &actions(&["ShowMenu", "Press"]),
                &[],
                &[]
            ),
            InteractionCapability::None
        );
        assert!(
            resolve_action(&SemanticRole::ComboBox, &actions(&[""]), UiIntent::OpenMenu).is_err()
        );
        assert!(
            resolve_action(
                &SemanticRole::ComboBox,
                &actions(&["ShowMenu", "Press"]),
                UiIntent::ClosePopup
            )
            .is_err()
        );
    }

    #[test]
    fn parent_selection_makes_a_gtk_style_list_item_selectable() {
        assert_eq!(
            interaction_capability(
                &SemanticRole::ListItem,
                &actions(&["listitem.scroll-to"]),
                &[],
                &[SemanticCapability::SelectChildren]
            ),
            InteractionCapability::Select
        );
    }

    #[test]
    fn text_edit_capability_is_explicit_and_independent_of_actions() {
        assert_eq!(
            interaction_capability(
                &SemanticRole::TextInput,
                &[],
                &[SemanticCapability::EditText],
                &[]
            ),
            InteractionCapability::EditText
        );
        assert_eq!(
            interaction_capability(&SemanticRole::TextInput, &actions(&["Activate"]), &[], &[]),
            InteractionCapability::None
        );
    }
}
