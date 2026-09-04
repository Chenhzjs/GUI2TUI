use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    content::{ContentBlockKind, ContentSearchResult, ReaderBlock, SearchProgress, SearchState},
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
    pub focused: Option<SceneElementId>,
    pub scroll_offset: u16,
    pub status: &'a str,
    pub application_available: bool,
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
    let navigator = context
        .spatial
        .map(|plan| RegionNavigator::derive(plan, context.scene));
    let hints = if context.edit_session.is_some() {
        "? Help · Enter Apply · Esc Cancel"
    } else if context.choice.is_some() {
        "? Help · ↑/↓ Select · Enter Apply · Esc Cancel"
    } else if context.palette.is_some() {
        "? Help · F2 Scope · Enter Use · Esc Back"
    } else if let Some(content) = &context.content {
        match content.mode {
            ContentViewMode::Reader => "? Help · j/k Scroll · / Search · o Outline · Esc Back",
            ContentViewMode::Search => "? Help · Ctrl-F Search more · Enter Go · Esc Back",
            ContentViewMode::Table => "? Help · Arrows Cells · Esc Back",
            ContentViewMode::Outline => "? Help · ↑/↓ Headings · Enter Go · Esc Back",
            ContentViewMode::VirtualCollection => "? Help · ↑/↓ Items · Esc Back",
        }
    } else if navigator
        .as_ref()
        .is_some_and(RegionNavigator::has_subregions)
    {
        "? Help · F6 Region · Ctrl+Tab Pane · Tab Control · Enter Use · : Commands"
    } else {
        "? Help · F6 Region · Tab Control · Enter Use · : Commands"
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
    if let Some(palette) = context.palette {
        render_palette(frame, content_area, palette);
    }
    if let Some(choice) = context.choice {
        render_choice(frame, content_area, choice);
    }
    if let Some(content) = context.content {
        render_content(frame, content_area, content);
    }
    let footer = if normal_footer_status(context.status) {
        format!("{hints} · {}", context.status)
    } else {
        hints.to_owned()
    };
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
        let height = if navigator.has_subregions() { 4 } else { 3 };
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
    let title = if hierarchical {
        " Regions — F6 major · Ctrl+Tab pane "
    } else {
        " Regions — F6 switch "
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
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
            if hierarchical { "Major" } else { "Region" },
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
                "Pane",
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
    let mut used = prefix.chars().count();
    for offset in 0..items.len() {
        let index = (selected + offset) % items.len();
        let item = &items[index];
        let suffix = if item.empty { " · empty" } else { "" };
        let text = if index == selected {
            format!("[{}{}]", item.title, suffix)
        } else {
            format!("{}{}", item.title, suffix)
        };
        let separator = usize::from(offset > 0) * 3;
        let text_width = text.chars().count();
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
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default()
        };
        spans.push(Span::styled(text, style));
        used = used.saturating_add(text_width);
    }
    Line::from(spans)
}

