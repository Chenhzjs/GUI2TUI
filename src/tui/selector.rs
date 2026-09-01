use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorIntent {
    Next,
    Previous,
    Open,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectorTarget {
    Running(String),
    Launcher(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SelectorEntry {
    label: String,
    search_key: String,
    target: SelectorTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectorHitRegion {
    index: usize,
    rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationSelector {
    entries: Vec<SelectorEntry>,
    query: String,
    filtering: bool,
    message: Option<String>,
    selected: usize,
    hit_regions: Vec<SelectorHitRegion>,
}

impl ApplicationSelector {
    pub fn new(applications: Vec<String>) -> Self {
        Self {
            entries: applications
                .into_iter()
                .map(|name| SelectorEntry {
                    label: format!("[running] {name}"),
                    search_key: name.clone(),
                    target: SelectorTarget::Running(name),
                })
                .collect(),
            query: String::new(),
            filtering: false,
            message: None,
            selected: 0,
            hit_regions: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn with_launchers(applications: Vec<String>, launchers: Vec<String>) -> Self {
        let mut selector = Self::new(applications);
        selector
            .entries
            .extend(launchers.into_iter().map(|id| SelectorEntry {
                label: format!("[launch]  {id}"),
                search_key: id.clone(),
                target: SelectorTarget::Launcher(id),
            }));
        selector
    }

    pub fn selected_target(&self) -> Option<&SelectorTarget> {
        self.filtered()
            .nth(self.selected)
            .map(|entry| &entry.target)
    }

    fn filtered(&self) -> impl Iterator<Item = &SelectorEntry> {
        self.entries.iter().filter(|entry| {
            entry
                .search_key
                .to_lowercase()
                .contains(&self.query.to_lowercase())
        })
    }

    pub fn replace(
        &mut self,
        applications: Vec<String>,
        launchers: Vec<String>,
        message: Option<String>,
    ) {
        let selected = self.selected_target().cloned();
        *self = Self::with_launchers(applications, launchers);
        self.message = message;
        let index = self
            .filtered()
            .position(|entry| Some(&entry.target) == selected.as_ref())
            .unwrap_or(0);
        self.selected = index;
        self.hit_regions.clear();
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    /// Text input takes precedence over navigation shortcuts while filtering.
    pub fn filter_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return false;
        }
        if key.kind == KeyEventKind::Release {
            return true;
        }
        if key.code == KeyCode::Char('/') && !self.filtering {
            self.filtering = true;
            return true;
        }
        if !self.filtering {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.filtering = false;
                self.query.clear();
            }
            KeyCode::Enter => {
                self.filtering = false;
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty()
                    || key.modifiers == crossterm::event::KeyModifiers::SHIFT =>
            {
                self.query.push(character)
            }
            _ => {}
        }
        self.selected = 0;
        self.hit_regions.clear();
        true
    }

    pub fn handle(&mut self, intent: SelectorIntent) -> Option<SelectorTarget> {
        let count = self.filtered().count();
        match intent {
            SelectorIntent::Next if count > 0 => {
                self.selected = (self.selected + 1) % count;
                None
            }
            SelectorIntent::Previous if count > 0 => {
                self.selected = if self.selected == 0 {
                    count - 1
                } else {
                    self.selected - 1
                };
                None
            }
            SelectorIntent::Open => self.selected_target().cloned(),
            SelectorIntent::Quit | SelectorIntent::Next | SelectorIntent::Previous => None,
        }
    }

    pub fn click(&mut self, x: u16, y: u16) -> Option<SelectorTarget> {
        let index = self
            .hit_regions
            .iter()
            .find(|region| contains(region.rect, x, y))?
            .index;
        self.selected = index;
        self.selected_target().cloned()
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let areas =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());
        let main = areas[0];
        let footer = areas[1];
        let block = Block::default()
            .title(if self.query.is_empty() && !self.filtering {
                " Select application ".to_owned()
            } else {
                format!(
                    " Select application /{}{} ",
                    self.query,
                    if self.filtering { " [filtering]" } else { "" }
                )
            })
            .borders(Borders::ALL);
        let inner = block.inner(main);
        frame.render_widget(block, main);

        self.hit_regions.clear();
        let start = self
            .selected
            .saturating_sub(inner.height.saturating_sub(1) as usize);
        let entries: Vec<_> = self.filtered().cloned().collect();
        if entries.is_empty() {
            let message = self.message.as_deref().unwrap_or(if self.query.is_empty() {
                "No accessible applications found. Start a GUI application in the same desktop session.\n\n[d] diagnostics  [r] refresh  [q] quit"
            } else { "No applications match this filter. Press / then Esc to clear." });
            frame.render_widget(
                Paragraph::new(message).wrap(ratatui::widgets::Wrap { trim: true }),
                inner,
            );
        }
        for (index, entry) in entries
            .iter()
            .enumerate()
            .skip(start)
            .take(inner.height as usize)
        {
            let selected = index == self.selected;
            let row = Rect::new(
                inner.x,
                inner.y.saturating_add((index - start) as u16),
                inner.width,
                1,
            );
            let marker = if selected { "> " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(
                Paragraph::new(format!("{marker}{}", entry.label)).style(style),
                row,
            );
            self.hit_regions
                .push(SelectorHitRegion { index, rect: row });
        }
        frame.render_widget(
            Paragraph::new(if let Some(message) = self.message.as_deref() {
                message
            } else if self.filtering {
                "Type filter | Enter Apply | Esc Clear"
            } else {
                "↑/↓ Select | Enter Open/Launch | / Filter | r/F5 Refresh | d Diagnose | q Quit"
            })
            .style(Style::default().fg(Color::Cyan)),
            footer,
        );
    }
}

pub fn key_to_selector_intent(event: KeyEvent) -> Option<SelectorIntent> {
    if event.kind == KeyEventKind::Release {
        return None;
    }
    if event.code == KeyCode::Char('c')
        && event
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        return Some(SelectorIntent::Quit);
    }
    match event.code {
        KeyCode::Down | KeyCode::Char('j') => Some(SelectorIntent::Next),
        KeyCode::Up | KeyCode::Char('k') => Some(SelectorIntent::Previous),
        KeyCode::Enter => Some(SelectorIntent::Open),
        KeyCode::Char('q') | KeyCode::Esc => Some(SelectorIntent::Quit),
        _ => None,
    }
}

pub fn mouse_click(event: MouseEvent) -> Option<(u16, u16)> {
    (event.kind == MouseEventKind::Down(MouseButton::Left)).then_some((event.column, event.row))
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn selector_filter_shortcuts_and_refresh_preserve_selection() {
        use crossterm::event::KeyModifiers;
        let mut selector =
            ApplicationSelector::new(vec!["Example One".into(), "Example Browser".into()]);
        for character in ['/', 'r'] {
            assert!(
                selector.filter_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
            );
        }
        assert_eq!(
            selector.selected_target(),
            Some(&SelectorTarget::Running("Example Browser".into()))
        );
        selector.filter_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        selector.replace(
            vec!["Example Other".into(), "Example Browser".into()],
            Vec::new(),
            None,
        );
        assert_eq!(
            selector.selected_target(),
            Some(&SelectorTarget::Running("Example Browser".into()))
        );
    }

    #[test]
    fn selector_scroll_keeps_last_item_visible() {
        let mut terminal = Terminal::new(TestBackend::new(80, 6)).unwrap();
        let mut selector = ApplicationSelector::new((0..30).map(|i| format!("App{i}")).collect());
        selector.handle(SelectorIntent::Previous);
        terminal.draw(|frame| selector.render(frame)).unwrap();
        assert!(selector.hit_regions.iter().any(|region| region.index == 29));
    }

    #[test]
    fn selector_navigation_wraps_and_opens_the_selected_application() {
        let mut selector =
            ApplicationSelector::new(vec!["Example One".to_owned(), "Example Two".to_owned()]);
        selector.handle(SelectorIntent::Previous);
        assert_eq!(
            selector.selected_target(),
            Some(&SelectorTarget::Running("Example Two".into()))
        );
        selector.handle(SelectorIntent::Next);
        assert_eq!(
            selector.selected_target(),
            Some(&SelectorTarget::Running("Example One".into()))
        );
        assert_eq!(
            selector.handle(SelectorIntent::Open),
            Some(SelectorTarget::Running("Example One".to_owned()))
        );
    }

    #[test]
    fn selector_terminal_hit_testing_opens_the_clicked_row() {
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut selector =
            ApplicationSelector::new(vec!["Example One".to_owned(), "Example Two".to_owned()]);
        terminal.draw(|frame| selector.render(frame)).unwrap();

        assert_eq!(
            selector.click(2, 2),
            Some(SelectorTarget::Running("Example Two".to_owned()))
        );
        assert_eq!(
            selector.selected_target(),
            Some(&SelectorTarget::Running("Example Two".into()))
        );
        assert!(selector.click(39, 7).is_none());
    }

    #[test]
    fn selector_exposes_registered_launchers_distinctly() {
        let mut selector = ApplicationSelector::with_launchers(
            vec!["Example App".into()],
            vec!["example-launcher".into()],
        );
        selector.handle(SelectorIntent::Next);
        assert_eq!(
            selector.handle(SelectorIntent::Open),
            Some(SelectorTarget::Launcher("example-launcher".into()))
        );
    }
}
