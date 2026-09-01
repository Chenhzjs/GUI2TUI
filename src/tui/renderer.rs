use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    content::{ContentBlockKind, ContentSearchResult, ReaderBlock, SearchProgress, SearchState},
    transcompile::ChoiceOption,
    transcompile::{SceneElement, SceneElementId, SceneElementKind, TuiScene},
    tui::palette::PaletteEntry,
};

use super::{
    action::InteractionCapability,
    content_view::ContentViewMode,
    edit::EditSession,
    hit_test::{HitInteraction, HitRegion},
};

pub struct RenderContext<'a> {
    pub scene: &'a TuiScene,
    pub focused: Option<SceneElementId>,
    pub scroll_offset: u16,
    pub status: &'a str,
    pub application_available: bool,
    pub edit_session: Option<&'a EditSession>,
    pub palette: Option<PaletteRender<'a>>,
    pub choice: Option<ChoiceRender<'a>>,
    pub content: Option<ContentRender>,
}

pub struct PaletteRender<'a> {
    pub query: &'a str,
    pub entries: &'a [PaletteEntry],
    pub selected: usize,
    pub all_scopes: bool,
}

pub struct ChoiceRender<'a> {
    pub label: &'a str,
    pub options: &'a [ChoiceOption],
    pub selected: usize,
    pub partial: bool,
}

pub struct ContentRender {
    pub title: String,
    pub mode: ContentViewMode,
    pub blocks: Vec<ReaderBlock>,
    pub outline: Vec<(String, Option<u8>)>,
    pub outline_selected: usize,
    pub query: String,
    pub results: Vec<ContentSearchResult>,
    pub result_selected: usize,
    pub partial: bool,
    pub full_search: Option<(SearchState, SearchProgress)>,
    pub structure_lines: Vec<String>,
}

pub fn render(frame: &mut Frame<'_>, context: RenderContext<'_>) -> Vec<HitRegion> {
    let hints = if context.edit_session.is_some() {
        "F1 Help | Enter Commit | Esc Cancel"
    } else if context.choice.is_some() {
        "? Help | ↑/↓ Choose | Enter Select | Esc Cancel"
    } else if context.palette.is_some() {
        "F1 Help | F2 Scope | Enter Choose | Esc Back"
    } else if let Some(content) = &context.content {
        match content.mode {
            ContentViewMode::Reader => "? Help | j/k Blocks | / Search | Esc Back",
            ContentViewMode::Search => "F1 Help | Ctrl-F Search more | Enter Go | Esc Back",
            ContentViewMode::Table => "? Help | Arrows Cells | Esc Back",
            ContentViewMode::Outline => "? Help | ↑/↓ Headings | Enter Go | Esc Back",
            ContentViewMode::VirtualCollection => "? Help | ↑/↓ Items | Esc Back",
        }
    } else {
        "? Help | Tab Focus | Enter Use"
    };
    let areas = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(frame.area());
    let main_area = areas[0];
    let footer_area = areas[1];
    let block = Block::default()
        .title(format!(" GUI2TUI — {} ", context.scene.title))
        .borders(Borders::ALL);
    let content_area = block.inner(main_area);
    frame.render_widget(block, main_area);
    let mut hit_regions = Vec::new();
    if context.application_available {
        render_elements(frame, content_area, &context, &mut hit_regions);
    } else {
        frame.render_widget(
            Paragraph::new("Application is no longer available. F5: retry; b: applications; d: diagnostics; q: quit."),
            content_area,
        );
    }
    if let Some(palette) = context.palette {
        render_palette(frame, content_area, palette);
    }
    if let Some(choice) = context.choice {
        render_choice(frame, content_area, choice);
    }
    if let Some(content) = context.content {
        render_content(frame, content_area, content);
    }
    let footer = format!("{hints} | {}", context.status);
    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::Cyan)),
        footer_area,
    );
    hit_regions
}

