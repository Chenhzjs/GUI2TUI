use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::semantic::RuntimeNodeId;

use super::{
    action::InteractionCapability,
    edit::EditSession,
    hit_test::{HitInteraction, HitRegion},
    view_model::{TuiElement, TuiElementKind, TuiViewModel},
};

pub struct RenderContext<'a> {
    pub view: &'a TuiViewModel,
    pub focused: Option<RuntimeNodeId>,
    pub scroll_offset: u16,
    pub status: &'a str,
    pub application_available: bool,
    pub edit_session: Option<&'a EditSession>,
}

pub fn render(frame: &mut Frame<'_>, context: RenderContext<'_>) -> Vec<HitRegion> {
    let areas = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());
    let main_area = areas[0];
    let footer_area = areas[1];

    let block = Block::default()
        .title(format!(" GUI2TUI — {} ", context.view.title))
        .borders(Borders::ALL);
    let content_area = block.inner(main_area);
    frame.render_widget(block, main_area);

    let mut hit_regions = Vec::new();
    if context.application_available {
        render_elements(frame, content_area, &context, &mut hit_regions);
    } else {
        frame.render_widget(
            Paragraph::new("Application is no longer available. Press r to retry or q to quit."),
            content_area,
        );
    }

    let footer = format!(
        "{} | Tab/Shift-Tab Focus | Enter/Space Operate | ↑/↓ Scroll | r Refresh | q Quit",
        context.status
    );
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::Cyan)),
        footer_area,
    );
    hit_regions
}

fn render_elements(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    hit_regions: &mut Vec<HitRegion>,
) {
    let visible_bottom = context.scroll_offset.saturating_add(area.height);
    let mut logical_top = 0_u16;

    for element in &context.view.elements {
        let focused = context.focused == Some(element.runtime_id);
        let lines = element_lines_for_width(element, focused, context.edit_session, area.width);
        let mut first_y = None;
        let mut visible_height = 0_u16;

        for (line_index, line) in lines.into_iter().enumerate() {
            let logical_y = logical_top.saturating_add(line_index as u16);
            if logical_y < context.scroll_offset || logical_y >= visible_bottom {
                continue;
            }
            let y = area
                .y
                .saturating_add(logical_y.saturating_sub(context.scroll_offset));
            let line_area = Rect::new(area.x, y, area.width, 1);
            let style = if focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(Paragraph::new(line).style(style), line_area);
            first_y.get_or_insert(y);
            visible_height = visible_height.saturating_add(1);
        }

        if let (Some(y), Some(interaction)) = (first_y, interaction(element)) {
            hit_regions.push(HitRegion {
                runtime_id: element.runtime_id,
                rect: Rect::new(area.x, y, area.width, visible_height),
                interaction,
            });
        }
        logical_top = logical_top.saturating_add(element.height());
    }
}

fn interaction(element: &TuiElement) -> Option<HitInteraction> {
    match element.kind {
        TuiElementKind::Button { .. }
        | TuiElementKind::ToggleButton { .. }
        | TuiElementKind::CheckBox { .. }
        | TuiElementKind::ListItem { .. }
        | TuiElementKind::MenuItem { .. } => {
            Some(if element.capability == InteractionCapability::None {
                HitInteraction::Unavailable
            } else {
                HitInteraction::Activate
            })
        }
        TuiElementKind::TextInput { .. } | TuiElementKind::ComboBox { .. } => {
            Some(HitInteraction::Focus)
        }
        _ => None,
    }
}

pub fn element_lines(element: &TuiElement, focused: bool) -> Vec<String> {
    element_lines_for_width(element, focused, None, u16::MAX)
}

