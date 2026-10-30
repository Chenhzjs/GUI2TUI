use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    content::{ContentSearchResult, ReaderBlock, SearchProgress, SearchState},
    semantic::SemanticCapability,
    transcompile::ChoiceOption,
    transcompile::{
        LayoutImportance, LayoutNode, RegionPresentationKind, SceneElement, SceneElementId,
        SceneElementKind, SpatialRegion, SpatialRegionId, TuiLayoutPlan, TuiScene,
        realize_responsive_layout,
    },
    tui::palette::PaletteEntry,
};

use super::{
    action::InteractionCapability,
    content_view::ContentViewMode,
    edit::EditSession,
    hit_test::{HitInteraction, HitRegion},
    region_navigation::{RegionNavigationItem, RegionNavigator},
};

pub struct RenderContext<'a> {
    pub scene: &'a TuiScene,
    pub interaction_scope: Option<&'a crate::transcompile::InteractionScope>,
    pub focused: Option<SceneElementId>,
    pub scroll_offset: u16,
    pub status: &'a str,
    pub application_available: bool,
    pub external_text_handler_available: bool,
    pub edit_session: Option<&'a EditSession>,
    pub palette: Option<PaletteRender<'a>>,
    pub choice: Option<ChoiceRender<'a>>,
    pub content: Option<ContentRender>,
    pub spatial: Option<&'a TuiLayoutPlan>,
    pub active_region: Option<SpatialRegionId>,
    pub inline_content: Option<InlineContentRender>,
}

#[derive(Clone, Debug)]
pub struct InlineContentRender {
    pub title: String,
    pub lines: Vec<String>,
    pub total_lines: usize,
    pub partial: bool,
}

pub struct PaletteRender<'a> {
    pub query: &'a str,
    pub entries: &'a [PaletteEntry],
    pub selected: usize,
    pub all_scopes: bool,
    pub scope_switchable: bool,
    pub detail_scroll: usize,
}

// Terminal-native colors preserve the user's background. Focus also has a
// marker and weight so no operation/state depends on color alone.
fn accent() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn secondary() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn selected_style() -> Style {
    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
}

fn scope_caption(scope: Option<&crate::transcompile::InteractionScope>, fallback: &str) -> String {
    use crate::transcompile::InteractionScopeKind;
    let Some(scope) = scope else {
        return fallback.into();
    };
    let kind = if scope.modal {
        "Modal"
    } else {
        match scope.kind {
            InteractionScopeKind::Dialog => "Dialog",
            InteractionScopeKind::Popup | InteractionScopeKind::MenuPopup => "Popup",
            _ => return fallback.into(),
        }
    };
    format!("[{kind}] {}", scope.label.as_deref().unwrap_or("Unnamed"))
}

fn clipped(text: &str, width: u16) -> String {
    let text = text.replace(['\n', '\r', '\t'], " ");
    let line = Line::from(text.as_str());
    if line.width() <= usize::from(width) {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for grapheme in line.styled_graphemes(Style::default()) {
        let cells = Span::raw(grapheme.symbol).width();
        if used + cells > usize::from(width - 1) {
            break;
        }
        result.push_str(grapheme.symbol);
        used += cells;
    }
    result.push('…');
    result
}

pub(crate) fn main_content_area(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(2),
    )
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
    pub reader_scroll: usize,
    pub outline: Vec<(String, Option<u8>)>,
    pub outline_selected: usize,
    pub query: String,
    pub results: Vec<ContentSearchResult>,
    pub result_selected: usize,
    pub partial: bool,
    pub full_search: Option<(SearchState, SearchProgress)>,
    pub structure_lines: Vec<String>,
    pub table_row_selection: bool,
    pub table_cell_activation: bool,
}

pub fn render(frame: &mut Frame<'_>, mut context: RenderContext<'_>) -> Vec<HitRegion> {
    let panel_has_key_guide = context.palette.is_some() || context.edit_session.is_some();
    let navigator = context
        .spatial
        .map(|plan| RegionNavigator::derive(plan, context.scene));
    let focused_element = context.focused.and_then(|id| context.scene.element(id));
    let focused_external_edit = focused_element
        .is_some_and(|element| element_supports_external_edit(context.scene, element));
    let focused_document_summary = focused_element
        .is_some_and(|element| matches!(element.kind, SceneElementKind::DocumentSummary { .. }));
    let hints = if context.edit_session.is_some() {
        "Esc Cancel · Ctrl+S Apply · F1 Help"
    } else if context.choice.is_some() {
        "Esc Cancel · ↑↓ Select · Enter Use"
    } else if context.palette.is_some() {
        palette_help(frame.area().width)
    } else if let Some(content) = &context.content {
        match content.mode {
            ContentViewMode::Reader => "? Help · j/k Scroll · / Search · o Outline · Esc Back",
            ContentViewMode::Search => "? Help · Ctrl-F Search more · Enter Go · Esc Back",
            ContentViewMode::Table
                if content.table_row_selection && content.table_cell_activation =>
            {
                "? Help · Arrows Cells · Enter Select row · a Activate cell · Esc Back"
            }
            ContentViewMode::Table if content.table_row_selection => {
                "? Help · Arrows Cells · Enter Select row · Esc Back"
            }
            ContentViewMode::Table if content.table_cell_activation => {
                "? Help · Arrows Cells · a Activate cell · Esc Back"
            }
            ContentViewMode::Table => "? Help · Arrows Cells · Esc Back",
            ContentViewMode::Outline => "? Help · ↑/↓ Headings · Enter Go · Esc Back",
            ContentViewMode::VirtualCollection => "? Help · ↑/↓ Items · Esc Back",
        }
    } else if context
        .focused
        .and_then(|id| context.scene.element(id))
        .is_some_and(|element| element.capability() == InteractionCapability::AdjustValue)
    {
        "? Help · ↑/↓ Adjust · Tab Control · F6 Region"
    } else if focused_external_edit {
        if context.external_text_handler_available {
            if focused_document_summary {
                "? Help · Enter Read · F3 Actions · e Edit externally · Tab Control"
            } else {
                "? Help · Enter Edit · F3 Actions · e Edit externally · Tab Control"
            }
        } else if focused_document_summary {
            "? Help · Enter Read · F3 Actions · external editor not configured · Tab Control"
        } else {
            "? Help · F3 Actions · external editor not configured · Tab Control"
        }
    } else if navigator
        .as_ref()
        .is_some_and(RegionNavigator::has_subregions)
    {
        "Tab Move · Enter Use · F3 Find action · F6 Region · Ctrl+Tab Pane · F1 Help"
    } else {
        "Tab Move · Enter Use · F3 Find action · F6 Region · F1 Help"
    };
    let hints = if frame.area().width < 60
        && context.edit_session.is_none()
        && context.choice.is_none()
        && context.palette.is_none()
        && context.content.is_none()
    {
        "Tab · Enter Use · F3 Find · F1 Help"
    } else {
        hints
    };
    let area = frame.area();
    let content_area = main_content_area(area);
    let footer_area = Rect::new(
        area.x,
        area.bottom().saturating_sub(1),
        area.width,
        u16::from(area.height > 0),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("GUI2TUI", accent()),
            Span::styled(
                clipped(
                    &format!(
                        " · {}",
                        scope_caption(context.interaction_scope, &context.scene.title)
                    ),
                    area.width.saturating_sub(7),
                ),
                if context.interaction_scope.is_some_and(|scope| scope.modal) {
                    accent()
                } else {
                    secondary()
                },
            ),
        ])),
        Rect::new(area.x, area.y, area.width, u16::from(area.height > 0)),
    );
    let mut hit_regions = Vec::new();
    // The local editor owns one input surface. Background controls continue
    // to show the authoritative GUI value, not a second copy of the buffer.
    let edit_session = context.edit_session.take();
    if context.application_available {
        if let Some(plan) = context.spatial {
            render_spatial(
                frame,
                content_area,
                &context,
                plan,
                navigator.as_ref(),
                &mut hit_regions,
            );
        } else {
            render_elements(frame, content_area, &context, &mut hit_regions);
        }
    } else {
        frame.render_widget(
            Paragraph::new("Application is no longer available. F5: retry; b: applications; d: diagnostics; q: quit."),
            content_area,
        );
    }
    context.edit_session = edit_session;
    if let Some(palette) = context.palette {
        // The background remains visible, but is not an interactive surface.
        hit_regions.clear();
        frame.buffer_mut().set_style(content_area, secondary());
        render_palette(frame, content_area, palette);
    }
    if let Some(choice) = context.choice {
        render_choice(frame, content_area, choice);
    }
    if let Some(content) = context.content {
        render_content(frame, content_area, content);
    }
    // A public text target need not have a visible scene element. The input
    // surface belongs to the edit session, not to the region projection.
    if let Some(session) = context.edit_session {
        hit_regions.clear();
        let area = Rect::new(
            content_area.x,
            content_area.y,
            content_area.width,
            content_area.height.min(5),
        );
        frame.render_widget(Clear, area);
        let edit_label = context
            .scene
            .elements
            .iter()
            .find(|element| {
                element.binding.as_ref().is_some_and(|binding| {
                    binding.runtime_id == session.target
                        && binding.backend_locator == session.backend_locator
                })
            })
            .map(|element| element.label())
            .unwrap_or("text");
        let title = if session.range_parameter {
            " Edit range: start:end:text (Unicode characters, end exclusive) ".to_owned()
        } else if session.value_parameter {
            " Set value ".to_owned()
        } else {
            format!(" Edit {edit_label} ")
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(accent())
            .title(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(format!(
                "[editing] {}\n{}",
                edit_buffer_window(session, inner.width.saturating_sub(10)),
                if session.value_parameter || session.range_parameter {
                    "Esc Cancel · Enter Apply"
                } else {
                    "Esc Cancel · Ctrl+S Apply · F1 Help"
                }
            ))
            .style(accent()),
            inner,
        );
    }
    let footer = if panel_has_key_guide {
        // These floating panels already contain their own key guide. Reserve
        // the outer footer for meaningful outcomes or warnings.
        if normal_footer_status(context.status) && !context.status.starts_with("Editing \"") {
            context.status.to_owned()
        } else {
            String::new()
        }
    } else if normal_footer_status(context.status) {
        if area.width < 60 || context.edit_session.is_some() {
            format!("{hints} · {}", context.status)
        } else {
            format!("{} · {hints}", context.status)
        }
    } else {
        hints.to_owned()
    };
    frame.render_widget(Paragraph::new(footer).style(secondary()), footer_area);
    hit_regions
}

