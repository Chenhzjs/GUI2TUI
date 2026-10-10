use crate::modality::{
    ExternalModality, LocalModalityCapabilities, ModalityCandidate, ModalityResolution,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ModalityView {
    pub remote: bool,
    pub transfer_progress: Option<std::sync::Arc<std::sync::Mutex<String>>>,
    pub file_input: Option<super::edit::EditBuffer>,
    pub candidates: Vec<ModalityCandidate>,
    pub selected: usize,
    pub resolved: Option<ExternalModality>,
    pub capabilities: Option<LocalModalityCapabilities>,
}

impl ModalityView {
    pub fn move_selection(&mut self, delta: isize) {
        if self.candidates.is_empty() {
            return;
        }
        self.selected =
            (self.selected as isize + delta).rem_euclid(self.candidates.len() as isize) as usize;
        self.resolved = None;
    }

    pub fn render(&self, frame: &mut Frame<'_>, status: &str) {
        let progress = self
            .transfer_progress
            .as_ref()
            .and_then(|value| value.lock().ok().map(|s| s.clone()));
        let status = progress.as_deref().unwrap_or(status);
        let area = frame.area();
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" External modality — references / explicit static snapshots ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(4)]).split(inner);
        let height = rows[0].height as usize;
        let start = self.selected.saturating_sub(height.saturating_sub(1));
        let mut lines = Vec::new();
        for (index, candidate) in self.candidates.iter().enumerate().skip(start).take(height) {
            lines.push(format!(
                "{} {:?}: {:?}",
                if index == self.selected { ">" } else { " " },
                candidate.kind,
                candidate.label.as_deref().unwrap_or("Unnamed resource")
            ));
        }
        if lines.is_empty() {
            lines.push("No external modality objects exposed in the active scope".to_owned());
        }
        if self.file_input.is_some() {
            lines = vec![
                "Send user-selected file: absolute path on this host".into(),
                "Enter sends; Esc cancels input; file may differ from unsaved GUI".into(),
                String::new(),
            ];
        }
        frame.render_widget(Paragraph::new(lines.join("\n")), rows[0]);
        if let Some(input) = &self.file_input
            && rows[0].height > 2
            && rows[0].width > 2
        {
            let width = rows[0].width - 2;
            let (text, cursor) = input_window(input, width);
            let line = ratatui::layout::Rect::new(rows[0].x, rows[0].y + 2, rows[0].width, 1);
            frame.render_widget(Paragraph::new(format!("> {text}")), line);
            frame.set_cursor_position((line.x + 2 + cursor, line.y));
        }
        let availability = if self.remote {
            "Remote endpoint: f transfers file bytes; Enter/o path handoff disabled"
        } else {
            match &self.resolved {
                Some(modality) if modality.capabilities.reference_handoff => {
                    "[Open locally] — approval required in local broker"
                }
                Some(modality) => match &modality.resolution {
                    ModalityResolution::LiveVisualState { .. } => {
                        "Live graphical state — no portable representation"
                    }
                    ModalityResolution::Unavailable { .. } => {
                        "Original UNRESOLVED; m Request Image region snapshot (may be occluded)"
                    }
                    ModalityResolution::RenderedSnapshot(_) => {
                        "RenderedSnapshot on host; not original bytes; o Same-host viewer if configured"
                    }
                    _ if self.capabilities.is_none() => {
                        "Headless reference: Enter Inspect; no endpoint required"
                    }
                    _ => "No matching local handler or permitted resource scheme (read-only)",
                },
                None => "No resolved resource",
            }
        };
        frame.render_widget(Paragraph::new(format!("{availability}\n{status}\nf Send file | x Cancel | Enter Reference | m Snapshot | o Same-host | Esc Return")), rows[1]);
    }
}

// Keep the insertion point visible, measuring terminal cells rather than bytes.
fn input_window(input: &super::edit::EditBuffer, width: u16) -> (String, u16) {
    let chars: Vec<char> = input.text().chars().collect();
    let cursor = input.cursor().min(chars.len());
    let cells = |c: char| ratatui::text::Span::raw(c.to_string()).width();
    let available = usize::from(width.saturating_sub(1));
    let mut start = cursor;
    let mut before = 0;
    while start > 0 && before + cells(chars[start - 1]) <= available {
        start -= 1;
        before += cells(chars[start]);
    }
    let mut text = String::new();
    let mut used = 0;
    for &c in &chars[start..] {
        let size = cells(c);
        if used + size > usize::from(width) {
            break;
        }
        text.push(c);
        used += size;
    }
    (text, before as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modality::{ModalityKind, ModalityMetadata, ModalityResolver},
        semantic::{BackendLocator, RuntimeNodeId},
    };
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn reopened_view_shows_live_task_progress() {
        let progress =
            std::sync::Arc::new(std::sync::Mutex::new("Transferring 1/100 bytes".to_owned()));
        let make_view = || ModalityView {
            remote: true,
            transfer_progress: Some(progress.clone()),
            file_input: None,
            candidates: vec![],
            selected: 0,
            resolved: None,
            capabilities: None,
        };
        drop(make_view());
        let view = make_view();
        *progress.lock().unwrap() = "Transferring 80/100 bytes".into();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| view.render(frame, "idle")).unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("Transferring 80/100 bytes"));
    }

    #[test]
    fn long_unicode_path_keeps_cursor_and_filename_visible() {
        let mut input =
            super::super::edit::EditBuffer::new(format!("/{}文件?.pdf", "long/".repeat(30)));
        let (text, cursor) = input_window(&input, 20);
        assert!(text.ends_with("文件?.pdf"));
        assert!(cursor < 20);
        assert_eq!(ratatui::text::Span::raw(&text).width(), usize::from(cursor));
        input.home();
        let (text, cursor) = input_window(&input, 20);
        assert!(text.starts_with("/long/"));
        assert_eq!(cursor, 0);
    }

    #[test]
    fn no_connected_client_never_renders_a_fake_open_button() {
        let candidate = ModalityCandidate {
            owner: RuntimeNodeId::new(1),
            locator: BackendLocator::new(":1.2", "/image"),
            evidence_locators: vec![],
            kind: ModalityKind::Image,
            label: Some("Image".into()),
        };
        let resolved = ModalityResolver::default().resolve(
            &candidate,
            &[ModalityMetadata {
                hyperlink_uris: vec!["https://example.invalid/a.png".into()],
                ..Default::default()
            }],
        );
        let mut view = ModalityView {
            remote: false,
            transfer_progress: None,
            file_input: None,
            candidates: vec![candidate],
            selected: 0,
            resolved: Some(resolved),
            capabilities: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
        terminal.draw(|frame| view.render(frame, "test")).unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("Headless reference"));
        assert!(!output.contains("[Open locally]"));
        view.move_selection(1);
        assert!(view.resolved.is_none());
    }
}
