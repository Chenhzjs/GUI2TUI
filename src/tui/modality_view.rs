use crate::modality::{
    ExternalModality, LocalModalityCapabilities, ModalityCandidate, ModalityKind,
    ModalityResolution,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
        let block = Block::default().title(" Resources ").borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let rows = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner);
        let height = rows[0].height as usize;
        let start = self.selected.saturating_sub(height.saturating_sub(1));
        let mut lines = Vec::new();
        for (index, candidate) in self.candidates.iter().enumerate().skip(start).take(height) {
            lines.push(format!(
                "{} {}: {}",
                if index == self.selected { ">" } else { " " },
                resource_kind(candidate.kind),
                candidate.label.as_deref().unwrap_or("Unnamed resource")
            ));
        }
        if lines.is_empty() {
            lines.push("No resources exposed in this part of the application.".to_owned());
        }
        if self.file_input.is_some() {
            lines = vec![
                "Choose a file on this computer: enter its full path".into(),
                "Esc cancels; Enter sends. File may differ from unsaved GUI content.".into(),
                String::new(),
            ];
        }
        let body = Paragraph::new(lines.join("\n"));
        let body = if self.candidates.is_empty() || self.file_input.is_some() {
            body.wrap(Wrap { trim: false })
        } else {
            body
        };
        frame.render_widget(body, rows[0]);
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
            "Remote viewer: opening a reference here is unavailable; file transfer needs a receiver."
        } else {
            match &self.resolved {
                Some(modality) if modality.capabilities.reference_handoff => {
                    "Open in the connected viewer — confirmation required"
                }
                Some(modality) => match &modality.resolution {
                    ModalityResolution::LiveVisualState { .. } => {
                        "Live view: no file or saved preview is available"
                    }
                    ModalityResolution::Unavailable { .. } => {
                        "Original file unavailable; m captures an image preview (may include overlapping windows)"
                    }
                    ModalityResolution::RenderedSnapshot(_) => {
                        "Image preview saved on this computer; not the original file"
                    }
                    _ if self.capabilities.is_none() => {
                        "Reference available: Enter for details; no viewer connected"
                    }
                    _ => "No compatible viewer for this resource; details remain available",
                },
                None if self.candidates.is_empty() => {
                    if self
                        .capabilities
                        .as_ref()
                        .is_some_and(|cap| cap.artifact_receive)
                    {
                        "A file receiver is connected; f lets you choose a file"
                    } else {
                        "File sending requires a connected file receiver"
                    }
                }
                None => "Select a resource to see its details",
            }
        };
        let receiver = self
            .capabilities
            .as_ref()
            .is_some_and(|cap| cap.artifact_receive);
        let info = if super::renderer::normal_footer_status(status) {
            format!("{availability}\n{status}")
        } else {
            availability.to_owned()
        };
        frame.render_widget(Paragraph::new(info).wrap(Wrap { trim: false }), rows[1]);
        let hints = if self.file_input.is_some() && !receiver {
            "Esc Cancel · File receiver required"
        } else if self.file_input.is_some() {
            "Esc Cancel · Enter Send"
        } else if self.candidates.is_empty() && receiver {
            "Esc Back · f Send file · F1 Help"
        } else if self.candidates.is_empty() {
            "Esc Back · F1 Help"
        } else if self
            .resolved
            .as_ref()
            .is_some_and(|m| m.capabilities.reference_handoff)
        {
            "Esc Back · ↑↓ Select · Enter Open · F1 Help"
        } else {
            "Esc Back · ↑↓ Select · Enter Details · F1 Help"
        };
        frame.render_widget(Paragraph::new(hints), rows[2]);
    }
}

fn resource_kind(kind: ModalityKind) -> &'static str {
    match kind {
        ModalityKind::Image => "Image",
        ModalityKind::Document => "Document",
        ModalityKind::Video => "Video",
        ModalityKind::Audio => "Audio",
        ModalityKind::VectorGraphic => "Drawing",
        ModalityKind::PortableModel => "3D model",
        ModalityKind::LiveVisual => "Live view",
        ModalityKind::Unknown => "Resource",
    }
}

// Keep the insertion point visible, measuring terminal cells rather than bytes.
fn input_window(input: &super::edit::EditBuffer, width: u16) -> (String, u16) {
    let (text, cursor_byte) = input.visible_window(width);
    let cursor = ratatui::text::Span::raw(&text[..cursor_byte]).width() as u16;
    (text, cursor)
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
    fn empty_resources_show_plain_state_and_return_without_fake_actions() {
        let view = ModalityView {
            remote: false,
            transfer_progress: None,
            file_input: None,
            candidates: vec![],
            selected: 0,
            resolved: None,
            capabilities: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| view.render(frame, "Loaded 20 semantic nodes via cache"))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(output.contains("Resources"));
        assert!(output.contains("Esc Back"));
        assert!(!output.contains("Loaded 20"));
        assert!(!output.contains("Enter Open"));
        assert!(!output.contains("Snapshot"));
    }

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
        assert!(output.contains("Reference available"));
        assert!(!output.contains("Enter Open"));
        view.move_selection(1);
        assert!(view.resolved.is_none());
    }
}