fn render_content(frame: &mut Frame<'_>, area: Rect, content: ContentRender) {
    frame.render_widget(Clear, area);
    let (title, lines, help) = match content.mode {
        ContentViewMode::Reader => {
            let lines = content
                .blocks
                .iter()
                .flat_map(|block| {
                    let prefix = match block.kind {
                        ContentBlockKind::Heading { .. } => "# ",
                        ContentBlockKind::Link => "[Link] ",
                        ContentBlockKind::ListItem => "• ",
                        ContentBlockKind::Quote => "> ",
                        ContentBlockKind::OpaqueContent(_) => "[Media] ",
                        _ => "",
                    };
                    vec![format!("{prefix}{}", block.text), String::new()]
                })
                .collect::<Vec<_>>();
            (
                format!(" Reader — {} ", content.title),
                lines,
                "j/k Move | PageUp/PageDown | o Outline | / Search | Esc Back",
            )
        }
        ContentViewMode::Outline => {
            let lines = if content.outline.is_empty() {
                vec!["No semantic headings exposed.".to_owned()]
            } else {
                content
                    .outline
                    .iter()
                    .enumerate()
                    .map(|(index, (label, level))| {
                        let marker = if index == content.outline_selected {
                            ">"
                        } else {
                            " "
                        };
                        let indent = "  ".repeat(level.unwrap_or(1).saturating_sub(1) as usize);
                        format!("{marker} {indent}{label}")
                    })
                    .collect()
            };
            (
                format!(" Outline — {} ", content.title),
                lines,
                "↑/↓ Navigate | Enter Read | / Search | Esc Reader",
            )
        }
        ContentViewMode::Search => {
            let mut lines = vec![format!("> {}", content.query)];
            if let Some((state, progress)) = &content.full_search {
                let scope = if content.partial {
                    "Exposed semantic search"
                } else {
                    "Full search"
                };
                let scanned = progress.total_blocks.map_or_else(
                    || format!("{} blocks scanned", progress.scanned_blocks),
                    |total| format!("{} / {} blocks scanned", progress.scanned_blocks, total),
                );
                lines.push(format!(
                    "{}: {:?} — {} — {} matches — {} text RPCs",
                    scope,
                    state,
                    scanned,
                    content.results.len(),
                    progress.text_rpcs
                ));
            }
            if content.results.is_empty() {
                lines.push("No matches in indexed labels or loaded text.".to_owned());
            } else {
                lines.extend(content.results.iter().enumerate().map(|(index, result)| {
                    format!(
                        "{} {}",
                        if index == content.result_selected {
                            ">"
                        } else {
                            " "
                        },
                        result.preview
                    )
                }));
            }
            (
                format!(" Content search — {} ", content.title),
                lines,
                if content.partial {
                    "Type Search | Ctrl-F Search exposed content | ↑/↓ Navigate | Enter Read | Esc Cancel/Reader"
                } else {
                    "Type Search | Ctrl-F Full Search | ↑/↓ Navigate | Enter Read | Esc Cancel/Reader"
                },
            )
        }
        ContentViewMode::VirtualCollection => (
            format!(" Realized collection — {} ", content.title),
            content.structure_lines,
            "↑/↓ Navigate realized items | Esc Reader",
        ),
        ContentViewMode::Table => (
            format!(" Table — {} ", content.title),
            content.structure_lines,
            "↑/↓/←/→ Navigate semantic cells | Esc Reader",
        ),
    };
    let completeness = if content.partial { " — partial" } else { "" };
    let block = Block::default()
        .title(format!("{title}{completeness}"))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .wrap(Wrap { trim: false })
            .block(Block::default().title(help)),
        inner,
    );
}

fn render_choice(frame: &mut Frame<'_>, area: Rect, choice: ChoiceRender<'_>) {
    let width = area.width.clamp(20, 56);
    let height = (choice.options.len() as u16 + 4).min(area.height).max(5);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let title = if choice.partial {
        format!(" Choose {} (partial) ", choice.label)
    } else {
        format!(" Choose {} ", choice.label)
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let lines = choice
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let cursor = if index == choice.selected { ">" } else { " " };
            let selected = if option.selected { "*" } else { " " };
            let disabled = if option.enabled { "" } else { " (disabled)" };
            format!("{cursor} {selected} {}{disabled}", option.label)
        })
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_elements(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    hit_regions: &mut Vec<HitRegion>,
) {
    let visible_bottom = context.scroll_offset.saturating_add(area.height);
    let mut logical_top = 0_u16;
    for element in &context.scene.elements {
        let focused = context.focused == Some(element.id);
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
                scene_id: element.id,
                rect: Rect::new(area.x, y, area.width, visible_height),
                interaction,
            });
        }
        logical_top = logical_top.saturating_add(element.height_for_width(area.width));
    }
}

