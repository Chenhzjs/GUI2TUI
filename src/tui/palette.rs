use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::transcompile::{SceneElementId, TuiScene};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandPalette {
    query: String,
    selected: usize,
    entries: Vec<(SceneElementId, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteOutcome {
    Continue,
    Close,
    Execute(SceneElementId),
}

impl CommandPalette {
    pub fn from_scene(scene: &TuiScene) -> Self {
        let mut palette = Self {
            query: String::new(),
            selected: 0,
            entries: scene
                .commands()
                .map(|element| (element.id, element.label().to_owned()))
                .collect(),
        };
        palette.sort();
        palette
    }

    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn entries(&self) -> &[(SceneElementId, String)] {
        &self.entries
    }

    pub fn handle_key(&mut self, event: KeyEvent, scene: &TuiScene) -> PaletteOutcome {
        if event.kind == KeyEventKind::Release {
            return PaletteOutcome::Continue;
        }
        match event.code {
            KeyCode::Esc => PaletteOutcome::Close,
            KeyCode::Enter => self
                .entries
                .get(self.selected)
                .map_or(PaletteOutcome::Continue, |entry| {
                    PaletteOutcome::Execute(entry.0)
                }),
            KeyCode::Up => {
                if !self.entries.is_empty() {
                    self.selected = self
                        .selected
                        .checked_sub(1)
                        .unwrap_or(self.entries.len() - 1);
                }
                PaletteOutcome::Continue
            }
            KeyCode::Down => {
                if !self.entries.is_empty() {
                    self.selected = (self.selected + 1) % self.entries.len();
                }
                PaletteOutcome::Continue
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.rebuild(scene);
                PaletteOutcome::Continue
            }
            KeyCode::Char(character) => {
                self.query.push(character);
                self.rebuild(scene);
                PaletteOutcome::Continue
            }
            _ => PaletteOutcome::Continue,
        }
    }

    fn rebuild(&mut self, scene: &TuiScene) {
        let query = self.query.to_lowercase();
        self.entries = scene
            .commands()
            .filter_map(|element| {
                let label = element.label();
                label
                    .to_lowercase()
                    .contains(&query)
                    .then(|| (element.id, label.to_owned()))
            })
            .collect();
        self.sort();
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    fn sort(&mut self) {
        self.entries.sort_by(|left, right| left.1.cmp(&right.1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        semantic::{
            BackendLocator, DebugInfo, RuntimeNodeId, SemanticAction, SemanticNode, SemanticRole,
        },
        transcompile::{analyze_regions, compile_scene},
    };

    fn node(id: u64, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/n/{id}")),
            index_in_parent: None,
            role: SemanticRole::MenuItem,
            name: Some(name.into()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![],
            actions: vec![SemanticAction {
                index: 0,
                name: "Press".into(),
                description: None,
                keybinding: None,
            }],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn filters_and_wraps_selection() {
        let mut root = node(0, "Menu");
        root.role = SemanticRole::MenuBar;
        root.children = vec![node(1, "Open"), node(2, "Save")];
        let scene = compile_scene(&root, &analyze_regions(&root));
        let mut palette = CommandPalette::from_scene(&scene);
        assert_eq!(palette.entries.len(), 2);
        palette.handle_key(KeyEvent::from(KeyCode::Char('s')), &scene);
        assert_eq!(palette.entries.len(), 1);
        assert!(palette.entries[0].1.contains("Save"));
    }
}
