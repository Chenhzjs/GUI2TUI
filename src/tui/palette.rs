use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::{
    semantic::{BackendLocator, RuntimeNodeId},
    transcompile::{
        CommandEntry, CommandGroup, CommandHierarchy, InteractionScopeId, SemanticCommand,
    },
    tui::action::UiIntent,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicOperation {
    SetValue,
    EditRange,
    ReadSelection,
    Action(String),
    EditText {
        multiline: bool,
    },
    Selection {
        parent: BackendLocator,
        selected: bool,
    },
    Value {
        increase: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    pub label: String,
    /// Presentation only; `label` remains the full searchable public label.
    pub title: String,
    pub group: bool,
    target: Option<(RuntimeNodeId, BackendLocator, UiIntent)>,
    group_index: Option<usize>,
    action: Option<PublicOperation>,
    action_presentation: Option<crate::transcompile::document::ActionPresentation>,
    section: Option<ActionSection>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ActionSection {
    Current,
    Controls,
    Interfaces,
}

impl PaletteEntry {
    pub(crate) fn section_label(&self) -> Option<&'static str> {
        self.section.map(|section| match section {
            ActionSection::Current => "Current object",
            ActionSection::Controls => "Controls",
            ActionSection::Interfaces => "Interface operations",
        })
    }
    pub(crate) fn runtime_target(&self) -> Option<(RuntimeNodeId, &BackendLocator)> {
        self.target.as_ref().map(|(id, locator, _)| (*id, locator))
    }

    pub fn short_label(&self) -> String {
        let Some(action) = &self.action else {
            return self.title.clone();
        };
        let operation = self
            .action_presentation
            .as_ref()
            .map(|p| p.display_label.clone())
            .unwrap_or_else(|| match action {
                PublicOperation::SetValue => "Set value".into(),
                PublicOperation::EditRange => "Edit range".into(),
                PublicOperation::ReadSelection => "Read text selection".into(),
                PublicOperation::Action(name) => format!("Action: {name}"),
                PublicOperation::EditText { multiline: true } => "Edit whole text".into(),
                PublicOperation::EditText { multiline: false } => "Edit text".into(),
                PublicOperation::Selection { selected: true, .. } => "Add to selection".into(),
                PublicOperation::Selection {
                    selected: false, ..
                } => "Remove from selection".into(),
                PublicOperation::Value { increase: true } => "Increase value".into(),
                PublicOperation::Value { increase: false } => "Decrease value".into(),
            });
        format!("{operation} · {}", self.title)
    }

    pub fn detail_text(&self) -> String {
        let mut text = self.short_label();
        if let Some(presentation) = &self.action_presentation {
            text.push_str("\n\n");
            text.push_str(&presentation.detail_text());
        } else if let Some(operation) = &self.action {
            let explanation = match operation {
                PublicOperation::SetValue => "Parameter: requested numeric value",
                PublicOperation::EditRange => {
                    "Parameters: start:end:text (Unicode characters, end exclusive)"
                }
                PublicOperation::ReadSelection => {
                    "Read the current public text selection; no text write"
                }
                PublicOperation::EditText { multiline: true } => {
                    "Parameter: replacement text for the whole accessible text"
                }
                PublicOperation::EditText { multiline: false } => {
                    "Parameter: replacement text for this field"
                }
                PublicOperation::Selection { .. } => {
                    "Change selection for this exact public object"
                }
                PublicOperation::Value { .. } => "Adjust this object's public value",
                PublicOperation::Action(_) => "Invoke the exact advertised public action",
            };
            text.push_str(&format!("\n{explanation}"));
        }
        if let Some((id, locator, _)) = &self.target {
            text.push_str(&format!(
                "\n\nSource details\nSnapshot target: #{id}\n{}\n\nLocator: {}",
                self.label,
                locator.encode()
            ));
        }
        text
    }
}

/// Wrap exact detail text at grapheme boundaries using terminal cell widths.
/// Unlike result labels this is not an ellipsis projection.
pub(crate) fn detail_lines(text: &str, width: u16) -> Vec<String> {
    crate::transcompile::SceneElement::wrap_display_lines(text, width)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPalette {
    hierarchy: CommandHierarchy,
    active_scope: InteractionScopeId,
    query: String,
    navigation_query: Option<(Option<String>, bool)>,
    selected: usize,
    group_path: Vec<usize>,
    entries: Vec<PaletteEntry>,
    recent: HashMap<RuntimeNodeId, u32>,
    search_all_scopes: bool,
    public_actions: Option<Vec<PaletteEntry>>,
    detail_scroll: usize,
    detail_limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteOutcome {
    Continue,
    Close,
    Execute(RuntimeNodeId, BackendLocator, UiIntent),
    InvokeAction(RuntimeNodeId, BackendLocator, PublicOperation),
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
            navigation_query: None,
            selected: 0,
            group_path: Vec::new(),
            entries: Vec::new(),
            recent,
            search_all_scopes: false,
            public_actions: None,
            detail_scroll: 0,
            detail_limit: 0,
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

    pub(crate) fn is_public_actions(&self) -> bool {
        self.public_actions.is_some()
    }

    /// Contextual ordering only: retain every operation and its exact binding.
    pub(crate) fn prioritize_target(&mut self, target: RuntimeNodeId, locator: &BackendLocator) {
        if let Some(actions) = self.public_actions.as_mut() {
            for entry in actions.iter_mut() {
                if entry
                    .runtime_target()
                    .is_some_and(|(id, source)| id == target && source == locator)
                {
                    entry.section = Some(ActionSection::Current);
                }
            }
            actions.sort_by_key(|entry| {
                !entry
                    .runtime_target()
                    .is_some_and(|(id, source)| id == target && source == locator)
            });
            self.rebuild();
        }
    }

    /// Flat presentation sections: no entry, parameter, or binding is removed.
    pub(crate) fn group_public_actions(&mut self, auxiliary: impl Fn(RuntimeNodeId) -> bool) {
        if let Some(actions) = self.public_actions.as_mut() {
            for entry in actions.iter_mut() {
                let interface = entry.runtime_target().is_some_and(|(id, _)| auxiliary(id))
                    || !matches!(
                        entry.action,
                        Some(PublicOperation::Action(_) | PublicOperation::EditText { .. })
                    );
                entry.section = Some(if interface {
                    ActionSection::Interfaces
                } else {
                    ActionSection::Controls
                });
            }
            actions.sort_by_key(|entry| entry.section);
            self.rebuild();
        }
    }

    pub(crate) fn restore_query_and_exact_selection(&mut self, previous: &Self) {
        self.set_query(previous.query.clone());
        self.navigation_query = previous.navigation_query.clone();
        if let Some(old) = previous.entries.get(previous.selected) {
            if let Some(index) = self
                .entries
                .iter()
                .position(|entry| entry.target == old.target && entry.action == old.action)
            {
                self.selected = index;
                self.detail_scroll = previous.detail_scroll;
            }
        }
    }

    /// The same local query used by Human typing, without GUI key delivery.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.navigation_query = None;
        self.detail_scroll = 0;
        self.rebuild();
    }

    pub(crate) fn set_navigation_query(
        &mut self,
        query: String,
        context: Option<String>,
        exact: bool,
    ) {
        self.set_query(query);
        self.navigation_query = Some((context, exact));
    }

    pub(crate) fn navigation_query(&self) -> Option<(String, Option<String>, bool)> {
        self.navigation_query
            .as_ref()
            .map(|(context, exact)| (self.query.clone(), context.clone(), *exact))
    }

    pub(crate) fn retain_entries(&mut self, mut matches: impl FnMut(&PaletteEntry) -> bool) {
        let selected = self.entries.get(self.selected).cloned();
        self.entries.retain(|entry| matches(entry));
        self.selected = selected
            .and_then(|old| {
                self.entries
                    .iter()
                    .position(|entry| entry.target == old.target && entry.action == old.action)
            })
            .unwrap_or(0);
    }

    pub(crate) fn refresh_commands(
        &mut self,
        hierarchy: CommandHierarchy,
        scope: InteractionScopeId,
    ) {
        if self.public_actions.is_some() {
            return;
        }
        let previous = self
            .entries
            .get(self.selected)
            .and_then(|entry| entry.runtime_target())
            .map(|(id, locator)| (id, locator.clone()));
        if self.active_scope != scope {
            self.group_path.clear();
        }
        self.hierarchy = hierarchy;
        self.active_scope = scope;
        self.rebuild();
        if let Some((id, locator)) = previous {
            self.selected = self
                .entries
                .iter()
                .position(|entry| {
                    entry
                        .runtime_target()
                        .is_some_and(|target| target.0 == id && *target.1 == locator)
                })
                .unwrap_or(0);
        }
    }

    pub fn step_selection(&mut self, forward: bool) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = if forward {
            (self.selected + 1) % self.entries.len()
        } else {
            self.selected
                .checked_sub(1)
                .unwrap_or(self.entries.len() - 1)
        };
        self.detail_scroll = 0;
    }

    pub fn scroll_details(&mut self, delta: i16) {
        self.detail_scroll = if delta >= 0 {
            self.detail_scroll
                .saturating_add(delta as usize)
                .min(self.detail_limit)
        } else {
            self.detail_scroll
                .saturating_sub(delta.unsigned_abs() as usize)
        };
    }

    pub fn searches_all_scopes(&self) -> bool {
        self.search_all_scopes && self.scope_switchable()
    }

    pub fn scope_switchable(&self) -> bool {
        self.public_actions.is_none()
    }

    pub fn detail_scroll(&self) -> usize {
        self.detail_scroll
    }

    pub fn set_detail_viewport(&mut self, width: u16, height: u16) {
        self.detail_limit = self.entries.get(self.selected).map_or(0, |entry| {
            detail_lines(&entry.detail_text(), width)
                .len()
                .saturating_sub(usize::from(height))
        });
        self.detail_scroll = self.detail_scroll.min(self.detail_limit);
    }

    pub fn set_target_titles(&mut self, title: impl Fn(RuntimeNodeId) -> Option<String>) {
        if let Some(actions) = self.public_actions.as_mut() {
            for entry in actions {
                if let Some((id, _, _)) = &entry.target
                    && let Some(title) = title(*id)
                {
                    entry.title = title;
                }
            }
            self.rebuild();
        }
    }

    /// Explanation only: the exact target/action remain unchanged.
    pub fn set_action_presentations(
        &mut self,
        presentation: impl Fn(
            RuntimeNodeId,
            &PublicOperation,
        ) -> Option<crate::transcompile::document::ActionPresentation>,
    ) {
        if let Some(actions) = self.public_actions.as_mut() {
            for entry in actions {
                if let (Some((id, _, _)), Some(action)) = (&entry.target, &entry.action) {
                    entry.action_presentation = presentation(*id, action);
                }
            }
            self.rebuild();
        }
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> PaletteOutcome {
        if event.kind == KeyEventKind::Release {
            return PaletteOutcome::Continue;
        }
        if !matches!(
            event.code,
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
        ) {
            self.navigation_query = None;
        }
        match event.code {
            KeyCode::PageDown => {
                self.scroll_details(4);
                return PaletteOutcome::Continue;
            }
            KeyCode::PageUp => {
                self.scroll_details(-4);
                return PaletteOutcome::Continue;
            }
            _ => self.detail_scroll = 0,
        }
        match event.code {
            KeyCode::F(2) => {
                if self.scope_switchable() {
                    self.search_all_scopes = !self.search_all_scopes;
                }
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
                    if let (Some(action), Some((id, locator, _))) =
                        (entry.action.clone(), entry.target.clone())
                    {
                        return PaletteOutcome::InvokeAction(id, locator, action);
                    }
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
                self.step_selection(false);
                PaletteOutcome::Continue
            }
            KeyCode::Down => {
                self.step_selection(true);
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

    pub fn public_actions(
        hierarchy: CommandHierarchy,
        scope: InteractionScopeId,
        actions: Vec<(RuntimeNodeId, BackendLocator, String, PublicOperation)>,
    ) -> Self {
        let mut palette = Self::new(hierarchy, scope, HashMap::new());
        palette.public_actions = Some(
            actions
                .into_iter()
                .map(|(id, locator, label, action)| PaletteEntry {
                    title: label.clone(),
                    label,
                    group: false,
                    target: Some((id, locator, UiIntent::Activate)),
                    group_index: None,
                    action: Some(action),
                    action_presentation: None,
                    section: None,
                })
                .collect(),
        );
        palette.rebuild();
        palette
    }

    fn rebuild(&mut self) {
        if let Some(actions) = &self.public_actions {
            self.entries = actions
                .iter()
                .filter(|entry| {
                    entry
                        .label
                        .to_lowercase()
                        .contains(&self.query.to_lowercase())
                        || entry
                            .title
                            .to_lowercase()
                            .contains(&self.query.to_lowercase())
                })
                .cloned()
                .collect();
            self.selected = self.selected.min(self.entries.len().saturating_sub(1));
            return;
        }
        self.entries = if self.query.is_empty() {
            current_group(&self.hierarchy.root, &self.group_path)
                .map(|group| browse_entries(group, self.active_scope, self.search_all_scopes))
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
                .map(|ranked| PaletteEntry {
                    title: ranked.command.label.clone(),
                    label: format!("{} › {}", ranked.path.join(" › "), ranked.command.label),
                    group: false,
                    target: Some((
                        ranked.command.source,
                        ranked.command.backend_locator.clone(),
                        ranked.command.intent,
                    )),
                    group_index: None,
                    action: None,
                    action_presentation: None,
                    section: None,
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

fn browse_entries(
    group: &CommandGroup,
    scope: InteractionScopeId,
    all_scopes: bool,
) -> Vec<PaletteEntry> {
    group
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| match entry {
            CommandEntry::Group(group) if all_scopes || group_contains_scope(group, scope) => {
                Some(PaletteEntry {
                    title: group.label.clone(),
                    label: format!("{} ›", group.label),
                    group: true,
                    target: None,
                    group_index: Some(index),
                    action: None,
                    action_presentation: None,
                    section: None,
                })
            }
            CommandEntry::Command(command)
                if (all_scopes || command.scope == scope) && command.visible =>
            {
                Some(command_entry(command))
            }
            _ => None,
        })
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
        title: command.label.clone(),
        label: command.label.clone(),
        group: false,
        target: Some((
            command.source,
            command.backend_locator.clone(),
            command.intent,
        )),
        group_index: None,
        action: None,
        action_presentation: None,
        section: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_target_ordering_preserves_every_exact_operation() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let locator = BackendLocator::new(":1.2", "/current");
        let mut palette = CommandPalette::public_actions(
            CommandHierarchy {
                root: CommandGroup {
                    source: RuntimeNodeId::new(1),
                    label: "Commands".into(),
                    scope,
                    children: vec![],
                },
            },
            scope,
            vec![
                (
                    RuntimeNodeId::new(1),
                    BackendLocator::new(":1.2", "/other"),
                    "Other".into(),
                    PublicOperation::Action("press".into()),
                ),
                (
                    RuntimeNodeId::new(2),
                    locator.clone(),
                    "Current".into(),
                    PublicOperation::EditText { multiline: true },
                ),
                (
                    RuntimeNodeId::new(2),
                    BackendLocator::new(":1.2", "/stale"),
                    "Same numeric ID".into(),
                    PublicOperation::Action("press".into()),
                ),
            ],
        );
        let before = palette.entries().to_vec();
        palette.prioritize_target(RuntimeNodeId::new(2), &locator);
        assert_eq!(palette.entries().len(), before.len());
        let mut current = before[1].clone();
        current.section = Some(ActionSection::Current);
        assert_eq!(palette.entries()[0], current);
        assert_eq!(
            palette.entries()[1..],
            [before[0].clone(), before[2].clone()]
        );
    }

    #[test]
    fn presentation_sections_preserve_every_operation_and_global_search() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let locator = |id| BackendLocator::new(":1.2", format!("/{id}"));
        let mut palette = CommandPalette::public_actions(
            CommandHierarchy {
                root: CommandGroup {
                    source: RuntimeNodeId::new(1),
                    label: "Commands".into(),
                    scope,
                    children: vec![],
                },
            },
            scope,
            vec![
                (
                    RuntimeNodeId::new(1),
                    locator(1),
                    "Scroll value".into(),
                    PublicOperation::SetValue,
                ),
                (
                    RuntimeNodeId::new(2),
                    locator(2),
                    "Save".into(),
                    PublicOperation::Action("press".into()),
                ),
                (
                    RuntimeNodeId::new(3),
                    locator(3),
                    "Query".into(),
                    PublicOperation::EditText { multiline: false },
                ),
                (
                    RuntimeNodeId::new(4),
                    locator(4),
                    "Select item".into(),
                    PublicOperation::Selection {
                        parent: locator(5),
                        selected: true,
                    },
                ),
            ],
        );
        let before = palette
            .entries()
            .iter()
            .map(|e| (e.label.clone(), e.target.clone(), e.action.clone()))
            .collect::<Vec<_>>();
        palette.group_public_actions(|id| id == RuntimeNodeId::new(1));
        palette.prioritize_target(RuntimeNodeId::new(3), &locator(3));
        assert_eq!(palette.entries().len(), before.len());
        assert_eq!(
            palette
                .entries()
                .iter()
                .map(PaletteEntry::section_label)
                .collect::<Vec<_>>(),
            vec![
                Some("Current object"),
                Some("Controls"),
                Some("Interface operations"),
                Some("Interface operations")
            ]
        );
        for entry in palette.entries() {
            assert!(before.contains(&(
                entry.label.clone(),
                entry.target.clone(),
                entry.action.clone()
            )));
        }
        palette.set_query("Scroll value".into());
        assert_eq!(palette.entries().len(), 1);
        assert_eq!(
            palette.entries()[0].runtime_target().unwrap().0,
            RuntimeNodeId::new(1)
        );
    }

    #[test]
    fn navigation_predicate_survives_rebuild_and_human_typing_clears_it() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let actions = vec![
            (
                RuntimeNodeId::new(2),
                BackendLocator::new(":1.2", "/status"),
                "Statusbar [CheckBox] · View".to_owned(),
                PublicOperation::Action("click".into()),
            ),
            (
                RuntimeNodeId::new(3),
                BackendLocator::new(":1.2", "/file"),
                "File [Menu] · /tmp/statusbar-results".to_owned(),
                PublicOperation::Action("click".into()),
            ),
        ];
        let hierarchy = CommandHierarchy {
            root: CommandGroup {
                source: RuntimeNodeId::new(1),
                label: "root".into(),
                scope,
                children: Vec::new(),
            },
        };
        let mut previous =
            CommandPalette::public_actions(hierarchy.clone(), scope, actions.clone());
        previous.set_navigation_query("Statusbar".into(), Some("View".into()), true);
        previous.retain_entries(|entry| entry.label.starts_with("Statusbar "));
        assert_eq!(previous.entries().len(), 1);
        let mut refreshed = CommandPalette::public_actions(hierarchy, scope, actions);
        refreshed.restore_query_and_exact_selection(&previous);
        assert_eq!(
            refreshed.navigation_query(),
            Some(("Statusbar".into(), Some("View".into()), true))
        );
        refreshed.retain_entries(|entry| entry.label.starts_with("Statusbar "));
        assert_eq!(refreshed.entries().len(), 1);
        assert_eq!(
            refreshed.entries()[0].runtime_target().unwrap().0,
            RuntimeNodeId::new(2)
        );
        refreshed.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(refreshed.navigation_query(), None);
    }
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
    fn compact_titles_preserve_full_search_paged_details_and_exact_target() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let label = format!(
            "{} needle-at-the-end › Action: press",
            "中文e\u{301}".repeat(120)
        );
        let mut palette = CommandPalette::public_actions(
            CommandHierarchy {
                root: CommandGroup {
                    source: RuntimeNodeId::new(1),
                    label: "Workspace".into(),
                    scope,
                    children: vec![],
                },
            },
            scope,
            vec![
                (
                    RuntimeNodeId::new(2),
                    BackendLocator::new(":1.2", "/first"),
                    label.clone(),
                    PublicOperation::Action("press".into()),
                ),
                (
                    RuntimeNodeId::new(3),
                    BackendLocator::new(":1.2", "/second"),
                    "Other target › Action: press".into(),
                    PublicOperation::Action("press".into()),
                ),
            ],
        );
        palette.set_target_titles(|_| Some("Same title".into()));
        palette.handle_key(KeyEvent::from(KeyCode::F(2)));
        assert!(!palette.searches_all_scopes());
        assert!(!palette.scope_switchable());
        // Same public titles/actions remain ambiguous; numeric IDs belong in
        // source details and must not manufacture a semantic distinction.
        assert_eq!(
            palette.entries()[0].short_label(),
            palette.entries()[1].short_label()
        );
        let text = palette.entries()[0].detail_text();
        let lines = detail_lines(&text, 20);
        assert!(
            lines
                .iter()
                .all(|line| ratatui::text::Line::from(line.as_str()).width() <= 20)
        );
        assert_eq!(lines.concat(), text.replace('\n', ""));
        palette.set_detail_viewport(20, 3);
        for _ in 0..100 {
            palette.handle_key(KeyEvent::from(KeyCode::PageDown));
        }
        assert_eq!(palette.detail_scroll(), lines.len() - 3);
        let old = palette.clone();
        let mut refreshed = palette.clone();
        refreshed.set_query(String::new());
        refreshed.restore_query_and_exact_selection(&old);
        refreshed.set_detail_viewport(20, 3);
        assert_eq!(refreshed.detail_scroll(), old.detail_scroll());
        assert_eq!(palette.selected(), 0);
        palette.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(palette.detail_scroll(), 0);
        for c in "needle-at-the-end".chars() {
            palette.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
        assert_eq!(palette.entries().len(), 1);
        assert_eq!(palette.entries()[0].label, label);
        assert!(matches!(palette.handle_key(KeyEvent::from(KeyCode::Enter)),
            PaletteOutcome::InvokeAction(id, locator, PublicOperation::Action(name))
                if id == RuntimeNodeId::new(2) && locator == BackendLocator::new(":1.2", "/first") && name == "press"));
        // Human-visible titles remain searchable even when their word order
        // differs from the full public label shown in Details.
        palette.set_query("Same title".into());
        assert_eq!(palette.entries().len(), 2);
    }

    #[test]
    fn named_action_is_returned_without_semantic_guessing() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let hierarchy = CommandHierarchy {
            root: CommandGroup {
                source: RuntimeNodeId::new(1),
                label: "App".into(),
                scope,
                children: vec![],
            },
        };
        let mut palette = CommandPalette::public_actions(
            hierarchy,
            scope,
            vec![(
                RuntimeNodeId::new(2),
                BackendLocator::new(":1.2", "/target"),
                "Target › Action: delete".into(),
                PublicOperation::Action("delete".into()),
            )],
        );
        assert!(
            matches!(palette.handle_key(KeyEvent::from(KeyCode::Enter)), PaletteOutcome::InvokeAction(_, _, PublicOperation::Action(name)) if name == "delete")
        );
    }

    #[test]
    fn all_commands_remain_reachable_beyond_the_old_display_limit() {
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let hierarchy = CommandHierarchy {
            root: CommandGroup {
                source: RuntimeNodeId::new(1),
                label: "App".into(),
                scope,
                children: (2..42).map(|id| command(id, scope, "Same label")).collect(),
            },
        };
        let mut palette = CommandPalette::new(hierarchy, scope, HashMap::new());
        assert_eq!(palette.entries().len(), 40);
        for _ in 0..39 {
            palette.handle_key(KeyEvent::from(KeyCode::Down));
        }
        assert!(matches!(palette.handle_key(KeyEvent::from(KeyCode::Enter)),
            PaletteOutcome::Execute(id, _, _) if id == RuntimeNodeId::new(41)));
        palette.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(palette.entries().len(), 40);
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
        palette.handle_key(KeyEvent::from(KeyCode::F(2)));
        assert_eq!(palette.entries().len(), 2);
        palette.handle_key(KeyEvent::from(KeyCode::F(2)));
        assert_eq!(palette.entries().len(), 1);
        palette.handle_key(KeyEvent::from(KeyCode::Char('c')));
        assert_eq!(palette.entries().len(), 1);
        assert!(palette.entries()[0].label.contains("Close dialog"));

        palette.handle_key(KeyEvent::from(KeyCode::F(2)));
        assert!(palette.searches_all_scopes());
        assert_eq!(palette.entries().len(), 2);
    }
}