fn element_supports_external_edit(scene: &TuiScene, element: &SceneElement) -> bool {
    matches!(
        element.kind,
        SceneElementKind::DocumentSummary {
            external_edit: true,
            ..
        }
    ) || element
        .binding
        .as_ref()
        .and_then(|binding| scene.node_metadata(binding.runtime_id))
        .is_some_and(|metadata| {
            metadata
                .capabilities
                .contains(&SemanticCapability::EditComplexText)
        })
}

fn render_content(frame: &mut Frame<'_>, area: Rect, content: ContentRender) {
    frame.render_widget(Clear, area);
    let (title, lines, help) = match content.mode {
        ContentViewMode::Reader => (
            format!(" Reader — {} ", content.title),
            super::content_view::reader_lines(&content.blocks, area.width.saturating_sub(2).max(1)),
            "j/k Scroll text | PageUp/PageDown | o Outline | / Search | Esc Back",
        ),
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
            if content.table_row_selection && content.table_cell_activation {
                "↑/↓/←/→ Navigate semantic cells | Enter Select row | a Activate cell | Esc Back"
            } else if content.table_row_selection {
                "↑/↓/←/→ Navigate semantic cells | Enter Select current row | Esc Back"
            } else if content.table_cell_activation {
                "↑/↓/←/→ Navigate semantic cells | a Activate cell | Esc Back"
            } else {
                "↑/↓/←/→ Navigate semantic cells | Esc Back"
            },
        ),
    };
    let completeness = if content.partial { " — partial" } else { "" };
    let block = Block::default()
        .title(format!("{title}{completeness}"))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let selected = match content.mode {
        ContentViewMode::Outline => Some(content.outline_selected),
        ContentViewMode::Search if !content.results.is_empty() => {
            Some(1 + usize::from(content.full_search.is_some()) + content.result_selected)
        }
        _ => None,
    };
    let selected_row = selected.map(|index| {
        lines
            .iter()
            .take(index)
            .map(|line| super::palette::detail_lines(line, inner.width).len())
            .sum::<usize>()
    });
    let lines = if content.mode == ContentViewMode::Reader {
        lines
    } else {
        lines
            .iter()
            .flat_map(|line| super::palette::detail_lines(line, inner.width))
            .collect()
    };
    let height = usize::from(Block::default().title(help).inner(inner).height);
    let offset = if content.mode == ContentViewMode::Reader {
        content.reader_scroll
    } else {
        selected_row.unwrap_or(0).saturating_sub(height / 2)
    };
    let offset = offset.min(lines.len().saturating_sub(1));
    frame.render_widget(
        Paragraph::new(
            lines
                .into_iter()
                .skip(offset)
                .take(height)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .block(Block::default().title(help)),
        inner,
    );
}

fn render_spatial(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    plan: &TuiLayoutPlan,
    navigator: Option<&RegionNavigator>,
    hit_regions: &mut Vec<HitRegion>,
) {
    // Runtime accessibility events can change composition between frames.
    // Clear the old terminal realization so borders from a larger prior pane
    // cannot survive after a surface collapses or moves.
    frame.render_widget(Clear, area);
    let main = if let Some(navigator) = navigator.filter(|navigator| navigator.region_count() > 1)
        && area.height >= 7
    {
        let height = if navigator.has_subregions() { 2 } else { 1 };
        let areas = Layout::vertical([Constraint::Length(height), Constraint::Min(3)]).split(area);
        render_region_navigator(frame, areas[0], navigator, context.active_region);
        areas[1]
    } else {
        area
    };
    let responsive =
        realize_responsive_layout(plan, main.width, main.height, context.active_region);
    let root = simplify_empty_direct_surfaces(plan, &responsive.root, context.active_region)
        .unwrap_or_else(|| responsive.root.clone());
    let main = bounded_non_expanding_composition_area(plan, &root, main);
    render_spatial_node(frame, main, context, plan, &root, hit_regions);
}

fn simplify_empty_direct_surfaces(
    plan: &TuiLayoutPlan,
    node: &LayoutNode,
    active: Option<SpatialRegionId>,
) -> Option<LayoutNode> {
    match node {
        LayoutNode::Leaf(id) => plan
            .regions
            .iter()
            .find(|region| region.id == *id)
            .filter(|region| {
                Some(region.id) == active
                    || region.demand != crate::transcompile::LayoutDemand::Minimal
                    || region.visibility == crate::transcompile::VisibilityGuarantee::Pinned
            })
            .map(|_| node.clone()),
        LayoutNode::Stack(children) => {
            simplified_children(plan, children, active).map(LayoutNode::Stack)
        }
        LayoutNode::HorizontalSplit { children, weights } => {
            let children = simplified_children(plan, children, active)?;
            Some(LayoutNode::HorizontalSplit {
                weights: simplified_weights(plan, &children, weights),
                children,
            })
        }
        LayoutNode::VerticalSplit { children, weights } => {
            let children = simplified_children(plan, children, active)?;
            Some(LayoutNode::VerticalSplit {
                weights: simplified_weights(plan, &children, weights),
                children,
            })
        }
        LayoutNode::Overlay { base, overlays } => {
            let base = simplify_empty_direct_surfaces(plan, base, active)?;
            let overlays = overlays
                .iter()
                .filter_map(|overlay| simplify_empty_direct_surfaces(plan, overlay, active))
                .collect();
            Some(LayoutNode::Overlay {
                base: Box::new(base),
                overlays,
            })
        }
    }
}

fn simplified_children(
    plan: &TuiLayoutPlan,
    children: &[LayoutNode],
    active: Option<SpatialRegionId>,
) -> Option<Vec<LayoutNode>> {
    let children = children
        .iter()
        .filter_map(|child| simplify_empty_direct_surfaces(plan, child, active))
        .collect::<Vec<_>>();
    (!children.is_empty()).then_some(children)
}

fn simplified_weights(plan: &TuiLayoutPlan, children: &[LayoutNode], original: &[u16]) -> Vec<u16> {
    if children.len() == original.len() {
        original.to_vec()
    } else {
        children
            .iter()
            .map(
                |child| match region_for_node(plan, child).map(|region| region.demand) {
                    Some(crate::transcompile::LayoutDemand::Expand) => 5,
                    Some(crate::transcompile::LayoutDemand::Supporting) => 3,
                    Some(crate::transcompile::LayoutDemand::Compact) => 2,
                    _ => 1,
                },
            )
            .collect()
    }
}

