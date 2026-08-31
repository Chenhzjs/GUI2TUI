use crate::modality::{
    ExternalModality, LocalModalityCapabilities, ModalityCandidate, ModalityResolution,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct ModalityView {
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
        let area = frame.area();
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" External modality — original content ")
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
        frame.render_widget(Paragraph::new(lines.join("\n")), rows[0]);
        let availability = match &self.resolved {
            Some(modality) if modality.capabilities.reference_handoff => {
                "[Open locally] — approval required in local broker"
            }
            Some(modality) => match &modality.resolution {
                ModalityResolution::LiveVisualState { .. } => {
                    "Live graphical state — no portable representation"
                }
                ModalityResolution::Unavailable { .. } => {
                    "Original modality UNRESOLVED (read-only)"
                }
                _ if self.capabilities.is_none() => "Local modality client unavailable (read-only)",
                _ => "No matching local handler or permitted resource scheme (read-only)",
            },
            None => "No resolved resource",
        };
        frame.render_widget(Paragraph::new(format!("{availability}\n{status}\n↑/↓ Choose resource | Enter Open if available | Esc Return (GUI unchanged)")), rows[1]);
    }
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
        assert!(output.contains("Local modality client unavailable"));
        assert!(!output.contains("[Open locally]"));
        view.move_selection(1);
        assert!(view.resolved.is_none());
    }
}