fn interaction(element: &SceneElement) -> Option<HitInteraction> {
    match element.kind {
        SceneElementKind::Button { .. }
        | SceneElementKind::Toggle { .. }
        | SceneElementKind::Checkbox { .. }
        | SceneElementKind::SelectionItem { .. }
        | SceneElementKind::Command { .. } => {
            Some(if element.capability() == InteractionCapability::None {
                HitInteraction::Unavailable
            } else {
                HitInteraction::Activate
            })
        }
        SceneElementKind::Field { .. } | SceneElementKind::Selector { .. } => {
            Some(HitInteraction::Focus)
        }
        SceneElementKind::DocumentSummary { .. } => Some(HitInteraction::Activate),
        _ => None,
    }
}

pub fn element_lines(element: &SceneElement, focused: bool) -> Vec<String> {
    element_lines_for_width(element, focused, None, u16::MAX)
}

fn element_lines_for_width(
    element: &SceneElement,
    focused: bool,
    edit_session: Option<&EditSession>,
    width: u16,
) -> Vec<String> {
    let marker = if focused { "> " } else { "  " };
    let unavailable =
        if element.capability() == InteractionCapability::None && element.is_focusable() {
            "  (read-only)"
        } else {
            ""
        };
    match &element.kind {
        SceneElementKind::Text { text } => vec![format!("  {text}")],
        SceneElementKind::Status { text } => {
            let available = usize::from(width.clamp(20, 100).saturating_sub(14));
            let status = truncate(text, available);
            vec![format!("  ── Status: {status} ──")]
        }
        SceneElementKind::Hint { text } => vec![format!("    Hint: {text}")],
        SceneElementKind::Error { text } => vec![format!("    Error: {text}")],
        SceneElementKind::Group { label } | SceneElementKind::CommandHeader { label } => {
            section_lines(label, focused, width)
        }
        SceneElementKind::Button { label } => vec![format!("{marker}[ {label} ]{unavailable}")],
        SceneElementKind::Toggle { label, pressed } => vec![format!(
            "{marker}[{} {label}]{unavailable}",
            if *pressed { "*" } else { " " }
        )],
        SceneElementKind::Checkbox { label, checked } => vec![format!(
            "{marker}[{}] {label}{unavailable}",
            if *checked { "x" } else { " " }
        )],
        SceneElementKind::Field { label, display, .. } => {
            if let Some(session) = edit_session.filter(|edit| {
                element
                    .binding
                    .as_ref()
                    .is_some_and(|binding| edit.target == binding.runtime_id)
            }) {
                vec![
                    format!("{marker}{label}  [editing]"),
                    format!("    > {}", edit_buffer_window(session, width)),
                ]
            } else if width >= 100 {
                vec![format!("{marker}{label}: {display}{unavailable}")]
            } else {
                vec![
                    format!("{marker}{label}{unavailable}"),
                    format!("    > {display}"),
                ]
            }
        }
        SceneElementKind::Selector { label } => vec![format!("{marker}[ {label} ▼ ]{unavailable}")],
        SceneElementKind::DocumentSummary {
            title,
            blocks,
            headings,
            links,
            forms,
            completeness,
        } => boxed_lines(
            &format!("Document: {title}"),
            &[
                format!("{blocks} blocks | {headings} headings | {links} links | {forms} forms"),
                format!("completeness: {completeness}"),
                "[ Enter: Read document ]".to_owned(),
                "o Outline | / Content search".to_owned(),
            ],
            focused,
            width,
        ),
        SceneElementKind::SelectionItem { label, selected } => vec![format!(
            "{marker}{} {label}{unavailable}",
            if *selected { "*" } else { "•" }
        )],
        SceneElementKind::Command { path } => vec![format!("{marker}{path}{unavailable}")],
        SceneElementKind::OpaqueContent { label, dimensions } => {
            let detail = dimensions.map_or_else(
                || "[static visual; fidelity preferred]".to_owned(),
                |(w, h)| format!("[static visual; source {w}×{h} GUI pixels]"),
            );
            compact_boxed_lines(
                label,
                &[detail, "Enter: request visual handoff".to_owned()],
                focused,
                width,
                5,
            )
        }
        SceneElementKind::Unsupported { label } => vec![format!("  <Unsupported: {label}>")],
    }
}

