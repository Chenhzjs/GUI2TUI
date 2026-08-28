use thiserror::Error;

use crate::semantic::SemanticAction;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionResolutionError {
    #[error("the focused control exposes no AT-SPI actions")]
    NoActions,
    #[error("no compatible action was found; available actions: {available}")]
    NoCompatibleAction { available: String },
}

pub fn resolve_action(
    actions: &[SemanticAction],
    intent: UiIntent,
) -> Result<&SemanticAction, ActionResolutionError> {
    if actions.is_empty() {
        return Err(ActionResolutionError::NoActions);
    }

    let preferred = match intent {
        UiIntent::Activate => &["click", "press", "activate", "open"][..],
        UiIntent::Toggle => &["click", "toggle", "press", "activate"][..],
        _ => &[][..],
    };

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
        let resolved = resolve_action(&available, UiIntent::Activate).unwrap();
        assert_eq!(resolved.index, 1);
        assert_eq!(resolved.name, "Click");
    }

    #[test]
    fn reports_missing_compatible_action() {
        let available = actions(&["show-menu"]);
        assert!(matches!(
            resolve_action(&available, UiIntent::Activate),
            Err(ActionResolutionError::NoCompatibleAction { .. })
        ));
        assert_eq!(
            resolve_action(&[], UiIntent::Activate),
            Err(ActionResolutionError::NoActions)
        );
    }
}
