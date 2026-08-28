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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectorHitRegion {
    index: usize,
    rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationSelector {
    applications: Vec<String>,
    selected: usize,
    hit_regions: Vec<SelectorHitRegion>,
}

impl ApplicationSelector {
    pub fn new(applications: Vec<String>) -> Self {
        Self {
            applications,
            selected: 0,
            hit_regions: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.applications.is_empty()
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.applications.get(self.selected).map(String::as_str)
    }

    pub fn handle(&mut self, intent: SelectorIntent) -> Option<String> {
        match intent {
            SelectorIntent::Next if !self.applications.is_empty() => {
                self.selected = (self.selected + 1) % self.applications.len();
                None
            }
            SelectorIntent::Previous if !self.applications.is_empty() => {
                self.selected = if self.selected == 0 {
                    self.applications.len() - 1
                } else {
                    self.selected - 1
                };
                None
            }
            SelectorIntent::Open => self.selected_name().map(str::to_owned),
            SelectorIntent::Quit | SelectorIntent::Next | SelectorIntent::Previous => None,
        }
    }

    pub fn click(&mut self, x: u16, y: u16) -> Option<String> {
        let index = self
            .hit_regions
            .iter()
            .find(|region| contains(region.rect, x, y))?
            .index;
        self.selected = index;
        self.selected_name().map(str::to_owned)
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        let areas =
            Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());
        let main = areas[0];
        let footer = areas[1];
        let block = Block::default()
            .title(" Select application ")
            .borders(Borders::ALL);
        let inner = block.inner(main);
        frame.render_widget(block, main);

        self.hit_regions.clear();
        for (index, name) in self
            .applications
            .iter()
            .take(inner.height as usize)
            .enumerate()
        {
            let selected = index == self.selected;
            let row = Rect::new(
                inner.x,
                inner.y.saturating_add(index as u16),
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
            frame.render_widget(Paragraph::new(format!("{marker}{name}")).style(style), row);
            self.hit_regions
                .push(SelectorHitRegion { index, rect: row });
        }
        frame.render_widget(
            Paragraph::new("↑/↓ or j/k Select | Enter/Click Open | q/Esc Quit")
                .style(Style::default().fg(Color::Cyan)),
            footer,
        );
    }
}

pub fn key_to_selector_intent(event: KeyEvent) -> Option<SelectorIntent> {
    if event.kind == KeyEventKind::Release {
        return None;
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
    fn selector_navigation_wraps_and_opens_the_selected_application() {
        let mut selector = ApplicationSelector::new(vec!["GTK".to_owned(), "Qt".to_owned()]);
        selector.handle(SelectorIntent::Previous);
        assert_eq!(selector.selected_name(), Some("Qt"));
        selector.handle(SelectorIntent::Next);
        assert_eq!(selector.selected_name(), Some("GTK"));
        assert_eq!(
            selector.handle(SelectorIntent::Open),
            Some("GTK".to_owned())
        );
    }

    #[test]
    fn selector_terminal_hit_testing_opens_the_clicked_row() {
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut selector = ApplicationSelector::new(vec!["GTK".to_owned(), "Qt".to_owned()]);
        terminal.draw(|frame| selector.render(frame)).unwrap();

        assert_eq!(selector.click(2, 2), Some("Qt".to_owned()));
        assert_eq!(selector.selected_name(), Some("Qt"));
        assert!(selector.click(39, 7).is_none());
    }
}