fn render_region_navigator(
    frame: &mut Frame<'_>,
    area: Rect,
    navigator: &RegionNavigator,
    active: Option<SpatialRegionId>,
) {
    let active_group = navigator.active_group_index(active).unwrap_or(0);
    let groups = navigator.groups();
    let hierarchical = navigator.has_subregions();
    let inner = area;
    let major = groups
        .iter()
        .map(|group| RegionNavigationItem {
            id: group
                .regions
                .first()
                .map_or(SpatialRegionId(u64::MAX), |item| item.id),
            title: group.title.clone(),
            empty: group.regions.iter().all(|region| region.empty),
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(navigation_line(
            &format!("F6 Regions ({})", groups.len()),
            &major,
            active_group,
            inner.width,
        )),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    if hierarchical
        && inner.height > 1
        && let Some(group) = groups.get(active_group)
    {
        let selected = active
            .and_then(|active| group.regions.iter().position(|region| region.id == active))
            .unwrap_or(0);
        frame.render_widget(
            Paragraph::new(navigation_line(
                "Ctrl+Tab Pane",
                &group.regions,
                selected,
                inner.width,
            )),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
}

fn navigation_line(
    label: &str,
    items: &[RegionNavigationItem],
    selected: usize,
    width: u16,
) -> Line<'static> {
    let prefix = format!("{label}: ");
    let mut spans = vec![Span::raw(prefix.clone())];
    let mut used = Line::from(prefix.as_str()).width();
    for offset in 0..items.len() {
        let index = (selected + offset) % items.len();
        let item = &items[index];
        let suffix = if item.empty { " · empty" } else { "" };
        let text = if index == selected {
            let available = usize::from(width).saturating_sub(used).saturating_sub(2);
            format!(
                "[{}]",
                clipped(
                    &format!("{}{suffix}", item.title),
                    available.min(usize::from(u16::MAX)) as u16
                )
            )
        } else {
            format!("{}{}", item.title, suffix)
        };
        let separator = usize::from(offset > 0) * 3;
        let text_width = Line::from(text.as_str()).width();
        if used.saturating_add(separator).saturating_add(text_width) > usize::from(width) {
            if used.saturating_add(2) <= usize::from(width) {
                spans.push(Span::raw(" …"));
            }
            break;
        }
        if offset > 0 {
            spans.push(Span::raw(" · "));
            used = used.saturating_add(3);
        }
        let style = if index == selected {
            selected_style()
        } else {
            secondary()
        };
        spans.push(Span::styled(text, style));
        used = used.saturating_add(text_width);
    }
    Line::from(spans)
}

pub(crate) fn normal_footer_status(status: &str) -> bool {
    !status.is_empty()
        && !status.starts_with("Loaded ")
        && !status.starts_with("Live update")
        && !status.starts_with("Replayed bootstrap events")
        && !status.starts_with("Full refresh fallback:")
        && !status.starts_with("Reader loaded ")
        && !status.starts_with("Command palette — ")
        && !status.starts_with("Choice overlay — ")
        && !status.starts_with("Region: ")
        && status != "Public actions — choose the exact named operation"
}

fn bounded_non_expanding_composition_area(
    plan: &TuiLayoutPlan,
    root: &LayoutNode,
    area: Rect,
) -> Rect {
    let mut leaves = Vec::new();
    collect_visible_regions(plan, root, &mut leaves);
    if leaves.is_empty()
        || leaves.iter().any(|region| {
            matches!(
                region.demand,
                crate::transcompile::LayoutDemand::Expand
                    | crate::transcompile::LayoutDemand::Hidden
            )
        })
    {
        return area;
    }
    // Semantic importance does not imply a full-height terminal payload. If
    // every selected surface asks for bounded demand, realize the complete
    // composition in a bounded terminal-native viewport. Rich or focused
    // surfaces can still request Expand; GUI coordinates are never scaled.
    let Some(height) = terminal_height_demand(plan, root) else {
        return area;
    };
    let height = height.min(area.height);
    if height == 0 || area.height <= height {
        return area;
    }
    // Main content starts at a stable reading edge; floating overlays own
    // centering. Do not spend the first terminal rows on empty padding.
    Rect::new(area.x, area.y, area.width, height)
}

fn terminal_height_demand(plan: &TuiLayoutPlan, node: &LayoutNode) -> Option<u16> {
    match node {
        LayoutNode::Leaf(id) => {
            let region = plan.regions.iter().find(|region| region.id == *id)?;
            match region.demand {
                crate::transcompile::LayoutDemand::Expand => None,
                crate::transcompile::LayoutDemand::Supporting => {
                    if region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder {
                        Some(14)
                    } else {
                        Some(
                            u16::try_from(region.presentation.meaningful_items.min(10))
                                .unwrap_or(10)
                                .saturating_add(2)
                                .max(6),
                        )
                    }
                }
                crate::transcompile::LayoutDemand::Compact => {
                    Some(if context_summary(region) { 1 } else { 3 })
                }
                crate::transcompile::LayoutDemand::Minimal => {
                    Some(if context_summary(region) { 1 } else { 3 })
                }
                crate::transcompile::LayoutDemand::Hidden => Some(0),
            }
        }
        LayoutNode::Stack(children) | LayoutNode::VerticalSplit { children, .. } => children
            .iter()
            .filter(|child| layout_node_visible(plan, child))
            .try_fold(0_u16, |height, child| {
                terminal_height_demand(plan, child).map(|child| height.saturating_add(child))
            }),
        LayoutNode::HorizontalSplit { children, .. } => children
            .iter()
            .filter(|child| layout_node_visible(plan, child))
            .try_fold(0_u16, |height, child| {
                terminal_height_demand(plan, child).map(|child| height.max(child))
            }),
        LayoutNode::Overlay { base, overlays } => std::iter::once(base.as_ref())
            .chain(overlays.iter())
            .filter(|child| layout_node_visible(plan, child))
            .try_fold(0_u16, |height, child| {
                terminal_height_demand(plan, child).map(|child| height.max(child))
            }),
    }
}

fn collect_visible_regions<'a>(
    plan: &'a TuiLayoutPlan,
    node: &LayoutNode,
    output: &mut Vec<&'a SpatialRegion>,
) {
    match node {
        LayoutNode::Leaf(id) => {
            if let Some(region) = plan.regions.iter().find(|region| region.id == *id)
                && layout_node_visible(plan, node)
            {
                output.push(region);
            }
        }
        LayoutNode::Stack(children)
        | LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => {
            for child in children {
                collect_visible_regions(plan, child, output);
            }
        }
        LayoutNode::Overlay { base, overlays } => {
            collect_visible_regions(plan, base, output);
            for overlay in overlays {
                collect_visible_regions(plan, overlay, output);
            }
        }
    }
}

fn render_spatial_node(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    plan: &TuiLayoutPlan,
    node: &LayoutNode,
    hit_regions: &mut Vec<HitRegion>,
) {
    if area.width < 4 || area.height == 0 {
        return;
    }
    match node {
        LayoutNode::Leaf(id) => {
            let Some(region) = plan.regions.iter().find(|region| region.id == *id) else {
                return;
            };
            if region.importance == LayoutImportance::Structural
                || region.priority == crate::transcompile::PresentationPriority::HiddenByDefault
            {
                return;
            }
            render_spatial_leaf(frame, area, context, region, hit_regions);
        }
        LayoutNode::Stack(children) => {
            let children: Vec<_> = children
                .iter()
                .filter(|child| layout_node_visible(plan, child))
                .collect();
            if children.is_empty() {
                return;
            }
            // A semantic analysis can legitimately produce many supporting
            // regions.  Giving every one a fixed terminal height would make
            // ratatui resolve the whole stack to zero-height rectangles. Keep
            // the dominant region large and compress the rest into one
            // bounded supporting bar; structural leaves are still omitted.
            if children.len() > 4 {
                if let Some((dominant, rest)) = dominant_stack_child(plan, &children) {
                    let areas =
                        Layout::vertical([Constraint::Min(8), Constraint::Length(5)]).split(area);
                    render_spatial_node(frame, areas[0], context, plan, dominant, hit_regions);
                    render_support_bar(frame, areas[1], context, plan, &rest, hit_regions);
                    return;
                }
            }
            let constraints = children
                .iter()
                .map(|child| stack_constraint(plan, child, context.scene, area.width))
                .collect::<Vec<_>>();
            for (child, child_area) in Layout::vertical(constraints).split(area).iter().enumerate()
            {
                render_spatial_node(
                    frame,
                    *child_area,
                    context,
                    plan,
                    children[child],
                    hit_regions,
                );
            }
        }
        LayoutNode::HorizontalSplit { children, weights } => {
            let children: Vec<_> = children
                .iter()
                .filter(|child| layout_node_visible(plan, child))
                .collect();
            if children.is_empty() {
                return;
            }
            let constraints = weighted_constraints(plan, &children, weights, |plan, child| {
                split_constraint(plan, child, area.width)
            });
            for (index, child_area) in Layout::horizontal(constraints)
                .split(area)
                .iter()
                .enumerate()
            {
                render_spatial_node(
                    frame,
                    *child_area,
                    context,
                    plan,
                    children[index],
                    hit_regions,
                );
            }
        }
        LayoutNode::VerticalSplit { children, weights } => {
            let children: Vec<_> = children
                .iter()
                .filter(|child| layout_node_visible(plan, child))
                .collect();
            if children.is_empty() {
                return;
            }
            let constraints = weighted_constraints(plan, &children, weights, |plan, child| {
                stack_constraint(plan, child, context.scene, area.width)
            });
            for (index, child_area) in Layout::vertical(constraints).split(area).iter().enumerate()
            {
                render_spatial_node(
                    frame,
                    *child_area,
                    context,
                    plan,
                    children[index],
                    hit_regions,
                );
            }
        }
        LayoutNode::Overlay { base, overlays } => {
            render_spatial_node(frame, area, context, plan, base, hit_regions);
            for overlay in overlays {
                render_spatial_node(frame, area, context, plan, overlay, hit_regions);
            }
        }
    }
}

fn weighted_constraints(
    plan: &TuiLayoutPlan,
    children: &[&LayoutNode],
    weights: &[u16],
    fallback: impl Fn(&TuiLayoutPlan, &LayoutNode) -> Constraint,
) -> Vec<Constraint> {
    let has_compact = children.iter().any(|child| {
        region_for_node(plan, child).is_some_and(|region| {
            matches!(
                region.demand,
                crate::transcompile::LayoutDemand::Compact
                    | crate::transcompile::LayoutDemand::Minimal
            )
        })
    });
    if !has_compact && weights.len() == children.len() && weights.iter().any(|weight| *weight > 0) {
        let total = weights.iter().copied().map(u32::from).sum::<u32>();
        return weights
            .iter()
            .map(|weight| Constraint::Ratio(u32::from(*weight), total))
            .collect();
    }
    children.iter().map(|child| fallback(plan, child)).collect()
}

fn dominant_stack_child<'a>(
    plan: &TuiLayoutPlan,
    children: &[&'a LayoutNode],
) -> Option<(&'a LayoutNode, Vec<&'a LayoutNode>)> {
    let index = children.iter().position(|child| {
        region_for_node(plan, child)
            .is_some_and(|region| region.importance == LayoutImportance::Dominant)
    })?;
    let dominant = children[index];
    let rest = children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| (i != index).then_some(*child))
        .collect();
    Some((dominant, rest))
}

fn render_support_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    plan: &TuiLayoutPlan,
    children: &[&LayoutNode],
    hit_regions: &mut Vec<HitRegion>,
) {
    if area.height < 2 {
        return;
    }
    let block = Block::default()
        .title(" Application controls ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut labels = Vec::new();
    for child in children {
        let Some(region) = region_for_node(plan, child) else {
            continue;
        };
        if region.importance == LayoutImportance::Structural {
            continue;
        }
        let sources = &region.presentation.source_nodes;
        let sample = context
            .scene
            .elements
            .iter()
            .filter(|element| {
                element
                    .sources
                    .iter()
                    .any(|source| sources.contains(source))
                    && !matches!(
                        element.kind,
                        SceneElementKind::Unsupported { .. }
                            | SceneElementKind::CommandHeader { .. }
                    )
            })
            .take(2)
            .map(SceneElement::label)
            .collect::<Vec<_>>();
        if sample.is_empty() {
            labels.push(spatial_title(region));
        } else {
            labels.push(format!("{}: {}", spatial_title(region), sample.join(", ")));
        }
        if labels.len() >= 8 {
            break;
        }
    }
    let line = if labels.is_empty() {
        "(no secondary semantic regions)".to_owned()
    } else {
        labels.join("  ·  ")
    };
    frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: true }), inner);
    // Supporting controls remain reachable through the normal focus model;
    // the compact bar is a visual summary rather than a second action model.
    let _ = (context, hit_regions);
}

fn layout_node_visible(plan: &TuiLayoutPlan, node: &LayoutNode) -> bool {
    match node {
        LayoutNode::Leaf(id) => plan
            .regions
            .iter()
            .find(|region| region.id == *id)
            .is_some_and(|region| {
                region.importance != LayoutImportance::Structural
                    && !matches!(
                        region.presentation.kind,
                        RegionPresentationKind::Structural
                            | RegionPresentationKind::DiagnosticOnly
                            | RegionPresentationKind::Empty
                    )
            }),
        LayoutNode::Stack(children)
        | LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => children
            .iter()
            .any(|child| layout_node_visible(plan, child)),
        LayoutNode::Overlay { base, overlays } => {
            layout_node_visible(plan, base)
                || overlays
                    .iter()
                    .any(|child| layout_node_visible(plan, child))
        }
    }
}