fn section_lines(label: &str, focused: bool, width: u16) -> Vec<String> {
    let available = usize::from(width.clamp(20, 100).saturating_sub(6));
    let title = truncate(label, available);
    let inner = format!("─ {title} ");
    let horizontal = "─".repeat(inner.chars().count().max(available));
    let marker = if focused { "> " } else { "  " };
    vec![
        format!(
            "{marker}┌{inner}{:─<width$}┐",
            "",
            width = horizontal
                .chars()
                .count()
                .saturating_sub(inner.chars().count())
        ),
        "  │".to_owned(),
        format!("  └{}┘", horizontal),
    ]
}

fn boxed_lines(title: &str, body: &[String], focused: bool, width: u16) -> Vec<String> {
    // Reserve the two-column focus marker so the border remains inside the
    // terminal viewport even when the document is focused.
    let box_width = usize::from(width.clamp(24, 100).saturating_sub(2));
    let inner_width = box_width.saturating_sub(4).max(8);
    let title = truncate(title, inner_width.saturating_sub(2));
    let top_fill = inner_width.saturating_sub(title.chars().count() + 2);
    let marker = if focused { "> " } else { "  " };
    let mut lines = vec![format!(
        "{marker}┌─ {title} {:─<top_fill$}┐",
        "",
        top_fill = top_fill
    )];
    for line in body {
        let text = truncate(line, inner_width);
        let padding = inner_width.saturating_sub(text.chars().count());
        lines.push(format!("  │ {text}{:padding$} │", "", padding = padding));
    }
    while lines.len() < 6 {
        lines.push(format!(
            "  │{:inner_width$}│",
            "",
            inner_width = inner_width + 2
        ));
    }
    lines.push(format!("  └{}┘", "─".repeat(inner_width + 2)));
    lines
}

