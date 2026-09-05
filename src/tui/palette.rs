use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::{
    semantic::{BackendLocator, RuntimeNodeId},
    transcompile::{
        CommandEntry, CommandGroup, CommandHierarchy, InteractionScopeId, SemanticCommand,
    },
    tui::action::UiIntent,
};

const DEFAULT_CONTEXT_LIMIT: usize = 15;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    pub label: String,
    pub group: bool,
    target: Option<(RuntimeNodeId, BackendLocator, UiIntent)>,
    group_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPalette {
    hierarchy: CommandHierarchy,
    active_scope: InteractionScopeId,
    query: String,
    selected: usize,
    group_path: Vec<usize>,
    entries: Vec<PaletteEntry>,
    recent: HashMap<RuntimeNodeId, u32>,
    search_all_scopes: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteOutcome {
    Continue,
    Close,
    Execute(RuntimeNodeId, BackendLocator, UiIntent),
}

impl CommandPalette {
    pub fn new(
        hierarchy: CommandHierarchy,
        active_scope: InteractionScopeId,
        recent: HashMap<RuntimeNodeId, u32>,
    ) -> Self {
        let mut palette = Self {
            hierarchy,
            active_scope,
            query: String::new(),
            selected: 0,
            group_path: Vec::new(),
            entries: Vec::new(),
            recent,
            search_all_scopes: false,
        };
        palette.rebuild();
        palette
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }

    pub fn searches_all_scopes(&self) -> bool {
        self.search_all_scopes
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> PaletteOutcome {
        if event.kind == KeyEventKind::Release {
            return PaletteOutcome::Continue;
        }
        match event.code {
            KeyCode::F(2) => {
                self.search_all_scopes = !self.search_all_scopes;
                self.rebuild();
                PaletteOutcome::Continue
            }
            KeyCode::Esc => {
                if self.query.is_empty() && !self.group_path.is_empty() {
                    self.group_path.pop();
                    self.rebuild();
                    PaletteOutcome::Continue
                } else {
                    PaletteOutcome::Close
                }
            }
            KeyCode::Enter | KeyCode::Right => {
                let Some(entry) = self.entries.get(self.selected).cloned() else {
                    return PaletteOutcome::Continue;
                };
                if let Some(index) = entry.group_index {
                    self.group_path.push(index);
                    self.query.clear();
                    self.rebuild();
                    PaletteOutcome::Continue
                } else {
                    entry
                        .target
                        .map_or(PaletteOutcome::Continue, |(id, locator, intent)| {
                            PaletteOutcome::Execute(id, locator, intent)
                        })
                }
            }
            KeyCode::Left => {
                if self.query.is_empty() && self.group_path.pop().is_some() {
                    self.rebuild();
                }
                PaletteOutcome::Continue
            }
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
                if self.query.pop().is_none() {
                    self.group_path.pop();
                }
                self.rebuild();
                PaletteOutcome::Continue
            }
            KeyCode::Char(character) => {
                self.query.push(character);
                self.rebuild();
                PaletteOutcome::Continue
            }
            _ => PaletteOutcome::Continue,
        }
    }

    fn rebuild(&mut self) {
        self.entries = if self.query.is_empty() {
            current_group(&self.hierarchy.root, &self.group_path)
                .map(|group| browse_entries(group, self.active_scope))
                .unwrap_or_default()
        } else {
            self.hierarchy
                .search(
                    &self.query,
                    self.active_scope,
                    self.search_all_scopes,
                    &self.recent,
                )
                .into_iter()
                .take(DEFAULT_CONTEXT_LIMIT)
                .map(|ranked| PaletteEntry {
                    label: format!("{} › {}", ranked.path.join(" › "), ranked.command.label),
                    group: false,
                    target: Some((
                        ranked.command.source,
                        ranked.command.backend_locator.clone(),
                        ranked.command.intent,
                    )),
                    group_index: None,
                })
                .collect()
        };
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }
}

fn current_group<'a>(root: &'a CommandGroup, path: &[usize]) -> Option<&'a CommandGroup> {
    let mut group = root;
    for index in path {
        let CommandEntry::Group(next) = group.children.get(*index)? else {
            return None;
        };
        group = next;
    }
    Some(group)
}

fn browse_entries(group: &CommandGroup, scope: InteractionScopeId) -> Vec<PaletteEntry> {
    group
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match entry {
            CommandEntry::Group(group) if group_contains_scope(group, scope) => {
                Some(PaletteEntry {
                    label: format!("{} ›", group.label),
                    group: true,
                    target: None,
                    group_index: Some(index),
                })
            }
            CommandEntry::Command(command) if command.scope == scope && command.visible => {
                Some(command_entry(command))
            }
            _ => None,
        })
        .take(DEFAULT_CONTEXT_LIMIT)
        .collect()
}

fn group_contains_scope(group: &CommandGroup, scope: InteractionScopeId) -> bool {
    group.children.iter().any(|entry| match entry {
        CommandEntry::Group(group) => group_contains_scope(group, scope),
        CommandEntry::Command(command) => command.scope == scope && command.visible,
    })
}

fn command_entry(command: &SemanticCommand) -> PaletteEntry {
    PaletteEntry {
        label: command.label.clone(),
        group: false,
        target: Some((
            command.source,
            command.backend_locator.clone(),
            command.intent,
        )),
        group_index: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcompile::{CommandEntry, CommandGroup, SemanticCommand};

    fn command(id: u64, scope: InteractionScopeId, label: &str) -> CommandEntry {
        CommandEntry::Command(SemanticCommand {
            source: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/{id}")),
            label: label.to_owned(),
            scope,
            intent: UiIntent::Activate,
            enabled: true,
            visible: true,
            shortcut: None,
        })
    }

    #[test]
    fn browses_true_groups_then_searches_flattened_projection() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let hierarchy = CommandHierarchy {
            root: CommandGroup {
                source: RuntimeNodeId::new(1),
                label: "App".to_owned(),
                scope,
                children: vec![CommandEntry::Group(CommandGroup {
                    source: RuntimeNodeId::new(2),
                    label: "File".to_owned(),
                    scope,
                    children: vec![command(3, scope, "Open")],
                })],
            },
        };
        let mut palette = CommandPalette::new(hierarchy, scope, HashMap::new());
        assert!(palette.entries()[0].group);
        palette.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(palette.entries()[0].label, "Open");
        palette.handle_key(KeyEvent::from(KeyCode::Char('o')));
        assert!(palette.entries()[0].label.contains("Open"));
    }

    #[test]
    fn search_stays_in_current_scope_until_explicitly_expanded() {
        let current = InteractionScopeId(RuntimeNodeId::new(1));
        let background = InteractionScopeId(RuntimeNodeId::new(2));
        let hierarchy = CommandHierarchy {
            root: CommandGroup {
                source: RuntimeNodeId::new(1),
                label: "App".to_owned(),
                scope: current,
                children: vec![
                    command(3, current, "Close dialog"),
                    command(4, background, "Close document"),
                ],
            },
        };
        let mut palette = CommandPalette::new(hierarchy, current, HashMap::new());
        palette.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(palette.entries().len(), 1);
        assert!(palette.entries()[0].label.contains("Close dialog"));

        palette.handle_key(KeyEvent::from(KeyCode::F(2)));
        assert!(palette.searches_all_scopes());
        assert_eq!(palette.entries().len(), 2);
    }
}