fn region_for_node<'a>(plan: &'a TuiLayoutPlan, node: &LayoutNode) -> Option<&'a SpatialRegion> {
    let LayoutNode::Leaf(id) = node else {
        return None;
    };
    plan.regions.iter().find(|region| region.id == *id)
}

fn stack_constraint(
    plan: &TuiLayoutPlan,
    node: &LayoutNode,
    scene: &TuiScene,
    width: u16,
) -> Constraint {
    let region = region_for_node(plan, node);
    if region.is_some_and(context_summary) {
        return Constraint::Length(1);
    }
    if let Some(region) = region
        && region.demand == crate::transcompile::LayoutDemand::Supporting
        && region.presentation.kind == RegionPresentationKind::Form
    {
        let rows = scene
            .elements
            .iter()
            .filter(|element| {
                element
                    .sources
                    .iter()
                    .any(|source| region.presentation.source_nodes.contains(source))
                    && !matches!(
                        element.kind,
                        SceneElementKind::Unsupported { .. }
                            | SceneElementKind::CommandHeader { .. }
                    )
            })
            .fold(0_u16, |height, element| {
                height.saturating_add(element.height_for_width(width.saturating_sub(2)))
            });
        // Border plus one extra row for the currently edited field. This
        // supporting pane need not grow merely because the terminal is tall.
        return Constraint::Length(rows.saturating_add(3).max(5));
    }
    if region.is_some_and(|region| {
        region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && region.demand == crate::transcompile::LayoutDemand::Supporting
    }) {
        return Constraint::Length(14);
    }
    match region.map(|region| region.demand) {
        Some(crate::transcompile::LayoutDemand::Expand) => Constraint::Min(8),
        Some(crate::transcompile::LayoutDemand::Supporting) => Constraint::Min(5),
        Some(crate::transcompile::LayoutDemand::Compact) => Constraint::Length(3),
        Some(crate::transcompile::LayoutDemand::Minimal) => Constraint::Length(3),
        _ => Constraint::Min(3),
    }
}

fn split_constraint(plan: &TuiLayoutPlan, node: &LayoutNode, _width: u16) -> Constraint {
    match region_for_node(plan, node).map(|region| region.demand) {
        Some(crate::transcompile::LayoutDemand::Expand) => Constraint::Min(30),
        Some(crate::transcompile::LayoutDemand::Supporting) => Constraint::Min(22),
        Some(crate::transcompile::LayoutDemand::Compact) => Constraint::Length(20),
        Some(crate::transcompile::LayoutDemand::Minimal) => Constraint::Length(14),
        _ => Constraint::Min(16),
    }
}

fn spatial_title(region: &SpatialRegion) -> String {
    region.presentation.title.clone()
}

fn context_summary(region: &SpatialRegion) -> bool {
    matches!(
        region.demand,
        crate::transcompile::LayoutDemand::Compact | crate::transcompile::LayoutDemand::Minimal
    ) && !matches!(
        region.semantic_kind,
        crate::transcompile::SemanticRegionKind::Dialog
            | crate::transcompile::SemanticRegionKind::Overlay
    ) && (region.presentation.kind == RegionPresentationKind::CommandBar
        || (region.presentation.kind == RegionPresentationKind::Navigation
            && region.kind == crate::transcompile::SpatialRegionKind::TabStrip))
}

fn render_spatial_leaf(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    region: &SpatialRegion,
    hit_regions: &mut Vec<HitRegion>,
) {
    if context_summary(region) {
        // Each region retains its own row, identity and focus position. Only
        // its frame is compressed; same-named contexts are not merged.
        let title = format!(
            "{}{} · ",
            if context.active_region == Some(region.id) {
                "> "
            } else {
                ""
            },
            spatial_title(region)
        );
        let title = clipped(&title, area.width / 2);
        let width = Line::from(title.as_str()).width() as u16;
        frame.render_widget(
            Paragraph::new(title).style(if context.active_region == Some(region.id) {
                accent()
            } else {
                secondary()
            }),
            Rect::new(area.x, area.y, width, 1),
        );
        let payload = Rect::new(
            area.x.saturating_add(width),
            area.y,
            area.width.saturating_sub(width),
            1,
        );
        render_compact_surface(frame, payload, context, region, hit_regions);
        return;
    }
    let active = context.active_region == Some(region.id);
    let borders = if matches!(
        region.presentation.kind,
        RegionPresentationKind::Form | RegionPresentationKind::Table
    ) || matches!(
        region.semantic_kind,
        crate::transcompile::SemanticRegionKind::Dialog
            | crate::transcompile::SemanticRegionKind::Overlay
    ) {
        Borders::ALL
    } else {
        Borders::TOP
    };
    let block = Block::default()
        .title(format!(
            " {}{} ",
            if context.active_region == Some(region.id) {
                "> "
            } else {
                ""
            },
            spatial_title(region)
        ))
        .borders(borders)
        .border_style(if active {
            accent()
        } else {
            Style::default().fg(Color::DarkGray)
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if region.kind == crate::transcompile::SpatialRegionKind::PrimaryContent
        && matches!(
            region.presentation.kind,
            RegionPresentationKind::InlineContent | RegionPresentationKind::GraphicalPlaceholder
        )
    {
        if let Some(inline) = context.inline_content.as_ref() {
            let areas = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);
            let suffix = if inline.partial { " (partial)" } else { "" };
            let mut lines = inline.lines.clone();
            if let Some(element) = context
                .scene
                .elements
                .iter()
                .find(|element| {
                    element
                        .sources
                        .iter()
                        .any(|source| region.presentation.source_nodes.contains(source))
                        && element.is_focusable()
                })
                .or_else(|| {
                    context.scene.elements.iter().find(|element| {
                        matches!(element.kind, SceneElementKind::DocumentSummary { .. })
                            && element.is_focusable()
                    })
                })
            {
                if let Some(first) = lines.first_mut() {
                    let marker = if context.focused == Some(element.id) {
                        "> "
                    } else {
                        "  "
                    };
                    *first = format!("{marker}{first}");
                }
                if let Some(interaction) = interaction(element)
                    && areas[1].height > 0
                {
                    hit_regions.push(HitRegion {
                        scene_id: element.id,
                        rect: Rect::new(areas[1].x, areas[1].y, areas[1].width, 1),
                        interaction,
                    });
                }
            }
            let status = format!("preview{suffix}");
            let status_width = Line::from(status.as_str())
                .width()
                .min(usize::from(u16::MAX)) as u16;
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        clipped(
                            &inline.title,
                            areas[0]
                                .width
                                .saturating_sub(status_width.saturating_add(3)),
                        ),
                        secondary(),
                    ),
                    Span::styled(
                        format!(" · {status}"),
                        if inline.partial {
                            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            secondary()
                        },
                    ),
                ])),
                areas[0],
            );
            frame.render_widget(
                Paragraph::new(lines.join("\n"))
                    .wrap(Wrap { trim: true })
                    .scroll((context.scroll_offset, 0)),
                areas[1],
            );
            return;
        }
        if region.presentation.kind == RegionPresentationKind::InlineContent
            && let Some(element) = context.scene.elements.iter().find(|element| {
                matches!(element.kind, SceneElementKind::DocumentSummary { .. })
                    && element.is_focusable()
            })
        {
            let mut lines = element_lines_for_width(
                element,
                context.focused == Some(element.id),
                context.edit_session,
                inner.width,
                context.external_text_handler_available,
            );
            lines.push("  Semantic content preview unavailable; Enter opens Reader.".into());
            frame.render_widget(
                Paragraph::new(lines.join("\n")).wrap(Wrap { trim: true }),
                inner,
            );
            if let Some(interaction) = interaction(element) {
                hit_regions.push(HitRegion {
                    scene_id: element.id,
                    rect: Rect::new(inner.x, inner.y, inner.width, 1),
                    interaction,
                });
            }
            return;
        }
        let placeholder =
            if region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder {
                "Graphical content\n[View / Materialize]"
            } else {
                "Semantic content\n[Reader available: Enter]"
            };
        frame.render_widget(Paragraph::new(placeholder).wrap(Wrap { trim: true }), inner);
        return;
    }
    if region.demand == crate::transcompile::LayoutDemand::Compact {
        render_compact_surface(frame, inner, context, region, hit_regions);
        return;
    }
    let sources = &region.presentation.source_nodes;
    let mut elements = context
        .scene
        .elements
        .iter()
        .filter(|element| {
            element
                .sources
                .iter()
                .any(|source| sources.contains(source))
                && !matches!(element.kind, SceneElementKind::Unsupported { .. })
                && !matches!(element.kind, SceneElementKind::CommandHeader { .. })
        })
        .collect::<Vec<_>>();
    let contextual_limit =
        if region.presentation.kind == RegionPresentationKind::ControlGroup && elements.len() > 4 {
            // Large coalesced action sets stay bound individually, but ordinary
            // scene rendering shows only a bounded contextual sample. The focused
            // control is kept visible; the existing palette remains the complete
            // command surface.
            elements.sort_by_key(|element| context.focused != Some(element.id));
            usize::from(if context.active_region == Some(region.id) {
                inner.height.saturating_sub(1).min(8)
            } else {
                inner.height.saturating_sub(1).min(3)
            })
        } else {
            elements.len()
        };
    // An aggregate workspace can contain more rows than its allocated pane.
    // Keep terminal focus visible without changing the shared scene or order.
    let start = elements
        .iter()
        .position(|element| context.focused == Some(element.id))
        .filter(|index| {
            elements[..=*index]
                .iter()
                .map(|element| {
                    element_lines_for_width(
                        element,
                        context.focused == Some(element.id),
                        context.edit_session,
                        inner.width,
                        context.external_text_handler_available,
                    )
                    .len()
                })
                .sum::<usize>()
                > usize::from(inner.height)
        })
        .unwrap_or(0);
    let hidden = elements
        .len()
        .saturating_sub(start)
        .saturating_sub(contextual_limit);
    let mut rendered = 0_u16;
    for element in elements.into_iter().skip(start).take(contextual_limit) {
        if rendered >= inner.height {
            break;
        }
        let focused = context.focused == Some(element.id);
        let lines = element_lines_for_width(
            element,
            focused,
            context.edit_session,
            inner.width,
            context.external_text_handler_available,
        );
        for line in lines {
            if rendered >= inner.height {
                break;
            }
            frame.render_widget(
                Paragraph::new(line).style(if focused {
                    selected_style()
                } else if matches!(
                    element.kind,
                    SceneElementKind::Group { .. } | SceneElementKind::Unsupported { .. }
                ) {
                    secondary()
                } else {
                    Style::default()
                }),
                Rect::new(inner.x, inner.y + rendered, inner.width, 1),
            );
            rendered += 1;
        }
        if let Some(interaction) = interaction(element) {
            hit_regions.push(HitRegion {
                scene_id: element.id,
                rect: Rect::new(
                    inner.x,
                    inner.y.saturating_add(rendered.saturating_sub(1)),
                    inner.width,
                    1,
                ),
                interaction,
            });
        }
    }
    if hidden > 0 && rendered < inner.height {
        frame.render_widget(
            Paragraph::new(format!("… {hidden} more · Tab cycles controls")),
            Rect::new(inner.x, inner.y + rendered, inner.width, 1),
        );
        rendered += 1;
    }
    if rendered == 0 {
        let text = match region.presentation.kind {
            RegionPresentationKind::CommandBar => "Press : to browse and search commands",
            RegionPresentationKind::InputSurface => "— read only",
            RegionPresentationKind::Navigation
                if region.kind == crate::transcompile::SpatialRegionKind::TabStrip =>
            {
                "Current context"
            }
            RegionPresentationKind::GraphicalPlaceholder => {
                "Graphical content\n[View / Materialize]"
            }
            _ => "— empty",
        };
        frame.render_widget(Paragraph::new(text), inner);
    }
}