fn normal_footer_status(status: &str) -> bool {
    !status.is_empty()
        && !status.starts_with("Loaded ")
        && !status.starts_with("Live update")
        && !status.starts_with("Replayed bootstrap events")
        && !status.starts_with("Full refresh fallback:")
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
    Rect::new(
        area.x,
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        area.width,
        height,
    )
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
                crate::transcompile::LayoutDemand::Compact => Some(4),
                crate::transcompile::LayoutDemand::Minimal => Some(3),
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
    if area.width < 4 || area.height < 2 {
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
                .map(|child| stack_constraint(plan, child))
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
            let constraints = weighted_constraints(plan, &children, weights, stack_constraint);
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

fn stack_constraint(plan: &TuiLayoutPlan, node: &LayoutNode) -> Constraint {
    let region = region_for_node(plan, node);
    if region.is_some_and(|region| {
        region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && region.demand == crate::transcompile::LayoutDemand::Supporting
    }) {
        return Constraint::Length(14);
    }
    match region.map(|region| region.demand) {
        Some(crate::transcompile::LayoutDemand::Expand) => Constraint::Min(8),
        Some(crate::transcompile::LayoutDemand::Supporting) => Constraint::Min(5),
        Some(crate::transcompile::LayoutDemand::Compact) => Constraint::Length(4),
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

fn render_spatial_leaf(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &RenderContext<'_>,
    region: &SpatialRegion,
    hit_regions: &mut Vec<HitRegion>,
) {
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
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if region.kind == crate::transcompile::SpatialRegionKind::PrimaryContent
        && matches!(
            region.presentation.kind,
            RegionPresentationKind::InlineContent | RegionPresentationKind::GraphicalPlaceholder
        )
    {
        if let Some(inline) = context.inline_content.as_ref() {
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
                if let Some(interaction) = interaction(element) {
                    hit_regions.push(HitRegion {
                        scene_id: element.id,
                        rect: Rect::new(inner.x, inner.y, inner.width, 1),
                        interaction,
                    });
                }
            }
            let status = format!("{} lines{}", inline.total_lines, suffix);
            frame.render_widget(
                Paragraph::new(lines.join("\n"))
                    .block(Block::default().title(format!(" {} — {} ", inline.title, status)))
                    .wrap(Wrap { trim: true })
                    .scroll((context.scroll_offset, 0)),
                inner,
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
    let hidden = elements.len().saturating_sub(contextual_limit);
    let mut rendered = 0_u16;
    for element in elements.into_iter().take(contextual_limit) {
        if rendered >= inner.height {
            break;
        }
        let focused = context.focused == Some(element.id);
        let lines = element_lines_for_width(element, focused, context.edit_session, inner.width);
        for line in lines {
            if rendered >= inner.height {
                break;
            }
            frame.render_widget(
                Paragraph::new(line),
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
            RegionPresentationKind::InputSurface => "— unavailable · ro",
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
                    SceneElementKind::Unsupported { .. } | SceneElementKind::CommandHeader { .. }
                )
        })
        .collect::<Vec<_>>();
    elements.sort_by_key(|element| context.focused != Some(element.id));
    let mut x = area.x;
    let mut shown = 0_usize;
    for element in &elements {
        let focused = context.focused == Some(element.id);
        let label = format!("[{}{}]", if focused { "> " } else { "" }, element.label());
        let width = label.chars().count().min(usize::from(u16::MAX)) as u16;
        let reserve = if shown + 1 < elements.len() { 9 } else { 0 };
        if x.saturating_add(width).saturating_add(reserve) > area.right() {
            break;
        }
        let style = if focused {
            Style::default().add_modifier(Modifier::REVERSED)
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
            Paragraph::new("[: More…]"),
            Rect::new(x.min(area.right().saturating_sub(1)), area.y, 9, 1),
        );
    } else if shown == 0 {
        let label = match region.presentation.kind {
            RegionPresentationKind::CommandBar => ": Commands",
            RegionPresentationKind::InputSurface => "— unavailable · ro",
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
        SceneElementKind::Value { .. } => Some(HitInteraction::Focus),
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
            " · ro"
        } else {
            ""
        };
    match &element.kind {
        SceneElementKind::Text { text } | SceneElementKind::Status { text } => {
            vec![format!("  {text}")]
        }
        SceneElementKind::Hint { text } => vec![format!("    Hint: {text}")],
        SceneElementKind::Error { text } => vec![format!("    Error: {text}")],
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
            if *external_edit {
                "    [ Enter: Read document | e: Edit with configured handler ]".to_owned()
            } else {
                "    [ Enter: Read document ]".to_owned()
            },
            "    o Outline | / Content search".to_owned(),
        ],
        SceneElementKind::SelectionItem { label, selected } => vec![format!(
            "{marker}{} {label}{unavailable}",
            if *selected { "*" } else { "•" }
        )],
        SceneElementKind::Command { path } => vec![format!("{marker}{path}{unavailable}")],
        SceneElementKind::OpaqueContent { label, dimensions } => vec![
            format!("  {label}"),
            dimensions.map_or_else(
                || "  [fidelity-required content]".to_owned(),
                |(w, h)| format!("  [fidelity-required content: {w}×{h} GUI pixels]"),
            ),
        ],
        SceneElementKind::Unsupported { label } => vec![format!("  <Unsupported: {label}>")],
    }
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
            vec!["  [ ] Enable feature · ro"]
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
            title: "Document".into(),
            lines,
            total_lines: 20,
            partial: false,
        };
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    RenderContext {
                        scene: &scene,
                        focused: None,
                        scroll_offset: 3,
                        status: "ready",
                        application_available: true,
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
        assert_eq!(area.height, 18);
        assert!(area.y > 0);
    }
}
