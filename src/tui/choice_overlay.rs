use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::transcompile::{ChoiceOption, SemanticChoice};
use crate::{semantic::RuntimeNodeId, transcompile::SceneElementId};

#[derive(Clone, Debug)]
pub struct ChoiceOverlay {
    choice: SemanticChoice,
    selected: usize,
    restore_scene: SceneElementId,
    restore_runtime: RuntimeNodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChoiceOverlayOutcome {
    Continue,
    Cancel,
    Select(ChoiceOption),
}

impl ChoiceOverlay {
    pub fn new(
        choice: SemanticChoice,
        restore_scene: SceneElementId,
        restore_runtime: RuntimeNodeId,
    ) -> Self {
        let selected = choice
            .options
            .options()
            .iter()
            .position(|option| option.selected)
            .unwrap_or(0);
        Self {
            choice,
            selected,
            restore_scene,
            restore_runtime,
        }
    }

    pub fn choice(&self) -> &SemanticChoice {
        &self.choice
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn restore_scene(&self) -> SceneElementId {
        self.restore_scene
    }

    pub fn restore_runtime(&self) -> RuntimeNodeId {
        self.restore_runtime
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ChoiceOverlayOutcome {
        if key.kind == KeyEventKind::Release {
            return ChoiceOverlayOutcome::Continue;
        }
        let options = self.choice.options.options();
        match key.code {
            KeyCode::Esc => ChoiceOverlayOutcome::Cancel,
            KeyCode::Up | KeyCode::Char('k') => {
                if !options.is_empty() {
                    self.selected = self.selected.checked_sub(1).unwrap_or(options.len() - 1);
                }
                ChoiceOverlayOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !options.is_empty() {
                    self.selected = (self.selected + 1) % options.len();
                }
                ChoiceOverlayOutcome::Continue
            }
            KeyCode::Home => {
                self.selected = 0;
                ChoiceOverlayOutcome::Continue
            }
            KeyCode::End => {
                self.selected = options.len().saturating_sub(1);
                ChoiceOverlayOutcome::Continue
            }
            KeyCode::Enter => options
                .get(self.selected)
                .filter(|option| option.enabled && option.selection.is_some())
                .cloned()
                .map_or(ChoiceOverlayOutcome::Continue, ChoiceOverlayOutcome::Select),
            _ => ChoiceOverlayOutcome::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyModifiers};

    use crate::{
        semantic::{CollectionCompleteness, SemanticAction},
        transcompile::{
            ChoiceOptions, ChoiceSelectionStrategy, DisclosureRequirement, DismissBehavior,
        },
    };

    use super::*;

    fn choice() -> SemanticChoice {
        let option = |id: u64, label: &str, selected: bool| ChoiceOption {
            runtime_id: RuntimeNodeId::new(id),
            label: label.to_owned(),
            selected,
            enabled: true,
            selection: Some(ChoiceSelectionStrategy::ChildSemanticAction {
                child: RuntimeNodeId::new(id),
                action: SemanticAction {
                    index: 0,
                    name: "Toggle".to_owned(),
                    description: None,
                    keybinding: None,
                },
            }),
        };
        SemanticChoice {
            owner: RuntimeNodeId::new(1),
            current: Some(RuntimeNodeId::new(2)),
            options: ChoiceOptions::Available(vec![
                option(2, "Alpha", true),
                option(3, "Beta", false),
            ]),
            disclosure: DisclosureRequirement::NotRequired,
            dismiss: DismissBehavior::NotApplicable,
            completeness: CollectionCompleteness::Complete,
        }
    }

    #[test]
    fn enter_selects_terminal_option_and_escape_only_closes_overlay() {
        let mut overlay =
            ChoiceOverlay::new(choice(), SceneElementId::new(41), RuntimeNodeId::new(1));
        assert_eq!(overlay.selected(), 0);
        assert_eq!(
            overlay.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ChoiceOverlayOutcome::Continue
        );
        assert!(matches!(
            overlay.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ChoiceOverlayOutcome::Select(ChoiceOption { label, .. }) if label == "Beta"
        ));

        let mut overlay =
            ChoiceOverlay::new(choice(), SceneElementId::new(41), RuntimeNodeId::new(1));
        assert_eq!(
            overlay.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ChoiceOverlayOutcome::Cancel
        );
        assert_eq!(overlay.restore_scene(), SceneElementId::new(41));
        assert_eq!(overlay.restore_runtime(), RuntimeNodeId::new(1));
    }
}