fn render_compact_surface(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    region: &SpatialRegion,
    hit_regions: &mut Vec<HitRegion>,
) {
    let mut elements = context
        .scene
        .elements
        .iter()
        .filter(|element| {
            element
                .sources
                .iter()
                .any(|source| region.presentation.source_nodes.contains(source))
                && !matches!(
                    element.kind,
                    SceneElementKind::Unsupported { .. }
                        | SceneElementKind::CommandHeader { .. }
                        | SceneElementKind::Group { .. }
                )
        })
        .collect::<Vec<_>>();
    elements.sort_by_key(|element| {
        (
            context.focused != Some(element.id),
            element.capability() == InteractionCapability::None,
        )
    });
    if let Some(session) = context.edit_session
        && elements.iter().any(|element| {
            element
                .binding
                .as_ref()
                .is_some_and(|binding| binding.runtime_id == session.target)
        })
    {
        frame.render_widget(
            Paragraph::new(format!(
                "[editing] {}",
                edit_buffer_window(session, area.width.saturating_sub(10))
            ))
            .style(accent()),
            area,
        );
        return;
    }
    let mut x = area.x;
    let mut shown = 0_usize;
    for element in &elements {
        let focused = context.focused == Some(element.id);
        let label = match &element.kind {
            SceneElementKind::Field { label, display, .. } => format!(
                "[{}{label}: {display}{}]",
                if focused { "> " } else { "" },
                if element.capability() == InteractionCapability::None {
                    " · read only"
                } else {
                    ""
                }
            ),
            SceneElementKind::Value { label, display } => {
                format!("[{}{label}: {display}]", if focused { "> " } else { "" },)
            }
            SceneElementKind::Checkbox { label, checked } => format!(
                "[{}{} {label}]",
                if focused { "> " } else { "" },
                if *checked { "x" } else { " " },
            ),
            _ => format!("[{}{}]", if focused { "> " } else { "" }, element.label()),
        };
        let reserve = if shown + 1 < elements.len() { 10 } else { 0 };
        let available = area.right().saturating_sub(x).saturating_sub(reserve);
        if available < 4 {
            break;
        }
        // A long field must not suppress every later button. Share the compact
        // row, clip by terminal cells, and keep the exact full scene for navigation.
        let share = if reserve > 0 {
            (area.width / 3).max(16).min(available)
        } else {
            available
        };
        let label = clipped(&label, share);
        let width = Line::from(label.as_str()).width() as u16;
        let style = if focused {
            selected_style()
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(label).style(style),
            Rect::new(x, area.y, width.min(area.right().saturating_sub(x)), 1),
        );
        if let Some(interaction) = interaction(element) {
            hit_regions.push(HitRegion {
                scene_id: element.id,
                rect: Rect::new(x, area.y, width, 1),
                interaction,
            });
        }
        x = x.saturating_add(width).saturating_add(1);
        shown += 1;
    }
    if shown < elements.len() {
        frame.render_widget(
            Paragraph::new("[F3 More]"),
            Rect::new(x.min(area.right().saturating_sub(9)), area.y, 9, 1),
        );
    } else if shown == 0 {
        let label = match region.presentation.kind {
            RegionPresentationKind::CommandBar => ": Commands",
            RegionPresentationKind::InputSurface => "— read only",
            RegionPresentationKind::Navigation
                if region.kind == crate::transcompile::SpatialRegionKind::TabStrip =>
            {
                "Current context"
            }
            _ => "— empty",
        };
        frame.render_widget(Paragraph::new(label), area);
    }
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
        let lines = element_lines_for_width(
            element,
            focused,
            context.edit_session,
            area.width,
            context.external_text_handler_available,
        );
        let line_count = lines.len().min(usize::from(u16::MAX)) as u16;
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
                selected_style()
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
        logical_top = logical_top.saturating_add(line_count);
    }
}

fn interaction(element: &SceneElement) -> Option<HitInteraction> {
    match element.kind {
        SceneElementKind::Button { .. }
        | SceneElementKind::Toggle { .. }
        | SceneElementKind::Checkbox { .. }
        | SceneElementKind::SelectionItem { .. }
        | SceneElementKind::Disclosure { .. }
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
        SceneElementKind::Value { .. } => Some(HitInteraction::Focus),
        SceneElementKind::DocumentSummary { .. } => Some(HitInteraction::Activate),
        _ => None,
    }
}

pub fn element_lines(element: &SceneElement, focused: bool) -> Vec<String> {
    element_lines_for_width(element, focused, None, u16::MAX, false)
}

fn element_lines_for_width(
    element: &SceneElement,
    focused: bool,
    edit_session: Option<&EditSession>,
    width: u16,
    external_text_handler_available: bool,
) -> Vec<String> {
    let marker = if focused { "> " } else { "  " };
    let unavailable =
        if element.capability() == InteractionCapability::None && element.is_focusable() {
            match element.kind {
                SceneElementKind::Field { .. } | SceneElementKind::Value { .. } => " · read only",
                SceneElementKind::Selector { .. } => " · options unavailable",
                _ => " · action unavailable",
            }
        } else {
            ""
        };
    match &element.kind {
        SceneElementKind::Text { text } | SceneElementKind::Status { text } => {
            SceneElement::wrap_display_lines(&format!("  {text}"), width)
        }
        SceneElementKind::Hint { text } => {
            SceneElement::wrap_display_lines(&format!("    Hint: {text}"), width)
        }
        SceneElementKind::Error { text } => {
            SceneElement::wrap_display_lines(&format!("    Error: {text}"), width)
        }
        SceneElementKind::Group { label } | SceneElementKind::CommandHeader { label } => {
            vec![format!("  {label}:")]
        }
        SceneElementKind::Button { label } => vec![format!("{marker}[{label}]{unavailable}")],
        SceneElementKind::Toggle { label, pressed } => vec![format!(
            "{marker}[{}] {label}{unavailable}",
            if *pressed { "x" } else { " " }
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
                    format!(
                        "    > {}",
                        edit_buffer_window(session, width.saturating_sub(6))
                    ),
                ]
            } else if element.height_for_width(width) == 1 {
                vec![format!("{marker}{label}: {display}{unavailable}")]
            } else {
                vec![
                    format!("{marker}{label}{unavailable}"),
                    format!("    > {display}"),
                ]
            }
        }
        SceneElementKind::Selector { label } => vec![format!("{marker}[{label} ▾]{unavailable}")],
        SceneElementKind::Value { label, display } => vec![format!(
            "{marker}{label}: {display}  [↑ increase / ↓ decrease]{unavailable}"
        )],
        SceneElementKind::DocumentSummary {
            title,
            blocks,
            headings,
            links,
            forms,
            completeness,
            external_edit,
        } => vec![
            format!("{marker}Document: {title}"),
            format!("    {blocks} blocks | {headings} headings | {links} links | {forms} forms"),
            format!("    completeness: {completeness}"),
            if *external_edit && external_text_handler_available {
                "    [ Enter: Read document | e: Edit externally ]".to_owned()
            } else if *external_edit {
                "    [ Enter: Read document | F3: Actions | external editor not configured ]"
                    .to_owned()
            } else {
                "    [ Enter: Read document ]".to_owned()
            },
            "    o Outline | / Content search".to_owned(),
        ],
        SceneElementKind::SelectionItem { label, selected } => vec![format!(
            "{marker}{} {label}{}{unavailable}",
            if *selected { "*" } else { "•" },
            if element.capability() == InteractionCapability::SwitchPage {
                "  [Enter: Switch page]"
            } else {
                ""
            }
        )],
        SceneElementKind::Disclosure { label, expanded } => vec![format!(
            "{marker}{} {label}  [Enter: {}]{unavailable}",
            if *expanded { "▾" } else { "▸" },
            if *expanded { "Collapse" } else { "Expand" }
        )],
        SceneElementKind::Command { path } => vec![format!("{marker}{path}{unavailable}")],
        SceneElementKind::OpaqueContent { label, dimensions } => vec![
            format!("  {label}"),
            dimensions.map_or_else(
                || "  [fidelity-required content]".to_owned(),
                |(w, h)| format!("  [fidelity-required content: {w}×{h} GUI pixels]"),
            ),
        ],
        SceneElementKind::Unsupported { label } => vec![format!("  Read only · {label}")],
    }
}