fn compact_boxed_lines(
    title: &str,
    body: &[String],
    focused: bool,
    width: u16,
    height: usize,
) -> Vec<String> {
    let box_width = usize::from(width.clamp(24, 100).saturating_sub(2));
    let inner_width = box_width.saturating_sub(4).max(8);
    let title = truncate(title, inner_width.saturating_sub(2));
    let top_fill = inner_width.saturating_sub(title.chars().count() + 2);
    let marker = if focused { "> " } else { "  " };
    let mut lines = vec![format!(
        "{marker}┌─ {title} {:─<top_fill$}┐",
        "",
        top_fill = top_fill
    )];
    for line in body {
        let text = truncate(line, inner_width);
        let padding = inner_width.saturating_sub(text.chars().count());
        lines.push(format!("  │ {text}{:padding$} │", "", padding = padding));
    }
    while lines.len() + 1 < height {
        lines.push(format!(
            "  │{:inner_width$}│",
            "",
            inner_width = inner_width + 2
        ));
    }
    lines.push(format!("  └{}┘", "─".repeat(inner_width + 2)));
    lines
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn render_palette(frame: &mut Frame<'_>, area: Rect, palette: PaletteRender<'_>) {
    let width = area.width.min(72);
    let height = (palette.entries.len() as u16 + 4).min(area.height).max(5);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let search_scope = if palette.all_scopes {
        "all application commands"
    } else {
        "current interaction scope"
    };
    let mut lines = vec![format!(
        "> {}  [search: {search_scope}; F2 toggle]",
        palette.query
    )];
    lines.extend(palette.entries.iter().enumerate().map(|(index, entry)| {
        format!(
            "{} {}",
            if index == palette.selected { ">" } else { " " },
            entry.label
        )
    }));
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(
                Block::default()
                    .title(" Command palette ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
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
    use super::*;
    use crate::{
        semantic::{BackendLocator, RuntimeNodeId, SemanticRole},
        transcompile::{PresentationStrategy, SceneBinding},
        tui::action::UiIntent,
    };

    fn element(kind: SceneElementKind) -> SceneElement {
        SceneElement {
            id: SceneElementId::new(9),
            kind,
            sources: vec![RuntimeNodeId::new(1)],
            binding: Some(SceneBinding {
                runtime_id: RuntimeNodeId::new(1),
                backend_locator: BackendLocator::new(":1.2", "/node/1"),
                semantic_role: SemanticRole::Button,
                actions: Vec::new(),
                capability: InteractionCapability::Activate,
                default_intent: UiIntent::Activate,
            }),
            strategy: PresentationStrategy::DirectWidget,
        }
    }

    #[test]
    fn renders_terminal_native_widgets_with_non_color_markers() {
        assert_eq!(
            element_lines(
                &element(SceneElementKind::Button {
                    label: "Apply".into()
                }),
                true
            ),
            vec!["> [ Apply ]"]
        );
        assert_eq!(
            element_lines(
                &element(SceneElementKind::Checkbox {
                    label: "Enabled".into(),
                    checked: true
                }),
                false
            ),
            vec!["  [x] Enabled"]
        );
    }

    #[test]
    fn responsive_field_switches_from_stacked_to_inline() {
        let field = element(SceneElementKind::Field {
            label: "Username".into(),
            display: "alice".into(),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        assert_eq!(element_lines_for_width(&field, false, None, 80).len(), 2);
        assert_eq!(element_lines_for_width(&field, false, None, 120).len(), 1);
    }

    #[test]
    fn selection_and_focus_have_independent_non_color_markers() {
        let item = element(SceneElementKind::SelectionItem {
            label: "Beta".into(),
            selected: true,
        });
        assert_eq!(element_lines(&item, false), vec!["  * Beta"]);
        assert_eq!(element_lines(&item, true), vec!["> * Beta"]);
    }

    #[test]
    fn unavailable_control_is_visibly_read_only() {
        let mut checkbox = element(SceneElementKind::Checkbox {
            label: "Enable feature".into(),
            checked: false,
        });
        checkbox.binding.as_mut().unwrap().capability = InteractionCapability::None;
        assert_eq!(
            element_lines(&checkbox, false),
            vec!["  [ ] Enable feature  (read-only)"]
        );
    }

    #[test]
    fn editing_renders_a_local_cursor_inside_a_narrow_window() {
        let mut field = element(SceneElementKind::Field {
            label: "Username".into(),
            display: "confirmed".into(),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        field.binding.as_mut().unwrap().capability = InteractionCapability::EditText;
        let session = EditSession::new(
            RuntimeNodeId::new(1),
            BackendLocator::new(":1.2", "/node/1"),
            "0123456789abcdef".into(),
            1,
        );
        let lines = element_lines_for_width(&field, true, Some(&session), 14);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains('|'));
        assert!(lines[1].ends_with("def|"));
    }

    #[test]
    fn opaque_content_is_explicitly_preserved() {
        let mut opaque = element(SceneElementKind::OpaqueContent {
            label: "Canvas".into(),
            dimensions: Some((640, 480)),
        });
        opaque.binding = None;
        assert!(element_lines(&opaque, false)[1].contains("static visual"));
    }

    #[test]
    fn document_summary_is_a_bounded_enterable_region() {
        let document = element(SceneElementKind::DocumentSummary {
            title: "A long document title that should be clipped".into(),
            blocks: 12,
            headings: 3,
            links: 4,
            forms: 1,
            completeness: "partial".into(),
        });
        let lines = element_lines_for_width(&document, true, None, 32);
        assert_eq!(lines.len(), 7);
        assert!(lines.iter().all(|line| line.chars().count() <= 32));
        assert!(lines[0].starts_with("> ┌─ Document:"));
        assert!(
            lines
                .last()
                .is_some_and(|line| line.trim_end().ends_with('┘'))
        );
    }

    #[test]
    fn group_headers_use_three_line_sections_and_fit_narrow_widths() {
        let group = element(SceneElementKind::Group {
            label: "Toolbar controls".into(),
        });
        let lines = element_lines_for_width(&group, false, None, 24);
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.chars().count() <= 24));
        assert!(lines[0].contains("Toolbar controls"));
    }
}