fn element_lines_for_width(
    element: &TuiElement,
    focused: bool,
    edit_session: Option<&EditSession>,
    width: u16,
) -> Vec<String> {
    let marker = if focused { "> " } else { "  " };
    let unavailable = if element.capability == InteractionCapability::None {
        "  (read-only)"
    } else {
        ""
    };
    match &element.kind {
        TuiElementKind::Label { text } => vec![format!("  {text}")],
        TuiElementKind::Group { label } => vec![format!("  {label}:")],
        TuiElementKind::Button { label } => vec![format!("{marker}[ {label} ]{unavailable}")],
        TuiElementKind::ToggleButton { label, pressed } => vec![format!(
            "{marker}[{} {label}]{unavailable}",
            if *pressed { "*" } else { " " },
        )],
        TuiElementKind::CheckBox { label, checked } => vec![format!(
            "{marker}[{}] {label}{unavailable}",
            if *checked { "x" } else { " " },
        )],
        TuiElementKind::TextInput { label, display, .. } => {
            if let Some(session) = edit_session.filter(|edit| edit.target == element.runtime_id) {
                vec![
                    format!("{marker}{label}  [editing]"),
                    format!("    > {}", edit_buffer_window(session, width)),
                ]
            } else {
                vec![
                    format!("{marker}{label}{unavailable}"),
                    format!("    > {display}"),
                ]
            }
        }
        TuiElementKind::ComboBox { label } => {
            vec![format!("{marker}[ {label} ▼ ]{unavailable}")]
        }
        TuiElementKind::List { label } => vec![format!("  {label}:")],
        TuiElementKind::ListItem { label, selected } => {
            let selection_marker = if *selected { "*" } else { "•" };
            vec![format!("{marker}{selection_marker} {label}{unavailable}")]
        }
        TuiElementKind::MenuBar => vec!["  Menu:".to_owned()],
        TuiElementKind::Menu { label } => vec![format!("  {label}:")],
        TuiElementKind::MenuItem { label, opens_menu } => vec![format!(
            "{marker}{label}{}{unavailable}",
            if *opens_menu { " >" } else { "" }
        )],
        TuiElementKind::Unsupported { role, label } => vec![match label {
            Some(label) => format!("  <Unsupported: {role} \"{label}\">"),
            None => format!("  <Unsupported: {role}>"),
        }],
    }
}

fn edit_buffer_window(session: &EditSession, width: u16) -> String {
    let available = usize::from(width.saturating_sub(7)).max(1);
    let characters: Vec<char> = session.buffer.text().chars().collect();
    let cursor = session.buffer.cursor().min(characters.len());
    let start = cursor.saturating_sub(available / 2);
    let end = (start + available.saturating_sub(1)).min(characters.len());
    let mut rendered: String = characters[start..end].iter().collect();
    let local_cursor = cursor.saturating_sub(start).min(rendered.chars().count());
    let byte = rendered
        .char_indices()
        .nth(local_cursor)
        .map_or(rendered.len(), |(index, _)| index);
    rendered.insert(byte, '|');
    rendered
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, RuntimeNodeId};

    use super::*;

    fn element(kind: TuiElementKind) -> TuiElement {
        TuiElement {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/node/1"),
            semantic_role: crate::semantic::SemanticRole::Button,
            kind,
            actions: Vec::new(),
            capability: InteractionCapability::Activate,
        }
    }

    #[test]
    fn renders_terminal_native_widget_text_with_non_color_focus_marker() {
        assert_eq!(
            element_lines(
                &element(TuiElementKind::Button {
                    label: "Apply".to_owned()
                }),
                true
            ),
            vec!["> [ Apply ]"]
        );
        assert_eq!(
            element_lines(
                &element(TuiElementKind::CheckBox {
                    label: "Enabled".to_owned(),
                    checked: true,
                }),
                false
            ),
            vec!["  [x] Enabled"]
        );
    }

    #[test]
    fn distinguishes_selected_list_items_from_keyboard_focus() {
        assert_eq!(
            element_lines(
                &element(TuiElementKind::ListItem {
                    label: "Beta".to_owned(),
                    selected: true,
                }),
                false
            ),
            vec!["  * Beta"]
        );
        assert_eq!(
            element_lines(
                &element(TuiElementKind::ListItem {
                    label: "Beta".to_owned(),
                    selected: false,
                }),
                true
            ),
            vec!["> • Beta"]
        );
    }

    #[test]
    fn marks_non_actionable_controls_as_read_only_without_relying_on_color() {
        let mut checkbox = element(TuiElementKind::CheckBox {
            label: "Enabled".to_owned(),
            checked: false,
        });
        checkbox.capability = InteractionCapability::None;
        checkbox.semantic_role = crate::semantic::SemanticRole::CheckBox;

        assert_eq!(
            element_lines(&checkbox, true),
            vec!["> [ ] Enabled  (read-only)"]
        );
    }

    #[test]
    fn edit_buffer_renders_a_non_color_cursor_and_horizontal_window() {
        let mut input = element(TuiElementKind::TextInput {
            label: "Username".to_owned(),
            display: "alice".to_owned(),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        input.semantic_role = crate::semantic::SemanticRole::TextInput;
        input.capability = InteractionCapability::EditText;
        let session = EditSession::new(
            input.runtime_id,
            input.backend_locator.clone(),
            "abcdefghijklmnopqrstuvwxyz".to_owned(),
            1,
        );
        let lines = element_lines_for_width(&input, true, Some(&session), 18);
        assert_eq!(lines[0], "> Username  [editing]");
        assert!(lines[1].contains('|'));
        assert!(lines[1].len() < 24);
    }
}