pub(crate) fn palette_areas(area: Rect) -> [Rect; 6] {
    // Leave context around the floating palette when the terminal can afford it.
    let width = if area.width >= 60 {
        (area.width.saturating_mul(9) / 10).min(112)
    } else {
        area.width
    };
    let height = if area.height >= 16 {
        (area.height.saturating_mul(4) / 5).min(28)
    } else {
        area.height
    };
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let inner = Block::default().borders(Borders::ALL).inner(popup);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .split(inner);
    let body = rows[2];
    let panes = if body.width >= 100 {
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(body)
    } else {
        let detail_height = (body.height / 3)
            .clamp(3, 7)
            .min(body.height.saturating_sub(2));
        Layout::vertical([Constraint::Min(2), Constraint::Length(detail_height)]).split(body)
    };
    [popup, rows[0], rows[1], panes[0], panes[1], rows[3]]
}

/// One viewport calculation for Human result rendering and Agent row bindings.
pub(crate) fn palette_result_range(
    results: Rect,
    count: usize,
    selected: usize,
) -> std::ops::Range<usize> {
    let visible = usize::from(Block::default().borders(Borders::TOP).inner(results).height);
    let start = selected
        .saturating_sub(visible / 2)
        .min(count.saturating_sub(visible));
    start..start.saturating_add(visible).min(count)
}

fn palette_help(width: u16) -> &'static str {
    if width < 60 {
        "↑↓ Select · Enter Use · Esc Back"
    } else {
        "↑/↓ Select · Enter Use · PgUp/PgDn Details · Esc Back"
    }
}

fn render_palette(frame: &mut Frame<'_>, area: Rect, palette: PaletteRender<'_>) {
    let [popup, search, scope, results, details, footer] = palette_areas(area);
    frame.render_widget(Clear, popup);
    let search_scope = if palette.all_scopes {
        "all application commands"
    } else {
        "current interaction scope"
    };
    frame.render_widget(
        Block::default()
            .title(format!(
                " {} {}/{} ",
                if palette.scope_switchable {
                    "Commands"
                } else {
                    "Public actions"
                },
                palette
                    .selected
                    .saturating_add(1)
                    .min(palette.entries.len()),
                palette.entries.len()
            ))
            .borders(Borders::ALL)
            .border_style(accent()),
        popup,
    );
    frame.render_widget(
        Paragraph::new(clipped(
            &format!("> Search: {}", palette.query),
            search.width,
        ))
        .style(accent()),
        search,
    );
    frame.render_widget(
        Paragraph::new(clipped(
            &format!(
                "Scope: {search_scope}{}",
                if palette.scope_switchable {
                    " · F2 toggle"
                } else {
                    " · fixed"
                }
            ),
            scope.width,
        ))
        .style(secondary()),
        scope,
    );
    let result_block = Block::default()
        .title(" Results ")
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));
    let list_area = result_block.inner(results);
    frame.render_widget(result_block, results);
    let range = palette_result_range(results, palette.entries.len(), palette.selected);
    let start = range.start;
    for (index, entry) in palette
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(range.len())
    {
        let selected = index == palette.selected;
        let section = entry.section_label().filter(|section| {
            index == start || palette.entries[index - 1].section_label() != Some(*section)
        });
        let text = format!(
            "{} {}{}",
            if selected { ">" } else { " " },
            section.map_or_else(
                || entry.short_label(),
                |label| format!("{label} · {}", entry.short_label())
            ),
            if entry.group { " ›" } else { "" }
        );
        frame.render_widget(
            Paragraph::new(clipped(&text, list_area.width)).style(if selected {
                selected_style()
            } else {
                Style::default()
            }),
            Rect::new(
                list_area.x,
                list_area.y + (index - start) as u16,
                list_area.width,
                1,
            ),
        );
    }
    if palette.entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching operations").style(secondary()),
            list_area,
        );
    }
    let detail_block = Block::default()
        .title(" Details · PgUp/PgDn ")
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));
    let detail_area = detail_block.inner(details);
    frame.render_widget(detail_block, details);
    if let Some(entry) = palette.entries.get(palette.selected) {
        let lines = super::palette::detail_lines(&entry.detail_text(), detail_area.width);
        let offset = palette
            .detail_scroll
            .min(lines.len().saturating_sub(usize::from(detail_area.height)));
        frame.render_widget(
            Paragraph::new(
                lines
                    .into_iter()
                    .skip(offset)
                    .take(usize::from(detail_area.height))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            detail_area,
        );
    }
    frame.render_widget(
        Paragraph::new(clipped(palette_help(footer.width), footer.width)).style(secondary()),
        footer,
    );
}

fn edit_buffer_window(session: &EditSession, width: u16) -> String {
    if width == 0 {
        return String::new();
    }
    let (mut rendered, byte) = session.buffer.visible_window(width.saturating_sub(1));
    rendered.insert(byte, '|');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        runtime::ApplicationGenerationId,
        semantic::{
            BackendLocator, DebugInfo, RuntimeNodeId, SemanticNode, SemanticRole, SemanticState,
        },
        transcompile::{
            CompositionKind, CoordinateSpace, GeometryTrust, LayoutImportance,
            PresentationPriority, PresentationStrategy, RegionId, RegionPresentation, SceneBinding,
            SemanticRegionKind, SpatialRegionId, SpatialRegionKind,
        },
        tui::action::UiIntent,
    };
    use ratatui::{Terminal, backend::TestBackend};

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

    fn spatial_region(
        id: u64,
        title: &str,
        kind: RegionPresentationKind,
        demand: crate::transcompile::LayoutDemand,
    ) -> SpatialRegion {
        SpatialRegion {
            id: SpatialRegionId(id),
            semantic_region: RegionId::new(id),
            source_nodes: vec![RuntimeNodeId::new(id + 1)],
            semantic_kind: SemanticRegionKind::Control,
            kind: SpatialRegionKind::Auxiliary,
            priority: PresentationPriority::Auxiliary,
            importance: LayoutImportance::Supporting,
            obligation: crate::transcompile::PresentationObligation::Persistent,
            demand,
            visibility: crate::transcompile::VisibilityGuarantee::Collapsible,
            purpose: crate::transcompile::InteractionPurpose::Unknown,
            bounds: None,
            coordinate_space: CoordinateSpace::Unknown,
            presentation: RegionPresentation {
                kind,
                title: title.into(),
                source_regions: vec![RegionId::new(id)],
                source_nodes: vec![RuntimeNodeId::new(id + 1)],
                meaningful_items: 1,
                dominant_eligible: false,
                reasons: Vec::new(),
            },
            reasons: Vec::new(),
        }
    }

    #[test]
    fn narrow_unicode_editor_keeps_the_cursor_inside_its_cell_budget() {
        let text = "中文abc".repeat(20);
        let mut edit = EditSession::new(
            RuntimeNodeId::new(1),
            BackendLocator::new(":1.2", "/input"),
            text.clone(),
            1,
        );
        for _ in 0..35 {
            edit.buffer.move_left();
        }
        let view = edit_buffer_window(&edit, 18);
        assert!(view.contains('|'));
        assert!(Line::from(view.as_str()).width() <= 18);
        assert_eq!(edit.buffer.text(), text);
    }

    #[test]
    fn routine_status_noise_does_not_hide_errors_or_coverage_warnings() {
        assert!(!normal_footer_status(
            "Reader loaded 2 blocks (text_rpcs=0 ranges=0 cache=0 bytes) in 1 ms"
        ));
        assert!(!normal_footer_status("Choice overlay — arrows select"));
        assert!(normal_footer_status(
            "Reader content load failed: unavailable"
        ));
        assert!(normal_footer_status(
            "Public actions — partial tree; refresh"
        ));
        assert!(normal_footer_status(
            "External change detected; commit blocked"
        ));
    }

    #[test]
    fn explanation_text_wraps_without_losing_its_tail_or_row_height() {
        let e = element(SceneElementKind::Text {
            text: "Confirm the project details before applying changes. 中文说明。".into(),
        });
        let lines = element_lines_for_width(&e, false, None, 38, false);
        assert!(lines.len() > 1);
        assert_eq!(usize::from(e.height_for_width(38)), lines.len());
        assert_eq!(lines.concat(), format!("  {}", e.label()));
        assert!(
            lines
                .iter()
                .all(|line| Line::from(line.as_str()).width() <= 38)
        );
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
            vec!["> [Apply]"]
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
        assert_eq!(
            element_lines_for_width(&field, false, None, 12, false).len(),
            2
        );
        assert_eq!(
            element_lines_for_width(&field, false, None, 80, false).len(),
            1
        );
        assert_eq!(field.height_for_width(12), 2);
        assert_eq!(field.height_for_width(80), 1);
        let wide = element(SceneElementKind::Field {
            label: "名字".into(),
            display: "中文".into(),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        assert_eq!(wide.height_for_width(10), 2);
        assert_eq!(
            element_lines_for_width(&wide, true, None, 10, false).len(),
            2
        );
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
            vec!["  [ ] Enable feature · action unavailable"]
        );
    }

    #[test]
    fn capability_wording_never_promises_an_unavailable_external_handler() {
        let document = element(SceneElementKind::DocumentSummary {
            title: "Notes".into(),
            blocks: 2,
            headings: 0,
            links: 0,
            forms: 0,
            completeness: "Complete".into(),
            external_edit: true,
        });
        let missing = element_lines_for_width(&document, true, None, 120, false);
        assert!(
            missing
                .iter()
                .any(|line| line.contains("external editor not configured"))
        );
        assert!(missing.iter().any(|line| line.contains("F3: Actions")));
        assert!(!missing.iter().any(|line| line.contains("e: Edit")));

        let configured = element_lines_for_width(&document, true, None, 120, true);
        assert!(
            configured
                .iter()
                .any(|line| line.contains("e: Edit externally"))
        );

        let unavailable = element(SceneElementKind::Unsupported {
            label: "Tree".into(),
        });
        assert_eq!(
            element_lines(&unavailable, false),
            vec!["  Read only · Tree"]
        );
    }

    #[test]
    fn qualified_complex_text_field_advertises_external_edit_command() {
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/node/1"),
            index_in_parent: None,
            role: SemanticRole::TextInput,
            name: Some("Notes".into()),
            description: None,
            value: Some("alpha\nbeta\n".into()),
            text_input_kind: Some(crate::semantic::TextInputKind::Plain),
            states: vec![SemanticState::Enabled],
            actions: Vec::new(),
            capabilities: vec![
                SemanticCapability::EditText,
                SemanticCapability::EditComplexText,
            ],
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        };
        let mut field = element(SceneElementKind::Field {
            label: "Notes".into(),
            display: "alpha…".into(),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        field.binding.as_mut().unwrap().semantic_role = SemanticRole::TextInput;
        field.binding.as_mut().unwrap().capability = InteractionCapability::EditText;
        field.binding.as_mut().unwrap().default_intent = UiIntent::BeginEdit;
        let field_id = field.id;
        let scene = TuiScene::new("Editor".into(), &root, vec![field]);
        let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: Some(field_id),
                        scroll_offset: 0,
                        status: "ready",
                        application_available: true,
                        external_text_handler_available: true,
                        edit_session: None,
                        palette: None,
                        choice: None,
                        content: None,
                        spatial: None,
                        active_region: None,
                        inline_content: None,
                    },
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("e Edit externally"));
        for document_summary in [false, true] {
            let mut focused = scene.element(field_id).unwrap().clone();
            if document_summary {
                focused.kind = SceneElementKind::DocumentSummary {
                    title: "Notes".into(),
                    blocks: 2,
                    headings: 0,
                    links: 0,
                    forms: 0,
                    completeness: "Complete".into(),
                    external_edit: true,
                };
                let binding = focused.binding.as_mut().unwrap();
                binding.capability = InteractionCapability::BrowseContent;
                binding.default_intent = UiIntent::BeginRead;
            }
            let missing_handler_scene = TuiScene::new("Editor".into(), &root, vec![focused]);
            terminal
                .draw(|frame| {
                    render(
                        frame,
                        RenderContext {
                            scene: &missing_handler_scene,
                            interaction_scope: None,
                            focused: Some(field_id),
                            scroll_offset: 0,
                            status: "ready",
                            application_available: true,
                            external_text_handler_available: false,
                            edit_session: None,
                            palette: None,
                            choice: None,
                            content: None,
                            spatial: None,
                            active_region: None,
                            inline_content: None,
                        },
                    );
                })
                .unwrap();
            let footer = (0..100)
                .map(|x| terminal.backend().buffer()[(x, 11)].symbol())
                .collect::<String>();
            assert!(footer.contains("F3 Actions"));
            assert!(footer.contains("external editor not configured"));
            assert!(!footer.contains("e Edit externally"));
            assert_eq!(footer.contains("Enter Read"), document_summary);
        }
        let edit = EditSession::new(
            RuntimeNodeId::new(1),
            BackendLocator::new(":1.2", "/node/1"),
            "typed value".into(),
            1,
        );
        let region = spatial_region(
            0,
            "Notes",
            RegionPresentationKind::InputSurface,
            crate::transcompile::LayoutDemand::Compact,
        );
        terminal
            .draw(|frame| {
                render_compact_surface(
                    frame,
                    Rect::new(0, 0, 40, 3),
                    &RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: Some(field_id),
                        scroll_offset: 0,
                        status: "ready",
                        application_available: true,
                        external_text_handler_available: true,
                        edit_session: Some(&edit),
                        palette: None,
                        choice: None,
                        content: None,
                        spatial: None,
                        active_region: None,
                        inline_content: None,
                    },
                    &region,
                    &mut Vec::new(),
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("[editing] typed value|"));
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
        let lines = element_lines_for_width(&field, true, Some(&session), 14, false);
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
        assert!(element_lines(&opaque, false)[1].contains("fidelity-required"));
    }

    #[test]
    fn spatial_inline_content_is_clipped_and_scrolled_by_the_actual_pane() {
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/root"),
            index_in_parent: None,
            role: SemanticRole::Document,
            name: Some("Document".into()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![SemanticState::Enabled],
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        };
        let scene = TuiScene::new("Application".into(), &root, Vec::new());
        let plan = TuiLayoutPlan {
            root: LayoutNode::Leaf(SpatialRegionId(0)),
            regions: vec![SpatialRegion {
                id: SpatialRegionId(0),
                semantic_region: RegionId::new(0),
                source_nodes: vec![RuntimeNodeId::new(1)],
                semantic_kind: SemanticRegionKind::Content,
                kind: SpatialRegionKind::PrimaryContent,
                priority: PresentationPriority::Primary,
                importance: LayoutImportance::Dominant,
                obligation: crate::transcompile::PresentationObligation::Persistent,
                demand: crate::transcompile::LayoutDemand::Expand,
                visibility: crate::transcompile::VisibilityGuarantee::Collapsible,
                purpose: crate::transcompile::InteractionPurpose::Unknown,
                bounds: None,
                coordinate_space: CoordinateSpace::Unknown,
                presentation: RegionPresentation {
                    kind: RegionPresentationKind::InlineContent,
                    title: "Document".into(),
                    source_regions: vec![RegionId::new(0)],
                    source_nodes: vec![RuntimeNodeId::new(1)],
                    meaningful_items: 20,
                    dominant_eligible: true,
                    reasons: Vec::new(),
                },
                reasons: Vec::new(),
            }],
            topology: crate::transcompile::SpatialTopology::default(),
            generation: ApplicationGenerationId(1),
            geometry_trust: GeometryTrust::Unavailable,
            composition: CompositionKind::ContentDominant,
        };
        let lines = (0..20).map(|line| format!("line {line}")).collect();
        let inline = InlineContentRender {
            title: "Long document title ".repeat(20),
            lines,
            total_lines: 20,
            partial: true,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: None,
                        scroll_offset: 3,
                        status: "ready",
                        application_available: true,
                        external_text_handler_available: false,
                        edit_session: None,
                        palette: None,
                        choice: None,
                        content: None,
                        spatial: Some(&plan),
                        active_region: Some(SpatialRegionId(0)),
                        inline_content: Some(inline.clone()),
                    },
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("line 3"));
        assert!(!rendered.contains("Regions —"));
        assert!(!rendered.contains("line 0"));
        assert!(!rendered.contains("line 19"));
        assert!(rendered.contains("preview (partial)"));
    }

    #[test]
    fn region_navigation_reports_width_overflow_compactly() {
        let items = (0..6)
            .map(|id| RegionNavigationItem {
                id: SpatialRegionId(id),
                title: format!("Surface {id}"),
                empty: false,
            })
            .collect::<Vec<_>>();
        let mut terminal = Terminal::new(TestBackend::new(32, 1)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(navigation_line("Region", &items, 0, frame.area().width)),
                    frame.area(),
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("[Surface 0]"));
        assert!(!rendered.contains("Surface 5"));
        let clipped_label = clipped("中文e\u{301}标题", 5);
        assert!(Line::from(clipped_label.as_str()).width() <= 5);
        assert!(!clipped_label.ends_with('e'));
    }

    #[test]
    fn reader_can_show_the_tail_of_one_large_accessibility_block() {
        use crate::content::ContentBlockKind;
        let text = (0..120)
            .map(|i| format!("Public line {i}\n"))
            .collect::<String>()
            + "LONG-BLOCK-TAIL";
        let draw = |offset| {
            let mut terminal = Terminal::new(TestBackend::new(80, 18)).unwrap();
            terminal
                .draw(|frame| {
                    render_content(
                        frame,
                        frame.area(),
                        ContentRender {
                            title: "Document".into(),
                            mode: ContentViewMode::Reader,
                            blocks: vec![ReaderBlock {
                                id: crate::content::ContentBlockId::new(1),
                                source: RuntimeNodeId::new(1),
                                kind: ContentBlockKind::Paragraph,
                                text: text.clone(),
                            }],
                            reader_scroll: offset,
                            outline: vec![],
                            outline_selected: 0,
                            query: String::new(),
                            results: vec![],
                            result_selected: 0,
                            partial: false,
                            full_search: None,
                            structure_lines: vec![],
                            table_row_selection: false,
                            table_cell_activation: false,
                        },
                    )
                })
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        };
        assert!(!draw(0).contains("LONG-BLOCK-TAIL"));
        assert!(draw(110).contains("LONG-BLOCK-TAIL"));
        assert!(text.ends_with("LONG-BLOCK-TAIL"));
    }

    #[test]
    fn palette_floats_with_context_and_falls_back_in_small_terminals() {
        for area in [Rect::new(0, 0, 100, 28), Rect::new(2, 3, 140, 38)] {
            let [popup, _, _, results, details, _] = palette_areas(area);
            assert!(popup.x > area.x && popup.y > area.y);
            assert!(popup.right() < area.right() && popup.bottom() < area.bottom());
            assert!(results.height >= 3 && details.height >= 3);
            let range = palette_result_range(results, 949, 948);
            assert!(range.contains(&948));
            assert!(range.end <= 949);
        }
        let small = Rect::new(0, 0, 40, 12);
        assert_eq!(palette_areas(small)[0], small);
    }

    #[test]
    fn long_palette_labels_do_not_wrap_over_other_results() {
        use crate::transcompile::{CommandGroup, CommandHierarchy, InteractionScopeId};
        use crate::tui::palette::{CommandPalette, PublicOperation};
        let scope = InteractionScopeId(RuntimeNodeId::new(1));
        let mut palette = CommandPalette::public_actions(
            CommandHierarchy {
                root: CommandGroup {
                    source: RuntimeNodeId::new(1),
                    label: "App".into(),
                    scope,
                    children: vec![],
                },
            },
            scope,
            vec![
                (
                    RuntimeNodeId::new(2),
                    BackendLocator::new(":1.2", "/first"),
                    "Very long full name ".repeat(400),
                    PublicOperation::Action("press".into()),
                ),
                (
                    RuntimeNodeId::new(3),
                    BackendLocator::new(":1.2", "/second"),
                    "Other".into(),
                    PublicOperation::Action("press".into()),
                ),
            ],
        );
        palette.set_target_titles(|_| Some("中文e\u{301} long title ".repeat(50)));
        for (width, height) in [(60, 20), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    render_palette(
                        frame,
                        frame.area(),
                        PaletteRender {
                            query: "",
                            entries: palette.entries(),
                            selected: 0,
                            all_scopes: false,
                            scope_switchable: false,
                            detail_scroll: 0,
                        },
                    )
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let rows = (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            let first = rows
                .iter()
                .position(|row| row.contains("Action: press"))
                .unwrap();
            assert!(rows[first + 1].contains("Action: press"));
            assert!(
                palette.entries()[0]
                    .detail_text()
                    .contains("Snapshot target: #2")
            );
        }
    }

    #[test]
    fn palette_visible_rows_are_bounded_and_include_nonselected_results() {
        for (height, count, selected, expected) in [
            (5, 30, 0, 0..4),
            (5, 30, 15, 13..17),
            (5, 30, 29, 26..30),
            (5, 2, 0, 0..2),
            (0, 30, 0, 0..0),
        ] {
            let range = palette_result_range(Rect::new(0, 0, 60, height), count, selected);
            assert_eq!(range, expected);
            assert!(range.end <= count);
        }
    }

    #[test]
    fn spatial_workspace_keeps_a_clipped_focused_reader_visible() {
        let mut root = SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", "/root"),
            index_in_parent: None,
            role: SemanticRole::Window,
            name: Some("Workspace".into()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: Default::default(),
        };
        let mut elements = Vec::new();
        for index in 1..=9 {
            let mut control = element(SceneElementKind::Button {
                label: format!("Control {index}"),
            });
            control.id = SceneElementId::new(index);
            control.sources = vec![RuntimeNodeId::new(index)];
            elements.push(control);
        }
        let reader = elements.last_mut().unwrap();
        reader.kind = SceneElementKind::Button {
            label: "Table".into(),
        };
        reader.binding.as_mut().unwrap().capability = InteractionCapability::BrowseContent;
        reader.binding.as_mut().unwrap().default_intent = UiIntent::BeginRead;
        root.children = vec![];
        let scene = TuiScene::new("Workspace".into(), &root, elements);
        let before = scene.elements.clone();
        let mut region = spatial_region(
            0,
            "Workspace",
            RegionPresentationKind::WorkspacePane,
            crate::transcompile::LayoutDemand::Supporting,
        );
        region.presentation.source_nodes = (1..=9).map(RuntimeNodeId::new).collect();
        let plan = TuiLayoutPlan {
            root: LayoutNode::Leaf(region.id),
            regions: vec![region],
            topology: Default::default(),
            generation: ApplicationGenerationId(1),
            geometry_trust: GeometryTrust::Unavailable,
            composition: CompositionKind::ContentDominant,
        };
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        let mut hits = vec![];
        terminal
            .draw(|frame| {
                render_spatial_node(
                    frame,
                    frame.area(),
                    &RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: Some(SceneElementId::new(9)),
                        scroll_offset: 0,
                        status: "",
                        application_available: true,
                        external_text_handler_available: false,
                        edit_session: None,
                        palette: None,
                        choice: None,
                        content: None,
                        spatial: Some(&plan),
                        active_region: None,
                        inline_content: None,
                    },
                    &plan,
                    &plan.root,
                    &mut hits,
                );
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Table"));
        assert!(
            hits.iter()
                .any(|hit| hit.scene_id == SceneElementId::new(9))
        );
        assert_eq!(scene.elements, before);
        terminal
            .draw(|frame| {
                hits = render(
                    frame,
                    RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: Some(SceneElementId::new(9)),
                        scroll_offset: 0,
                        status: "",
                        application_available: true,
                        external_text_handler_available: false,
                        edit_session: None,
                        palette: Some(PaletteRender {
                            query: "",
                            entries: &[],
                            selected: 0,
                            all_scopes: false,
                            scope_switchable: false,
                            detail_scroll: 0,
                        }),
                        choice: None,
                        content: None,
                        spatial: Some(&plan),
                        active_region: None,
                        inline_content: None,
                    },
                );
            })
            .unwrap();
        assert!(
            hits.is_empty(),
            "palette must not expose background hit targets"
        );
    }

    #[test]
    fn long_compact_field_keeps_later_action_visible_and_exactly_bound() {
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/root"),
            index_in_parent: None,
            role: SemanticRole::Window,
            name: None,
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: Default::default(),
        };
        let field = element(SceneElementKind::Field {
            label: "Query".into(),
            display: "long public value ".repeat(30),
            input_kind: crate::semantic::TextInputKind::Plain,
        });
        let mut button = element(SceneElementKind::Button {
            label: "Apply".into(),
        });
        button.id = SceneElementId::new(10);
        let mut heading = element(SceneElementKind::Group {
            label: "Navigation heading".into(),
        });
        heading.binding = None;
        let mut unavailable = element(SceneElementKind::Button {
            label: "Unavailable control".into(),
        });
        unavailable.binding = None;
        let scene = TuiScene::new(
            "Window".into(),
            &root,
            vec![heading, unavailable, field, button],
        );
        let before = scene.elements.clone();
        let region = spatial_region(
            0,
            "Controls",
            RegionPresentationKind::InputSurface,
            crate::transcompile::LayoutDemand::Compact,
        );
        let context = RenderContext {
            scene: &scene,
            interaction_scope: None,
            focused: Some(SceneElementId::new(9)),
            scroll_offset: 0,
            status: "",
            application_available: true,
            external_text_handler_available: false,
            edit_session: None,
            palette: None,
            choice: None,
            content: None,
            spatial: None,
            active_region: None,
            inline_content: None,
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        let mut hits = vec![];
        terminal
            .draw(|frame| render_compact_surface(frame, frame.area(), &context, &region, &mut hits))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Query"));
        assert!(text.contains("Apply"));
        assert!(!text.contains("Navigation heading"));
        for id in [9, 10] {
            assert!(
                hits.iter()
                    .any(|hit| hit.scene_id == SceneElementId::new(id))
            );
        }
        assert_eq!(scene.elements, before);
    }

    #[test]
    fn same_named_compact_contexts_keep_separate_rows_and_bindings() {
        let mut command = spatial_region(
            0,
            "Same context",
            RegionPresentationKind::CommandBar,
            crate::transcompile::LayoutDemand::Compact,
        );
        command.source_nodes = vec![RuntimeNodeId::new(1)];
        let mut modal = command.clone();
        modal.semantic_kind = SemanticRegionKind::Dialog;
        assert!(!context_summary(&modal));
        let mut tabs = spatial_region(
            1,
            "Same context",
            RegionPresentationKind::Navigation,
            crate::transcompile::LayoutDemand::Minimal,
        );
        tabs.kind = SpatialRegionKind::TabStrip;
        let plan = TuiLayoutPlan {
            root: LayoutNode::Stack(vec![
                LayoutNode::Leaf(command.id),
                LayoutNode::Leaf(tabs.id),
            ]),
            regions: vec![command, tabs],
            topology: Default::default(),
            generation: ApplicationGenerationId(1),
            geometry_trust: GeometryTrust::Unavailable,
            composition: CompositionKind::ContentDominant,
        };
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", "/root"),
            index_in_parent: None,
            role: SemanticRole::Window,
            name: Some("App".into()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: Default::default(),
        };
        let mut first = element(SceneElementKind::Button {
            label: "One".into(),
        });
        first.sources = vec![RuntimeNodeId::new(1)];
        let mut second = first.clone();
        second.id = SceneElementId::new(10);
        second.sources = vec![RuntimeNodeId::new(2)];
        second.binding.as_mut().unwrap().runtime_id = RuntimeNodeId::new(2);
        second.binding.as_mut().unwrap().backend_locator = BackendLocator::new(":1.2", "/second");
        let scene = TuiScene::new("App".into(), &root, vec![first, second]);
        let mut terminal = Terminal::new(TestBackend::new(80, 4)).unwrap();
        let mut hits = vec![];
        terminal
            .draw(|frame| {
                render_spatial_node(
                    frame,
                    frame.area(),
                    &RenderContext {
                        scene: &scene,
                        interaction_scope: None,
                        focused: None,
                        scroll_offset: 0,
                        status: "",
                        application_available: true,
                        external_text_handler_available: false,
                        edit_session: None,
                        palette: None,
                        choice: None,
                        content: None,
                        spatial: Some(&plan),
                        active_region: None,
                        inline_content: None,
                    },
                    &plan,
                    &plan.root,
                    &mut hits,
                )
            })
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].scene_id, hits[1].scene_id);
        assert_ne!(hits[0].rect.y, hits[1].rect.y);
        assert_eq!(
            stack_constraint(&plan, &LayoutNode::Leaf(SpatialRegionId(0)), &scene, 80),
            Constraint::Length(1)
        );
    }

    #[test]
    fn non_expanding_graphical_composition_has_bounded_terminal_height() {
        let mut graphical = spatial_region(
            0,
            "Graphical Content",
            RegionPresentationKind::GraphicalPlaceholder,
            crate::transcompile::LayoutDemand::Supporting,
        );
        graphical.presentation.meaningful_items = 1;
        let controls = spatial_region(
            1,
            "Controls",
            RegionPresentationKind::ControlGroup,
            crate::transcompile::LayoutDemand::Compact,
        );
        let plan = TuiLayoutPlan {
            root: LayoutNode::VerticalSplit {
                children: vec![
                    LayoutNode::Leaf(graphical.id),
                    LayoutNode::Leaf(controls.id),
                ],
                weights: vec![3, 2],
            },
            regions: vec![graphical, controls],
            topology: crate::transcompile::SpatialTopology::default(),
            generation: ApplicationGenerationId(1),
            geometry_trust: GeometryTrust::Unavailable,
            composition: CompositionKind::ContentDominant,
        };
        let area =
            bounded_non_expanding_composition_area(&plan, &plan.root, Rect::new(0, 0, 100, 50));
        assert_eq!(area.height, 17);
        assert_eq!(area.y, 0);
    }
}
