//! Presentation-only spatial evidence and layout planning.
//!
//! Geometry is deliberately kept out of [`SemanticNode`]'s identity and
//! capability contracts.  This module consumes semantic regions plus a bounded
//! sidecar and produces a terminal-independent layout tree.  No terminal
//! coordinates are stored here and no operation is created by spatial analysis.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    time::{Duration, Instant},
};

use crate::{
    content::{ContentBlockKind, ContentCompleteness, ContentRuntime, TextContentState},
    runtime::ApplicationGenerationId,
    semantic::{
        Geometry, RuntimeNodeId, SemanticCapability, SemanticNode, SemanticRole, SemanticState,
        TextInputKind,
    },
};

use super::{
    region::{RegionId, SemanticRegion, SemanticRegionKind},
    scene::{SceneElementKind, TuiScene},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CoordinateSpace {
    Screen,
    Window,
    Parent,
    #[default]
    Unknown,
}

impl fmt::Display for CoordinateSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeometryTrust {
    #[default]
    Unavailable,
    Partial,
    Consistent,
    Inconsistent,
}

impl fmt::Display for GeometryTrust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl SpatialBounds {
    pub fn from_geometry(g: &Geometry) -> Self {
        Self {
            x: g.x,
            y: g.y,
            width: g.width,
            height: g.height,
        }
    }
    pub fn area(self) -> i64 {
        i64::from(self.width.max(0)) * i64::from(self.height.max(0))
    }
    pub fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }
    pub fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }
    pub fn center(self) -> (i32, i32) {
        (
            self.x.saturating_add(self.width / 2),
            self.y.saturating_add(self.height / 2),
        )
    }
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
    pub fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpatialEvidence {
    pub runtime_id: RuntimeNodeId,
    pub bounds: Option<SpatialBounds>,
    pub coordinate_space: CoordinateSpace,
    pub visible: bool,
    pub showing: bool,
    pub layer: Option<i32>,
    pub trust: GeometryTrust,
    pub generation: ApplicationGenerationId,
    pub provenance: String,
}

impl SpatialEvidence {
    pub fn unavailable(runtime_id: RuntimeNodeId, generation: ApplicationGenerationId) -> Self {
        Self {
            runtime_id,
            bounds: None,
            coordinate_space: CoordinateSpace::Unknown,
            visible: true,
            showing: true,
            layer: None,
            trust: GeometryTrust::Unavailable,
            generation,
            provenance: "not-probed".into(),
        }
    }
    pub fn from_geometry(
        runtime_id: RuntimeNodeId,
        geometry: Option<&Geometry>,
        generation: ApplicationGenerationId,
    ) -> Self {
        let Some(geometry) = geometry else {
            return Self::unavailable(runtime_id, generation);
        };
        let bounds = SpatialBounds::from_geometry(geometry);
        let valid = bounds.width > 0
            && bounds.height > 0
            // AT-SPI uses INT_MIN for an unrealized/off-screen component.
            && bounds.x != i32::MIN
            && bounds.y != i32::MIN
            && bounds.x.abs_diff(0) < 1_000_000_000
            && bounds.y.abs_diff(0) < 1_000_000_000;
        // The AT-SPI backend requests CoordType::Screen.  Keeping that fact in
        // the sidecar makes comparisons explicit while still allowing other
        // producers to override it through `with_space`.
        Self {
            runtime_id,
            bounds: valid.then_some(bounds),
            coordinate_space: CoordinateSpace::Screen,
            visible: true,
            showing: true,
            layer: None,
            trust: if valid {
                GeometryTrust::Consistent
            } else {
                GeometryTrust::Inconsistent
            },
            generation,
            provenance: "atspi-component-screen-extents".into(),
        }
    }
    pub fn with_space(mut self, space: CoordinateSpace) -> Self {
        self.coordinate_space = space;
        if self.bounds.is_some() && self.trust == GeometryTrust::Partial {
            self.trust = GeometryTrust::Consistent;
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialProbeBudget {
    pub max_candidates: usize,
}

impl Default for SpatialProbeBudget {
    fn default() -> Self {
        Self {
            max_candidates: 128,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpatialProbeMetrics {
    pub nodes: usize,
    pub candidate_nodes: usize,
    pub geometry_requests: usize,
    pub geometry_successes: usize,
    pub geometry_failures: usize,
    pub geometry_rejected: usize,
    pub cache_hits: usize,
    pub elapsed: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpatialEvidenceIndex {
    pub generation: ApplicationGenerationId,
    pub entries: HashMap<RuntimeNodeId, SpatialEvidence>,
    pub metrics: SpatialProbeMetrics,
}

impl SpatialEvidenceIndex {
    /// Build a bounded sidecar from geometry already present in the semantic
    /// snapshot.  A backend may populate the same type with explicit screen or
    /// window coordinates; this constructor never performs an RPC.
    pub fn from_tree(
        root: &SemanticNode,
        generation: ApplicationGenerationId,
        budget: SpatialProbeBudget,
    ) -> Self {
        let started = Instant::now();
        let mut all = Vec::new();
        collect_node_list_with_depth(root, 0, &mut all);
        let mut ranked: Vec<_> = all
            .iter()
            .filter(|(node, depth)| {
                is_candidate(node)
                    || is_high_value_spatial_anchor(node)
                    || (*depth <= 2 && is_spatial_anchor(node))
            })
            .collect();
        ranked.sort_by_key(|(node, depth)| std::cmp::Reverse(candidate_priority(node, *depth)));
        ranked.truncate(budget.max_candidates);
        let mut entries = HashMap::new();
        let mut metrics = SpatialProbeMetrics {
            nodes: all.len(),
            candidate_nodes: ranked.len(),
            ..Default::default()
        };
        for (node, _) in ranked {
            if node.debug.geometry.is_some() {
                metrics.cache_hits += 1;
            }
            let evidence = SpatialEvidence::from_geometry(
                node.runtime_id,
                node.debug.geometry.as_ref(),
                generation,
            );
            match evidence.trust {
                GeometryTrust::Unavailable => metrics.geometry_failures += 1,
                GeometryTrust::Inconsistent => metrics.geometry_rejected += 1,
                _ => metrics.geometry_successes += 1,
            }
            entries.insert(node.runtime_id, evidence);
        }
        metrics.elapsed = started.elapsed();
        Self {
            generation,
            entries,
            metrics,
        }
    }

    /// Enrich a bounded semantic candidate set with real AT-SPI Component
    /// extents. Cached geometry is reused; missing geometry is queried with a
    /// small fixed concurrency limit, never once per semantic node.
    pub async fn from_backend(
        root: &SemanticNode,
        generation: ApplicationGenerationId,
        budget: SpatialProbeBudget,
        backend: &crate::backend::AtspiBackend,
    ) -> Self {
        let started = Instant::now();
        let mut all = Vec::new();
        collect_node_list_with_depth(root, 0, &mut all);
        let mut ranked: Vec<_> = all
            .iter()
            .filter(|(node, depth)| {
                is_candidate(node)
                    || is_high_value_spatial_anchor(node)
                    || (*depth <= 2 && is_spatial_anchor(node))
            })
            .collect();
        ranked.sort_by_key(|(node, depth)| std::cmp::Reverse(candidate_priority(node, *depth)));
        ranked.truncate(budget.max_candidates);
        let candidate_nodes = ranked.len();
        let mut entries = HashMap::new();
        let mut metrics = SpatialProbeMetrics {
            nodes: all.len(),
            candidate_nodes,
            ..Default::default()
        };
        let mut pending = Vec::new();
        for (node, _) in ranked {
            if node.debug.geometry.is_some() {
                metrics.cache_hits += 1;
                let evidence = SpatialEvidence::from_geometry(
                    node.runtime_id,
                    node.debug.geometry.as_ref(),
                    generation,
                );
                match evidence.trust {
                    GeometryTrust::Unavailable => metrics.geometry_failures += 1,
                    GeometryTrust::Inconsistent => metrics.geometry_rejected += 1,
                    _ => metrics.geometry_successes += 1,
                }
                entries.insert(node.runtime_id, evidence);
            } else {
                pending.push((node.runtime_id, node.backend_locator.clone()));
            }
        }

        let mut pending = pending.into_iter();
        let mut probes = tokio::task::JoinSet::new();
        const MAX_IN_FLIGHT: usize = 8;
        for _ in 0..MAX_IN_FLIGHT {
            let Some((runtime_id, locator)) = pending.next() else {
                break;
            };
            let backend = backend.clone();
            probes.spawn(async move {
                let geometry = backend.probe_geometry(&locator).await.ok().flatten();
                (runtime_id, geometry)
            });
        }
        while let Some(result) = probes.join_next().await {
            metrics.geometry_requests += 1;
            if let Ok((runtime_id, geometry)) = result {
                let evidence =
                    SpatialEvidence::from_geometry(runtime_id, geometry.as_ref(), generation);
                match evidence.trust {
                    GeometryTrust::Unavailable => metrics.geometry_failures += 1,
                    GeometryTrust::Inconsistent => metrics.geometry_rejected += 1,
                    _ => metrics.geometry_successes += 1,
                }
                entries.insert(runtime_id, evidence);
            } else {
                metrics.geometry_failures += 1;
            }
            if let Some((runtime_id, locator)) = pending.next() {
                let backend = backend.clone();
                probes.spawn(async move {
                    let geometry = backend.probe_geometry(&locator).await.ok().flatten();
                    (runtime_id, geometry)
                });
            }
        }
        metrics.elapsed = started.elapsed();
        Self {
            generation,
            entries,
            metrics,
        }
    }

    pub fn insert(&mut self, evidence: SpatialEvidence) {
        if evidence.generation == self.generation {
            self.entries.insert(evidence.runtime_id, evidence);
        }
    }
    pub fn get(&self, id: RuntimeNodeId) -> Option<&SpatialEvidence> {
        self.entries
            .get(&id)
            .filter(|e| e.generation == self.generation)
    }
    pub fn trusted_bounds(&self, id: RuntimeNodeId) -> Option<SpatialBounds> {
        self.get(id)
            .filter(|e| {
                e.visible
                    && e.showing
                    && matches!(e.trust, GeometryTrust::Consistent | GeometryTrust::Partial)
            })
            .and_then(|e| e.bounds)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpatialRegionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpatialRegionKind {
    PrimaryContent,
    Navigation,
    Toolbar,
    TabStrip,
    Sidebar,
    Auxiliary,
    Status,
    Dialog,
    Overlay,
    Split,
    Structural,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PresentationPriority {
    Primary,
    Secondary,
    Auxiliary,
    Structural,
    HiddenByDefault,
}

/// Terminal-native sizing importance.  This is deliberately independent of
/// GUI pixel area; the renderer uses it to allocate dominant/supporting/
/// compact space from the layout plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutImportance {
    Dominant,
    Supporting,
    Compact,
    Structural,
}

/// Whether a semantic presentation surface must remain represented.  This is
/// deliberately orthogonal to sizing: a persistent surface may still collapse
/// to a compact selector when terminal space is scarce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PresentationObligation {
    Persistent,
    Contextual,
    Discoverable,
    Structural,
    DiagnosticOnly,
}

/// Terminal-native space requested by a presentation payload.  Values express
/// utility, never a scaled GUI pixel ratio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LayoutDemand {
    Expand,
    Supporting,
    Compact,
    Minimal,
    Hidden,
}

/// How strongly a presentation surface should remain directly visible in the
/// normal scene. This is independent from semantic preservation and terminal
/// space demand: a collapsed surface can remain reachable while still failing
/// its direct-visibility contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VisibilityGuarantee {
    Pinned,
    PreferDirect,
    Collapsible,
    DiscoverableOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InteractionPurpose {
    Search,
    Navigate,
    Filter,
    Input,
    Edit,
    Select,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopologyRelationKind {
    Contains,
    Inside,
    Above,
    Below,
    LeftOf,
    RightOf,
    Overlaps,
    AdjacentHorizontal,
    AdjacentVertical,
    AlignedTop,
    AlignedBottom,
    AlignedLeft,
    AlignedRight,
    SameHorizontalBand,
    SameVerticalBand,
    PeripheralTo,
    DominatesArea,
}

/// Fixed-point normalized geometry (0..=1000) used only for inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NormalizedBounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialRelation {
    pub from: SpatialRegionId,
    pub to: SpatialRegionId,
    pub kind: TopologyRelationKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpatialTopology {
    pub normalized: HashMap<SpatialRegionId, NormalizedBounds>,
    pub relations: Vec<SpatialRelation>,
    pub anchors: usize,
    pub comparable_pairs: usize,
}

impl SpatialTopology {
    pub fn relates(
        &self,
        from: SpatialRegionId,
        to: SpatialRegionId,
        kind: TopologyRelationKind,
    ) -> bool {
        self.relations
            .iter()
            .any(|relation| relation.from == from && relation.to == to && relation.kind == kind)
    }

    pub fn infer(regions: &[SpatialRegion]) -> Self {
        let mut anchors = regions
            .iter()
            .filter(|region| {
                region.bounds.is_some() && region.coordinate_space != CoordinateSpace::Unknown
            })
            .collect::<Vec<_>>();
        anchors.sort_by_key(|region| region.id);
        let mut containers: HashMap<CoordinateSpace, SpatialBounds> = HashMap::new();
        for region in &anchors {
            let bounds = region.bounds.expect("filtered bounds");
            containers
                .entry(region.coordinate_space)
                .and_modify(|current| {
                    if bounds.area() > current.area() {
                        *current = bounds;
                    }
                })
                .or_insert(bounds);
        }
        let mut normalized = HashMap::new();
        for region in &anchors {
            let bounds = region.bounds.expect("filtered bounds");
            if let Some(container) = containers.get(&region.coordinate_space)
                && let Some(value) = normalize_bounds(bounds, *container)
            {
                normalized.insert(region.id, value);
            }
        }
        let mut relations = Vec::new();
        let mut comparable_pairs = 0;
        for (index, left) in anchors.iter().enumerate() {
            for right in anchors.iter().skip(index + 1) {
                if left.coordinate_space != right.coordinate_space {
                    continue;
                }
                comparable_pairs += 1;
                let a = left.bounds.expect("filtered bounds");
                let b = right.bounds.expect("filtered bounds");
                infer_pair_relations(left.id, a, right.id, b, &normalized, &mut relations);
            }
        }
        Self {
            normalized,
            relations,
            anchors: anchors.len(),
            comparable_pairs,
        }
    }
}

fn normalize_bounds(bounds: SpatialBounds, container: SpatialBounds) -> Option<NormalizedBounds> {
    if container.width <= 0 || container.height <= 0 || !container.contains(bounds) {
        return None;
    }
    let scale = |value: i32, origin: i32, extent: i32| {
        let relative = i64::from(value.saturating_sub(origin)).max(0);
        ((relative.saturating_mul(1000) / i64::from(extent)).clamp(0, 1000)) as u16
    };
    Some(NormalizedBounds {
        x: scale(bounds.x, container.x, container.width),
        y: scale(bounds.y, container.y, container.height),
        width: scale(bounds.width, 0, container.width),
        height: scale(bounds.height, 0, container.height),
    })
}

fn push_relation(
    output: &mut Vec<SpatialRelation>,
    from: SpatialRegionId,
    to: SpatialRegionId,
    kind: TopologyRelationKind,
) {
    output.push(SpatialRelation { from, to, kind });
}

fn infer_pair_relations(
    a_id: SpatialRegionId,
    a: SpatialBounds,
    b_id: SpatialRegionId,
    b: SpatialBounds,
    normalized: &HashMap<SpatialRegionId, NormalizedBounds>,
    output: &mut Vec<SpatialRelation>,
) {
    if a.contains(b) {
        push_relation(output, a_id, b_id, TopologyRelationKind::Contains);
        push_relation(output, b_id, a_id, TopologyRelationKind::Inside);
    } else if b.contains(a) {
        push_relation(output, b_id, a_id, TopologyRelationKind::Contains);
        push_relation(output, a_id, b_id, TopologyRelationKind::Inside);
    } else if a.intersects(b) {
        push_relation(output, a_id, b_id, TopologyRelationKind::Overlaps);
        push_relation(output, b_id, a_id, TopologyRelationKind::Overlaps);
    }

    let horizontal_overlap = a.x < b.right() && b.x < a.right();
    let vertical_overlap = a.y < b.bottom() && b.y < a.bottom();
    if a.right() <= b.x {
        push_relation(output, a_id, b_id, TopologyRelationKind::LeftOf);
        push_relation(output, b_id, a_id, TopologyRelationKind::RightOf);
    } else if b.right() <= a.x {
        push_relation(output, b_id, a_id, TopologyRelationKind::LeftOf);
        push_relation(output, a_id, b_id, TopologyRelationKind::RightOf);
    }
    if a.bottom() <= b.y {
        push_relation(output, a_id, b_id, TopologyRelationKind::Above);
        push_relation(output, b_id, a_id, TopologyRelationKind::Below);
    } else if b.bottom() <= a.y {
        push_relation(output, b_id, a_id, TopologyRelationKind::Above);
        push_relation(output, a_id, b_id, TopologyRelationKind::Below);
    }

    let Some(an) = normalized.get(&a_id) else {
        return;
    };
    let Some(bn) = normalized.get(&b_id) else {
        return;
    };
    let tolerance = 30_u16;
    let abs = |left: u16, right: u16| left.abs_diff(right);
    if abs(an.y, bn.y) <= tolerance {
        push_relation(output, a_id, b_id, TopologyRelationKind::AlignedTop);
        push_relation(output, b_id, a_id, TopologyRelationKind::AlignedTop);
    }
    if abs(
        an.y.saturating_add(an.height),
        bn.y.saturating_add(bn.height),
    ) <= tolerance
    {
        push_relation(output, a_id, b_id, TopologyRelationKind::AlignedBottom);
        push_relation(output, b_id, a_id, TopologyRelationKind::AlignedBottom);
    }
    if abs(an.x, bn.x) <= tolerance {
        push_relation(output, a_id, b_id, TopologyRelationKind::AlignedLeft);
        push_relation(output, b_id, a_id, TopologyRelationKind::AlignedLeft);
    }
    if abs(an.x.saturating_add(an.width), bn.x.saturating_add(bn.width)) <= tolerance {
        push_relation(output, a_id, b_id, TopologyRelationKind::AlignedRight);
        push_relation(output, b_id, a_id, TopologyRelationKind::AlignedRight);
    }
    if abs(
        an.y.saturating_add(an.height / 2),
        bn.y.saturating_add(bn.height / 2),
    ) <= 100
        && vertical_overlap
    {
        push_relation(output, a_id, b_id, TopologyRelationKind::SameHorizontalBand);
        push_relation(output, b_id, a_id, TopologyRelationKind::SameHorizontalBand);
    }
    if abs(
        an.x.saturating_add(an.width / 2),
        bn.x.saturating_add(bn.width / 2),
    ) <= 100
        && horizontal_overlap
    {
        push_relation(output, a_id, b_id, TopologyRelationKind::SameVerticalBand);
        push_relation(output, b_id, a_id, TopologyRelationKind::SameVerticalBand);
    }
    let horizontal_gap = if an.x.saturating_add(an.width) <= bn.x {
        bn.x - an.x.saturating_add(an.width)
    } else if bn.x.saturating_add(bn.width) <= an.x {
        an.x - bn.x.saturating_add(bn.width)
    } else {
        0
    };
    let vertical_gap = if an.y.saturating_add(an.height) <= bn.y {
        bn.y - an.y.saturating_add(an.height)
    } else if bn.y.saturating_add(bn.height) <= an.y {
        an.y - bn.y.saturating_add(bn.height)
    } else {
        0
    };
    let horizontally_separate = a.right() <= b.x || b.right() <= a.x;
    let vertically_separate = a.bottom() <= b.y || b.bottom() <= a.y;
    if vertical_overlap && horizontally_separate && horizontal_gap <= 30 {
        push_relation(output, a_id, b_id, TopologyRelationKind::AdjacentHorizontal);
        push_relation(output, b_id, a_id, TopologyRelationKind::AdjacentHorizontal);
    }
    if horizontal_overlap && vertically_separate && vertical_gap <= 30 {
        push_relation(output, a_id, b_id, TopologyRelationKind::AdjacentVertical);
        push_relation(output, b_id, a_id, TopologyRelationKind::AdjacentVertical);
    }
    let a_peripheral = an.x <= 100
        || an.y <= 100
        || an.x.saturating_add(an.width) >= 900
        || an.y.saturating_add(an.height) >= 900;
    let b_peripheral = bn.x <= 100
        || bn.y <= 100
        || bn.x.saturating_add(bn.width) >= 900
        || bn.y.saturating_add(bn.height) >= 900;
    if a_peripheral && a.area() < b.area() {
        push_relation(output, a_id, b_id, TopologyRelationKind::PeripheralTo);
    }
    if b_peripheral && b.area() < a.area() {
        push_relation(output, b_id, a_id, TopologyRelationKind::PeripheralTo);
    }
    if a.area().saturating_mul(2) >= b.area().saturating_mul(3) {
        push_relation(output, a_id, b_id, TopologyRelationKind::DominatesArea);
    }
    if b.area().saturating_mul(2) >= a.area().saturating_mul(3) {
        push_relation(output, b_id, a_id, TopologyRelationKind::DominatesArea);
    }
}

/// User-facing payload selected for a spatial region.  This is presentation
/// policy only: it neither changes semantic capabilities nor creates actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegionPresentationKind {
    InlineContent,
    InputSurface,
    Navigation,
    Form,
    ChoiceList,
    Table,
    CommandBar,
    GraphicalPlaceholder,
    Status,
    ControlGroup,
    WorkspacePane,
    CollapsedSummary,
    Structural,
    DiagnosticOnly,
    Empty,
}

impl RegionPresentationKind {
    fn is_meaningful(self) -> bool {
        !matches!(self, Self::Structural | Self::DiagnosticOnly | Self::Empty)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionPresentation {
    pub kind: RegionPresentationKind,
    pub title: String,
    pub source_regions: Vec<RegionId>,
    pub source_nodes: Vec<RuntimeNodeId>,
    pub meaningful_items: usize,
    pub dominant_eligible: bool,
    pub reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositionKind {
    ContentDominant,
    NavigationDetail,
    MultiPaneWorkspace,
    DialogForm,
    ControlSurface,
    FallbackStack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContentPresentationSummary {
    root: RuntimeNodeId,
    title: Option<String>,
    meaningful_blocks: usize,
    graphical_blocks: usize,
    total_blocks: usize,
    partial: bool,
}

/// Bounded, presentation-only facts derived from the existing content
/// architecture.  Keeping this as a sidecar avoids changing ContentBlock or
/// SemanticNode contracts merely to choose a layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionPresentationContext {
    content: HashMap<RuntimeNodeId, ContentPresentationSummary>,
}

impl RegionPresentationContext {
    pub fn from_content_runtime(runtime: &ContentRuntime) -> Self {
        let mut context = Self::default();
        for model in runtime.catalog().models() {
            let mut meaningful_blocks = 0;
            let mut graphical_blocks = 0;
            for block in model.blocks.iter() {
                if matches!(block.kind, ContentBlockKind::OpaqueContent(_)) {
                    graphical_blocks += 1;
                    continue;
                }
                if !is_user_content_kind(&block.kind) {
                    continue;
                }
                let text = runtime.displayed_block_text(model.root, block.id);
                let intrinsic = block.text.visible_text().is_some_and(is_meaningful_text);
                let runtime_loaded = text.as_deref().is_some_and(|text| {
                    is_meaningful_text(text)
                        && (block
                            .label
                            .as_deref()
                            .is_none_or(|label| label.trim() != text.trim())
                            || is_substantial_inline_text(text))
                });
                let substantial_label = block
                    .label
                    .as_deref()
                    .is_some_and(is_substantial_inline_text);
                if intrinsic || runtime_loaded || substantial_label {
                    meaningful_blocks += 1;
                }
            }
            context.content.insert(
                model.root,
                ContentPresentationSummary {
                    root: model.root,
                    title: model.metadata.title.clone(),
                    meaningful_blocks,
                    graphical_blocks,
                    total_blocks: model.blocks.len(),
                    partial: model.completeness != ContentCompleteness::Complete,
                },
            );
        }
        context
    }

    pub fn from_content_catalog(catalog: &crate::content::ContentCatalog) -> Self {
        let mut context = Self::default();
        for model in catalog.models() {
            let meaningful_blocks = model
                .blocks
                .iter()
                .filter(|block| {
                    is_user_content_kind(&block.kind)
                        && match &block.text {
                            TextContentState::Summary(text) | TextContentState::Loaded(text) => {
                                is_meaningful_text(text)
                            }
                            TextContentState::Unknown => block
                                .label
                                .as_deref()
                                .is_some_and(is_substantial_inline_text),
                            TextContentState::Unavailable => false,
                        }
                })
                .count();
            let graphical_blocks = model
                .blocks
                .iter()
                .filter(|block| matches!(block.kind, ContentBlockKind::OpaqueContent(_)))
                .count();
            context.content.insert(
                model.root,
                ContentPresentationSummary {
                    root: model.root,
                    title: model.metadata.title.clone(),
                    meaningful_blocks,
                    graphical_blocks,
                    total_blocks: model.blocks.len(),
                    partial: model.completeness != ContentCompleteness::Complete,
                },
            );
        }
        context
    }

    #[cfg(test)]
    fn with_content(mut self, root: RuntimeNodeId, title: &str, meaningful_blocks: usize) -> Self {
        self.content.insert(
            root,
            ContentPresentationSummary {
                root,
                title: Some(title.to_owned()),
                meaningful_blocks,
                graphical_blocks: 0,
                total_blocks: meaningful_blocks,
                partial: false,
            },
        );
        self
    }

    #[cfg(test)]
    fn with_mixed_content(
        mut self,
        root: RuntimeNodeId,
        title: &str,
        meaningful_blocks: usize,
        graphical_blocks: usize,
    ) -> Self {
        self.content.insert(
            root,
            ContentPresentationSummary {
                root,
                title: Some(title.to_owned()),
                meaningful_blocks,
                graphical_blocks,
                total_blocks: meaningful_blocks.saturating_add(graphical_blocks),
                partial: false,
            },
        );
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpatialRegion {
    pub id: SpatialRegionId,
    pub semantic_region: RegionId,
    pub source_nodes: Vec<RuntimeNodeId>,
    pub semantic_kind: SemanticRegionKind,
    pub kind: SpatialRegionKind,
    pub priority: PresentationPriority,
    pub importance: LayoutImportance,
    pub obligation: PresentationObligation,
    pub demand: LayoutDemand,
    pub visibility: VisibilityGuarantee,
    pub purpose: InteractionPurpose,
    pub bounds: Option<SpatialBounds>,
    pub coordinate_space: CoordinateSpace,
    pub presentation: RegionPresentation,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutNode {
    Leaf(SpatialRegionId),
    Stack(Vec<LayoutNode>),
    HorizontalSplit {
        children: Vec<LayoutNode>,
        weights: Vec<u16>,
    },
    VerticalSplit {
        children: Vec<LayoutNode>,
        weights: Vec<u16>,
    },
    Overlay {
        base: Box<LayoutNode>,
        overlays: Vec<LayoutNode>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TuiLayoutPlan {
    pub root: LayoutNode,
    pub regions: Vec<SpatialRegion>,
    pub topology: SpatialTopology,
    pub generation: ApplicationGenerationId,
    pub geometry_trust: GeometryTrust,
    pub composition: CompositionKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutMetrics {
    pub regions: usize,
    pub primary: usize,
    pub structural: usize,
    pub unplaced: usize,
    pub leaves: usize,
    pub surface_inference: Duration,
    pub topology_inference: Duration,
    pub composition_planning: Duration,
    pub layout_compilation: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutAnalysis {
    pub plan: TuiLayoutPlan,
    pub metrics: LayoutMetrics,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutReachabilityAudit {
    pub actionable_regions: usize,
    pub placed_regions: usize,
    pub unplaced: Vec<RegionId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalWidthClass {
    Wide,
    Medium,
    Narrow,
    Compact,
}

impl TerminalWidthClass {
    pub fn for_size(width: u16, region_count: usize) -> Self {
        // Thresholds follow generic minimum viable pane widths and surface
        // count; they are not tuned for an application identity.
        let pane = if region_count >= 4 { 34 } else { 30 };
        if width >= pane * 3 + 4 {
            Self::Wide
        } else if width >= pane * 2 + 2 {
            Self::Medium
        } else if width >= pane + 16 {
            Self::Narrow
        } else {
            Self::Compact
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResponsiveComposition {
    pub class: TerminalWidthClass,
    pub root: LayoutNode,
    pub represented: Vec<SpatialRegionId>,
    pub collapsed: Vec<SpatialRegionId>,
    pub forced_collapsed: Vec<SpatialRegionId>,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PresentationCoverageAudit {
    pub persistent: usize,
    pub represented: usize,
    pub collapsed_but_reachable: usize,
    pub discoverable: usize,
    pub structural_suppressed: usize,
    pub primary_represented: bool,
    pub persistent_inputs: usize,
    pub persistent_inputs_represented: usize,
    pub control_surfaces: usize,
    pub control_surfaces_represented: usize,
    pub tab_contexts: usize,
    pub tab_contexts_represented: usize,
    pub meaningful_statuses: usize,
    pub meaningful_statuses_represented: usize,
    pub command_surfaces_discoverable: usize,
    pub missing: Vec<SpatialRegionId>,
    pub pinned: usize,
    pub pinned_directly_represented: usize,
    pub pinned_forced_to_collapse: usize,
    pub pinned_improperly_collapsed: Vec<SpatialRegionId>,
}

pub fn infer_layout(
    analysis: &super::analyze::RegionAnalysis,
    root: &SemanticNode,
    evidence: &SpatialEvidenceIndex,
) -> LayoutAnalysis {
    infer_layout_with_presentations(analysis, root, evidence, None)
}

pub fn infer_layout_with_presentations(
    analysis: &super::analyze::RegionAnalysis,
    root: &SemanticNode,
    evidence: &SpatialEvidenceIndex,
    presentation_context: Option<&RegionPresentationContext>,
) -> LayoutAnalysis {
    let surface_started = Instant::now();
    let mut nodes = HashMap::new();
    collect_node_map(root, &mut nodes);
    let mut semantic = Vec::new();
    flatten_regions(&analysis.root, &mut semantic);
    let mut regions = Vec::with_capacity(semantic.len());
    for (index, region) in semantic.iter().enumerate() {
        let bounds = region
            .source_nodes
            .iter()
            .find_map(|id| evidence.trusted_bounds(*id));
        let coordinate_space = region
            .source_nodes
            .iter()
            .find_map(|id| evidence.get(*id).map(|item| item.coordinate_space))
            .unwrap_or(CoordinateSpace::Unknown);
        let source = region.source_nodes.first().and_then(|id| nodes.get(id));
        let structural = is_structural_region(region, source);
        let semantic_spatial_kind = kind_for_semantics(region.kind);
        let semantic_priority = priority_for_semantics(region.kind);
        let mut presentation =
            presentation_for_region(region, source.copied(), structural, presentation_context);
        if matches!(
            presentation.kind,
            RegionPresentationKind::DiagnosticOnly | RegionPresentationKind::Empty
        ) && let Some(input) = region
            .source_nodes
            .iter()
            .filter_map(|id| nodes.get(id).copied())
            .find(|node| is_presentable_single_line_input(node))
        {
            presentation.kind = RegionPresentationKind::ControlGroup;
            presentation.title = input
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("Input")
                .to_owned();
            presentation.meaningful_items = 1;
            presentation.dominant_eligible = false;
            presentation
                .reasons
                .push("related semantic sources contain a presentable single-line input".into());
        }
        let (obligation, demand, purpose) =
            initial_surface_policy(region, source.copied(), &presentation, structural);
        let presentation_hidden = matches!(
            presentation.kind,
            RegionPresentationKind::Structural
                | RegionPresentationKind::DiagnosticOnly
                | RegionPresentationKind::Empty
        );
        let presentation_priority = if structural {
            PresentationPriority::Structural
        } else if presentation_hidden {
            PresentationPriority::HiddenByDefault
        } else {
            semantic_priority
        };
        let mut spatial = SpatialRegion {
            id: SpatialRegionId(index as u64),
            semantic_region: region.id,
            source_nodes: region.source_nodes.clone(),
            semantic_kind: region.kind,
            kind: if structural {
                SpatialRegionKind::Structural
            } else if source.is_some_and(|node| node.role == SemanticRole::Dialog) {
                SpatialRegionKind::Dialog
            } else if source
                .is_some_and(|node| matches!(node.role, SemanticRole::TabList | SemanticRole::Tab))
            {
                SpatialRegionKind::TabStrip
            } else if source.is_some_and(|node| node.role == SemanticRole::MenuBar) {
                SpatialRegionKind::Toolbar
            } else {
                semantic_spatial_kind
            },
            priority: presentation_priority,
            importance: if structural || presentation_hidden {
                LayoutImportance::Structural
            } else {
                importance_for(presentation_priority)
            },
            obligation,
            demand,
            visibility: if obligation == PresentationObligation::Discoverable {
                VisibilityGuarantee::DiscoverableOnly
            } else {
                VisibilityGuarantee::Collapsible
            },
            purpose,
            bounds,
            coordinate_space,
            presentation,
            reasons: Vec::new(),
        };
        spatial
            .reasons
            .push(format!("semantic region = {:?}", region.kind));
        if structural {
            spatial
                .reasons
                .push("layout-useful structural evidence; no standalone semantic task".into());
        }
        if spatial.bounds.is_none() {
            spatial
                .reasons
                .push("geometry unavailable; semantic hierarchy fallback".into());
        }
        if let Some(node) = source {
            spatial.reasons.push(format!("role = {}", node.role));
        }
        regions.push(spatial);
    }
    enrich_command_surface_anchors(&mut regions, &nodes, evidence);
    coalesce_presentations(&mut regions, &analysis.root, &nodes);
    let primary = choose_primary(&mut regions, evidence);
    if let Some(primary) = primary {
        classify_surrounding(&mut regions, primary);
        suppress_untrusted_secondary_graphics(&mut regions, primary, &semantic);
    }
    refine_surface_policy(&mut regions, &nodes, root, primary);
    let surface_inference = surface_started.elapsed();
    let topology_started = Instant::now();
    let topology = SpatialTopology::infer(&regions);
    refine_policy_from_topology(&mut regions, &topology, primary);
    let topology_inference = topology_started.elapsed();
    let trust = overall_trust(evidence);
    let composition_started = Instant::now();
    let composition = choose_composition(&regions, primary, trust);
    let composition_planning = composition_started.elapsed();
    let layout_started = Instant::now();
    let root_layout = build_layout(&regions, primary, trust, composition);
    let layout_compilation = layout_started.elapsed();
    let mut metrics = LayoutMetrics {
        regions: regions.len(),
        primary: regions
            .iter()
            .filter(|r| r.kind == SpatialRegionKind::PrimaryContent)
            .count(),
        structural: regions
            .iter()
            .filter(|r| r.priority == PresentationPriority::Structural)
            .count(),
        surface_inference,
        topology_inference,
        composition_planning,
        layout_compilation,
        ..Default::default()
    };
    metrics.leaves = count_leaves(&root_layout);
    LayoutAnalysis {
        plan: TuiLayoutPlan {
            root: root_layout,
            regions,
            topology,
            generation: evidence.generation,
            geometry_trust: trust,
            composition,
        },
        metrics,
    }
}

fn enrich_command_surface_anchors(
    regions: &mut [SpatialRegion],
    nodes: &HashMap<RuntimeNodeId, &SemanticNode>,
    evidence: &SpatialEvidenceIndex,
) {
    fn collect(node: &SemanticNode, output: &mut Vec<RuntimeNodeId>) {
        if is_presentable_single_line_input(node)
            || matches!(node.role, SemanticRole::Tab | SemanticRole::TabList)
        {
            output.push(node.runtime_id);
        }
        for child in &node.children {
            collect(child, output);
        }
    }

    for region in regions
        .iter_mut()
        .filter(|region| region.presentation.kind == RegionPresentationKind::CommandBar)
    {
        let mut anchors = Vec::new();
        for source in &region.presentation.source_nodes {
            if let Some(node) = nodes.get(source) {
                collect(node, &mut anchors);
            }
        }
        anchors.sort_unstable();
        anchors.dedup();
        let before = region.presentation.source_nodes.len();
        for anchor in anchors {
            if !region.presentation.source_nodes.contains(&anchor) {
                region.presentation.source_nodes.push(anchor);
            }
        }
        let added = region
            .presentation
            .source_nodes
            .len()
            .saturating_sub(before);
        if added > 0 {
            if region.bounds.is_none()
                && let Some((bounds, coordinate_space)) =
                    region.presentation.source_nodes.iter().find_map(|id| {
                        evidence.trusted_bounds(*id).map(|bounds| {
                            (
                                bounds,
                                evidence
                                    .get(*id)
                                    .map(|item| item.coordinate_space)
                                    .unwrap_or(CoordinateSpace::Unknown),
                            )
                        })
                    })
            {
                region.bounds = Some(bounds);
                region.coordinate_space = coordinate_space;
            }
            region.presentation.reasons.push(format!(
                "retained {added} presentation anchor(s) nested in a command container"
            ));
        }
    }
}

fn is_user_content_kind(kind: &ContentBlockKind) -> bool {
    matches!(
        kind,
        ContentBlockKind::Heading { .. }
            | ContentBlockKind::Paragraph
            | ContentBlockKind::Text
            | ContentBlockKind::Link
            | ContentBlockKind::ListItem
            | ContentBlockKind::Quote
            | ContentBlockKind::Comment
            | ContentBlockKind::FormAnchor
            | ContentBlockKind::TableAnchor
    )
}

fn is_meaningful_text(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && text != "[text unavailable through the application's accessibility interface]"
}

fn is_substantial_inline_text(text: &str) -> bool {
    text.contains('\n') || text.chars().count() > 80
}

fn semantic_title(region: &SemanticRegion, source: Option<&SemanticNode>) -> String {
    if let Some(source) = source {
        match source.role {
            SemanticRole::Document => {
                if source.debug.atspi_role.to_ascii_lowercase().contains("web") {
                    return "Web Document".to_owned();
                }
                return "Document".to_owned();
            }
            SemanticRole::Image | SemanticRole::Video => return "Graphical Content".to_owned(),
            SemanticRole::Tree | SemanticRole::TreeItem | SemanticRole::List => {
                return region
                    .label
                    .clone()
                    .filter(|label| !label.trim().is_empty())
                    .unwrap_or_else(|| "Navigation".to_owned());
            }
            SemanticRole::TabList | SemanticRole::Tab => return "Tabs".to_owned(),
            SemanticRole::MenuBar | SemanticRole::Menu | SemanticRole::MenuItem => {
                return "Commands".to_owned();
            }
            SemanticRole::StatusBar => return "Status".to_owned(),
            _ => {}
        }
    }
    region
        .label
        .clone()
        .filter(|label| !label.trim().is_empty() && label.chars().count() <= 80)
        .unwrap_or_else(|| match region.kind {
            SemanticRegionKind::Navigation => "Navigation".to_owned(),
            SemanticRegionKind::Selection => "Selection".to_owned(),
            SemanticRegionKind::CommandSet => "Commands".to_owned(),
            SemanticRegionKind::Form | SemanticRegionKind::Field => "Form".to_owned(),
            SemanticRegionKind::Status => "Status".to_owned(),
            SemanticRegionKind::OpaqueContent => "Graphical Content".to_owned(),
            SemanticRegionKind::Content => "Content".to_owned(),
            SemanticRegionKind::Control => "Controls".to_owned(),
            SemanticRegionKind::Group | SemanticRegionKind::Unknown => "Details".to_owned(),
        })
}

fn descendant_presentation_items(region: &SemanticRegion) -> usize {
    region
        .children
        .iter()
        .map(|child| {
            usize::from(
                !child.interactions.is_empty()
                    || child
                        .label
                        .as_deref()
                        .is_some_and(|label| !label.trim().is_empty()),
            ) + descendant_presentation_items(child)
        })
        .sum()
}

fn presentation_for_region(
    region: &SemanticRegion,
    source: Option<&SemanticNode>,
    structural: bool,
    context: Option<&RegionPresentationContext>,
) -> RegionPresentation {
    let title = semantic_title(region, source);
    let source_regions = vec![region.id];
    let source_nodes = region.source_nodes.clone();
    if structural {
        return RegionPresentation {
            kind: RegionPresentationKind::Structural,
            title,
            source_regions,
            source_nodes,
            meaningful_items: 0,
            dominant_eligible: false,
            reasons: vec!["structural-only object retained for layout diagnostics".into()],
        };
    }

    let content = context.and_then(|context| {
        region
            .source_nodes
            .iter()
            .find_map(|source| context.content.get(source))
    });
    if let Some(content) = content {
        let title = match source.map(|node| &node.role) {
            Some(SemanticRole::Document) => semantic_title(region, source),
            Some(SemanticRole::TextInput) => "Document".to_owned(),
            _ => content.title.clone().unwrap_or(title),
        };
        let semantic_inline = source.is_some_and(|node| {
            node.role == SemanticRole::TextInput
                && node.text_input_kind != Some(crate::semantic::TextInputKind::Password)
                && node
                    .value
                    .as_deref()
                    .is_some_and(is_substantial_inline_text)
        });
        if content.meaningful_blocks > 0 || semantic_inline {
            return RegionPresentation {
                kind: RegionPresentationKind::InlineContent,
                title,
                source_regions,
                source_nodes,
                meaningful_items: content.meaningful_blocks.max(1),
                dominant_eligible: true,
                reasons: vec![
                    format!(
                        "{} meaningful block(s) or multiline semantic payload available",
                        content.meaningful_blocks.max(1)
                    ),
                    format!("content root = {}", content.root),
                    format!("bounded content model blocks = {}", content.total_blocks),
                    format!("partial content = {}", content.partial),
                ],
            };
        }
        if content.graphical_blocks > 0 {
            return RegionPresentation {
                kind: RegionPresentationKind::GraphicalPlaceholder,
                title,
                source_regions,
                source_nodes,
                meaningful_items: content.graphical_blocks,
                dominant_eligible: true,
                reasons: vec![
                    "content has no meaningful inline blocks but exposes fidelity-required content"
                        .into(),
                    format!("content root = {}", content.root),
                ],
            };
        }
        return RegionPresentation {
            kind: RegionPresentationKind::Empty,
            title,
            source_regions,
            source_nodes,
            meaningful_items: 0,
            dominant_eligible: false,
            reasons: vec![
                "semantic content owner has no meaningful inline payload".into(),
                "Reader-only/empty placeholder is not eligible for dominant allocation".into(),
            ],
        };
    }

    let child_items = descendant_presentation_items(region);
    let actionable = region.interactions.len()
        + region
            .children
            .iter()
            .map(descendant_interactions)
            .sum::<usize>();
    let role = source.map(|node| &node.role);
    let (kind, meaningful_items, dominant_eligible, reason) = match (region.kind, role) {
        (SemanticRegionKind::OpaqueContent, _)
        | (_, Some(SemanticRole::Image | SemanticRole::Video)) => (
            RegionPresentationKind::GraphicalPlaceholder,
            1,
            true,
            "fidelity-required semantic content has an honest terminal placeholder",
        ),
        (_, Some(SemanticRole::Table)) => (
            RegionPresentationKind::Table,
            child_items.max(actionable),
            child_items.max(actionable) > 0,
            "table descendants provide a meaningful structured presentation",
        ),
        (SemanticRegionKind::CommandSet, _) => (
            RegionPresentationKind::CommandBar,
            child_items.max(actionable),
            false,
            "command hierarchy is reachable through a compact command surface",
        ),
        (SemanticRegionKind::Status, _) => (
            RegionPresentationKind::Status,
            usize::from(region.label.is_some()),
            false,
            "status semantics map to a compact status presentation",
        ),
        (SemanticRegionKind::Selection, _)
        | (_, Some(SemanticRole::Tree | SemanticRole::TreeItem | SemanticRole::List)) => (
            RegionPresentationKind::ChoiceList,
            child_items.max(actionable),
            false,
            "selection/tree descendants provide a navigable presentation",
        ),
        (_, Some(SemanticRole::TabList | SemanticRole::Tab))
        | (SemanticRegionKind::Navigation, _)
            if !matches!(role, Some(SemanticRole::Application | SemanticRole::Window)) =>
        {
            (
                RegionPresentationKind::Navigation,
                child_items.max(actionable),
                false,
                "semantic container provides navigation structure",
            )
        }
        (SemanticRegionKind::Form, _) | (_, Some(SemanticRole::Dialog))
            if child_items.max(actionable) > 0 =>
        {
            (
                RegionPresentationKind::Form,
                child_items.max(actionable),
                true,
                "form/dialog descendants provide user-facing controls",
            )
        }
        (_, Some(SemanticRole::TextInput))
            if source.is_some_and(is_presentable_single_line_input) =>
        {
            (
                RegionPresentationKind::ControlGroup,
                1,
                false,
                "safe single-line editable input has a direct presentation payload",
            )
        }
        (SemanticRegionKind::Field | SemanticRegionKind::Control, _) if actionable > 0 => (
            RegionPresentationKind::ControlGroup,
            actionable,
            false,
            "safe semantic interaction is directly presentable",
        ),
        (SemanticRegionKind::Content, _)
            if region
                .label
                .as_deref()
                .is_some_and(|label| !label.trim().is_empty()) =>
        {
            (
                RegionPresentationKind::CollapsedSummary,
                1,
                false,
                "short semantic text is useful as supporting context",
            )
        }
        _ if matches!(role, Some(SemanticRole::Application | SemanticRole::Window)) => (
            RegionPresentationKind::DiagnosticOnly,
            0,
            false,
            "application/window container organizes descendants but is not a pane itself",
        ),
        _ if actionable > 0 => (
            RegionPresentationKind::ControlGroup,
            actionable,
            false,
            "container owns reachable semantic interactions",
        ),
        _ => (
            RegionPresentationKind::DiagnosticOnly,
            0,
            false,
            "no standalone user-facing presentation payload",
        ),
    };
    RegionPresentation {
        kind,
        title,
        source_regions,
        source_nodes,
        meaningful_items,
        dominant_eligible,
        reasons: vec![reason.into()],
    }
}

fn descendant_interactions(region: &SemanticRegion) -> usize {
    region.interactions.len()
        + region
            .children
            .iter()
            .map(descendant_interactions)
            .sum::<usize>()
}

fn presentation_default_title(kind: RegionPresentationKind) -> &'static str {
    match kind {
        RegionPresentationKind::InlineContent => "Document",
        RegionPresentationKind::InputSurface => "Input",
        RegionPresentationKind::Navigation | RegionPresentationKind::ChoiceList => "Navigation",
        RegionPresentationKind::Form => "Form",
        RegionPresentationKind::Table => "Table",
        RegionPresentationKind::CommandBar => "Commands",
        RegionPresentationKind::GraphicalPlaceholder => "Graphical Content",
        RegionPresentationKind::Status => "Status",
        RegionPresentationKind::ControlGroup => "Controls",
        RegionPresentationKind::WorkspacePane => "Details",
        RegionPresentationKind::CollapsedSummary => "Details",
        RegionPresentationKind::Structural => "Structure",
        RegionPresentationKind::DiagnosticOnly => "Diagnostics",
        RegionPresentationKind::Empty => "Content",
    }
}

fn semantic_parent_map(root: &SemanticRegion) -> HashMap<RegionId, RegionId> {
    fn visit(
        region: &SemanticRegion,
        parent: Option<RegionId>,
        output: &mut HashMap<RegionId, RegionId>,
    ) {
        if let Some(parent) = parent {
            output.insert(region.id, parent);
        }
        for child in &region.children {
            visit(child, Some(region.id), output);
        }
    }
    let mut output = HashMap::new();
    visit(root, None, &mut output);
    output
}

fn coalesce_presentations(
    regions: &mut [SpatialRegion],
    semantic_root: &SemanticRegion,
    nodes: &HashMap<RuntimeNodeId, &SemanticNode>,
) {
    let parents = semantic_parent_map(semantic_root);
    promote_leaf_workspace_containers(regions, semantic_root, &parents);
    let mut leaders: HashMap<(RegionId, RegionPresentationKind), usize> = HashMap::new();
    for index in 0..regions.len() {
        let kind = regions[index].presentation.kind;
        if !matches!(
            kind,
            RegionPresentationKind::Navigation
                | RegionPresentationKind::ChoiceList
                | RegionPresentationKind::CommandBar
                | RegionPresentationKind::ControlGroup
                | RegionPresentationKind::CollapsedSummary
        ) {
            continue;
        }
        let Some(parent) = parents.get(&regions[index].semantic_region).copied() else {
            continue;
        };
        let key = (parent, kind);
        let Some(leader) = leaders.get(&key).copied() else {
            leaders.insert(key, index);
            continue;
        };
        let merged_region = regions[index].semantic_region;
        let merged_nodes = regions[index].presentation.source_nodes.clone();
        let merged_items = regions[index].presentation.meaningful_items;
        regions[leader]
            .presentation
            .source_regions
            .push(merged_region);
        for node in merged_nodes {
            if !regions[leader].presentation.source_nodes.contains(&node) {
                regions[leader].presentation.source_nodes.push(node);
            }
        }
        regions[leader].presentation.meaningful_items = regions[leader]
            .presentation
            .meaningful_items
            .saturating_add(merged_items);
        regions[leader].presentation.title = presentation_default_title(kind).to_owned();
        regions[leader].presentation.reasons.push(format!(
            "coalesced compatible sibling region {merged_region} under semantic parent {parent}"
        ));
        regions[index].presentation.kind = RegionPresentationKind::DiagnosticOnly;
        regions[index].presentation.dominant_eligible = false;
        regions[index].presentation.reasons.push(format!(
            "coalesced into spatial region {}",
            regions[leader].id.0
        ));
        regions[index].priority = PresentationPriority::HiddenByDefault;
        regions[index].importance = LayoutImportance::Structural;
    }

    // A presentable semantic container owns compatible descendant payloads.
    // Keep every RuntimeNodeId/source RegionId on the owner so grouping never
    // erases operation identity or reachability.
    let by_semantic: HashMap<_, _> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| (region.semantic_region, index))
        .collect();
    for child in 0..regions.len() {
        if regions[child].priority == PresentationPriority::HiddenByDefault
            || !regions[child].presentation.kind.is_meaningful()
        {
            continue;
        }
        let child_kind = regions[child].presentation.kind;
        let mut ancestor = parents.get(&regions[child].semantic_region).copied();
        let mut owner = None;
        while let Some(id) = ancestor {
            if let Some(index) = by_semantic.get(&id).copied()
                && regions[index].priority != PresentationPriority::HiddenByDefault
                && presentation_container_owns(regions[index].presentation.kind, child_kind)
            {
                owner = Some(index);
                break;
            }
            ancestor = parents.get(&id).copied();
        }
        let Some(owner) = owner else { continue };
        if regions[owner].presentation.kind == RegionPresentationKind::WorkspacePane
            && regions[child]
                .presentation
                .source_nodes
                .iter()
                .filter_map(|id| nodes.get(id).copied())
                .any(is_persistent_surface_anchor)
        {
            regions[child].presentation.reasons.push(
                "retained outside workspace aggregation as a potential persistent surface anchor"
                    .into(),
            );
            continue;
        }
        let child_region_ids = regions[child].presentation.source_regions.clone();
        let child_nodes = regions[child].presentation.source_nodes.clone();
        let child_items = regions[child].presentation.meaningful_items;
        for region_id in child_region_ids {
            if !regions[owner]
                .presentation
                .source_regions
                .contains(&region_id)
            {
                regions[owner].presentation.source_regions.push(region_id);
            }
        }
        for node in child_nodes {
            if !regions[owner].presentation.source_nodes.contains(&node) {
                regions[owner].presentation.source_nodes.push(node);
            }
        }
        regions[owner].presentation.meaningful_items = regions[owner]
            .presentation
            .meaningful_items
            .saturating_add(child_items);
        regions[owner].presentation.reasons.push(format!(
            "absorbed compatible descendant presentation from region {}",
            regions[child].semantic_region
        ));
        regions[child].presentation.kind = RegionPresentationKind::DiagnosticOnly;
        regions[child].presentation.dominant_eligible = false;
        regions[child].presentation.reasons.push(format!(
            "presented by ancestor spatial region {}",
            regions[owner].id.0
        ));
        regions[child].priority = PresentationPriority::HiddenByDefault;
        regions[child].importance = LayoutImportance::Structural;
    }
    coalesce_compact_application_groups(regions);
}

fn is_persistent_surface_anchor(node: &SemanticNode) -> bool {
    matches!(
        node.role,
        SemanticRole::Tab | SemanticRole::TabList | SemanticRole::MenuBar
    ) || is_presentable_single_line_input(node)
}

fn is_presentable_single_line_input(node: &SemanticNode) -> bool {
    node.role == SemanticRole::TextInput
        && node.text_input_kind == Some(TextInputKind::Plain)
        && (node.capabilities.contains(&SemanticCapability::EditText)
            || node.states.contains(&SemanticState::Editable))
        && !node
            .states
            .iter()
            .any(|state| matches!(state, SemanticState::Other(value) if value == "multi-line"))
}

fn presentation_container_owns(
    parent: RegionPresentationKind,
    child: RegionPresentationKind,
) -> bool {
    match parent {
        RegionPresentationKind::Navigation | RegionPresentationKind::ChoiceList => matches!(
            child,
            RegionPresentationKind::Navigation
                | RegionPresentationKind::ChoiceList
                | RegionPresentationKind::ControlGroup
                | RegionPresentationKind::CollapsedSummary
        ),
        RegionPresentationKind::Form => matches!(
            child,
            RegionPresentationKind::Form
                | RegionPresentationKind::ChoiceList
                | RegionPresentationKind::ControlGroup
                | RegionPresentationKind::CollapsedSummary
        ),
        RegionPresentationKind::Table => matches!(
            child,
            RegionPresentationKind::ControlGroup | RegionPresentationKind::CollapsedSummary
        ),
        RegionPresentationKind::ControlGroup => matches!(
            child,
            RegionPresentationKind::ControlGroup | RegionPresentationKind::CollapsedSummary
        ),
        RegionPresentationKind::WorkspacePane => !matches!(
            child,
            RegionPresentationKind::InlineContent
                | RegionPresentationKind::GraphicalPlaceholder
                | RegionPresentationKind::Structural
                | RegionPresentationKind::DiagnosticOnly
                | RegionPresentationKind::Empty
                | RegionPresentationKind::WorkspacePane
        ),
        RegionPresentationKind::InlineContent => matches!(
            child,
            RegionPresentationKind::InlineContent
                | RegionPresentationKind::GraphicalPlaceholder
                | RegionPresentationKind::CollapsedSummary
        ),
        RegionPresentationKind::GraphicalPlaceholder => matches!(
            child,
            RegionPresentationKind::GraphicalPlaceholder | RegionPresentationKind::CollapsedSummary
        ),
        _ => false,
    }
}

fn promote_leaf_workspace_containers(
    regions: &mut [SpatialRegion],
    semantic_root: &SemanticRegion,
    parents: &HashMap<RegionId, RegionId>,
) {
    let by_semantic: HashMap<_, _> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| (region.semantic_region, index))
        .collect();
    let mut direct_children: HashMap<RegionId, Vec<RegionId>> = HashMap::new();
    fn collect_children(region: &SemanticRegion, output: &mut HashMap<RegionId, Vec<RegionId>>) {
        output.insert(
            region.id,
            region.children.iter().map(|child| child.id).collect(),
        );
        for child in &region.children {
            collect_children(child, output);
        }
    }
    collect_children(semantic_root, &mut direct_children);

    for index in 0..regions.len() {
        if regions[index].presentation.kind != RegionPresentationKind::DiagnosticOnly
            || !regions[index]
                .reasons
                .iter()
                .any(|reason| reason == "role = Window")
            || !parents.contains_key(&regions[index].semantic_region)
        {
            continue;
        }
        let has_child_window = direct_children
            .get(&regions[index].semantic_region)
            .into_iter()
            .flatten()
            .filter_map(|child| by_semantic.get(child).copied())
            .any(|child| {
                regions[child]
                    .reasons
                    .iter()
                    .any(|reason| reason == "role = Window")
            });
        if has_child_window {
            continue;
        }
        let region_id = regions[index].semantic_region;
        let has_content_surface = regions.iter().any(|candidate| {
            matches!(
                candidate.presentation.kind,
                RegionPresentationKind::InlineContent
                    | RegionPresentationKind::GraphicalPlaceholder
            ) && std::iter::successors(parents.get(&candidate.semantic_region).copied(), |parent| {
                parents.get(parent).copied()
            })
            .any(|ancestor| ancestor == region_id)
        });
        if has_content_surface {
            regions[index].presentation.reasons.push(
                "window remains structural because a descendant content surface owns presentation"
                    .into(),
            );
            continue;
        }
        let meaningful_items = regions
            .iter()
            .filter(|candidate| {
                candidate.presentation.kind.is_meaningful()
                    && std::iter::successors(
                        parents.get(&candidate.semantic_region).copied(),
                        |parent| parents.get(parent).copied(),
                    )
                    .any(|ancestor| ancestor == region_id)
            })
            .map(|candidate| candidate.presentation.meaningful_items.max(1))
            .sum::<usize>();
        if meaningful_items == 0 {
            continue;
        }
        regions[index].presentation.kind = RegionPresentationKind::WorkspacePane;
        regions[index].presentation.meaningful_items = meaningful_items;
        if matches!(
            regions[index].presentation.title.as_str(),
            "Navigation" | "Details"
        ) {
            let inherited = std::iter::successors(
                parents.get(&regions[index].semantic_region).copied(),
                |parent| parents.get(parent).copied(),
            )
            .filter_map(|ancestor| by_semantic.get(&ancestor).copied())
            .map(|ancestor| regions[ancestor].presentation.title.clone())
            .find(|title| {
                !matches!(
                    title.as_str(),
                    "Navigation" | "Details" | "Controls" | "Diagnostics"
                )
            });
            if let Some(title) = inherited {
                regions[index].presentation.title = title;
            }
        }
        regions[index]
            .presentation
            .reasons
            .push("leaf semantic window groups several compatible user-facing descendants".into());
        regions[index].priority = PresentationPriority::Auxiliary;
        regions[index].importance = LayoutImportance::Supporting;
        regions[index].obligation = PresentationObligation::Contextual;
        regions[index].demand = LayoutDemand::Supporting;
    }
}

fn coalesce_compact_application_groups(regions: &mut [SpatialRegion]) {
    let largest_area = regions
        .iter()
        .filter_map(|region| region.bounds.map(SpatialBounds::area))
        .max()
        .unwrap_or(0);
    let mut leaders: HashMap<RegionPresentationKind, usize> = HashMap::new();
    for index in 0..regions.len() {
        let kind = regions[index].presentation.kind;
        if regions[index].priority == PresentationPriority::HiddenByDefault
            || !matches!(
                kind,
                RegionPresentationKind::Navigation
                    | RegionPresentationKind::ChoiceList
                    | RegionPresentationKind::CommandBar
                    | RegionPresentationKind::Status
                    | RegionPresentationKind::ControlGroup
                    | RegionPresentationKind::CollapsedSummary
            )
        {
            continue;
        }
        let compact = regions[index]
            .bounds
            .map(|bounds| largest_area == 0 || bounds.area().saturating_mul(5) <= largest_area)
            .unwrap_or(true);
        if !compact {
            continue;
        }
        let Some(leader) = leaders.get(&kind).copied() else {
            leaders.insert(kind, index);
            continue;
        };
        let source_regions = regions[index].presentation.source_regions.clone();
        let source_nodes = regions[index].presentation.source_nodes.clone();
        let items = regions[index].presentation.meaningful_items;
        for region_id in source_regions {
            if !regions[leader]
                .presentation
                .source_regions
                .contains(&region_id)
            {
                regions[leader].presentation.source_regions.push(region_id);
            }
        }
        for node in source_nodes {
            if !regions[leader].presentation.source_nodes.contains(&node) {
                regions[leader].presentation.source_nodes.push(node);
            }
        }
        regions[leader].presentation.meaningful_items = regions[leader]
            .presentation
            .meaningful_items
            .saturating_add(items);
        regions[leader].presentation.title = presentation_default_title(kind).to_owned();
        regions[leader].presentation.reasons.push(format!(
            "coalesced compact compatible application-scope region {}",
            regions[index].semantic_region
        ));
        regions[index].presentation.kind = RegionPresentationKind::DiagnosticOnly;
        regions[index].presentation.dominant_eligible = false;
        regions[index].presentation.reasons.push(format!(
            "coalesced into spatial region {}",
            regions[leader].id.0
        ));
        regions[index].priority = PresentationPriority::HiddenByDefault;
        regions[index].importance = LayoutImportance::Structural;
    }
}

fn choose_composition(
    regions: &[SpatialRegion],
    primary: Option<SpatialRegionId>,
    _trust: GeometryTrust,
) -> CompositionKind {
    if let Some(primary) = primary {
        return regions.iter().find(|region| region.id == primary).map_or(
            CompositionKind::ContentDominant,
            |region| {
                if region.presentation.kind == RegionPresentationKind::Form {
                    CompositionKind::DialogForm
                } else {
                    CompositionKind::ContentDominant
                }
            },
        );
    }
    let visible: Vec<_> = regions
        .iter()
        .filter(|region| {
            region.presentation.kind.is_meaningful()
                && region.priority != PresentationPriority::HiddenByDefault
        })
        .collect();
    let navigation = visible.iter().any(|region| {
        matches!(
            region.presentation.kind,
            RegionPresentationKind::Navigation | RegionPresentationKind::ChoiceList
        )
    });
    let detail = visible.iter().any(|region| {
        matches!(
            region.presentation.kind,
            RegionPresentationKind::Form
                | RegionPresentationKind::Table
                | RegionPresentationKind::ControlGroup
                | RegionPresentationKind::WorkspacePane
                | RegionPresentationKind::CollapsedSummary
        )
    });
    if visible.iter().any(|region| {
        region.kind == SpatialRegionKind::Dialog
            || region.presentation.kind == RegionPresentationKind::Form
    }) {
        CompositionKind::DialogForm
    } else if navigation && detail {
        CompositionKind::NavigationDetail
    } else if visible.len() >= 2 {
        CompositionKind::MultiPaneWorkspace
    } else if detail {
        CompositionKind::ControlSurface
    } else {
        CompositionKind::FallbackStack
    }
}

fn build_no_primary_layout(
    regions: &[SpatialRegion],
    leaves: Vec<LayoutNode>,
    trust: GeometryTrust,
    composition: CompositionKind,
) -> LayoutNode {
    if leaves.len() <= 1 {
        return LayoutNode::Stack(leaves);
    }
    if composition == CompositionKind::NavigationDetail {
        let mut navigation = Vec::new();
        let mut details = Vec::new();
        for leaf in &leaves {
            let Some(region) = region_for_layout_node(regions, leaf) else {
                continue;
            };
            if matches!(
                region.presentation.kind,
                RegionPresentationKind::Navigation | RegionPresentationKind::ChoiceList
            ) {
                navigation.push(leaf.clone());
            } else {
                details.push(leaf.clone());
            }
        }
        if !navigation.is_empty() && !details.is_empty() {
            let navigation_weights = vec![1; navigation.len()];
            let detail_weights = vec![1; details.len()];
            return LayoutNode::HorizontalSplit {
                children: vec![
                    LayoutNode::VerticalSplit {
                        children: navigation,
                        weights: navigation_weights,
                    },
                    LayoutNode::VerticalSplit {
                        children: details,
                        weights: detail_weights,
                    },
                ],
                weights: vec![1, 2],
            };
        }
    }
    if matches!(trust, GeometryTrust::Consistent | GeometryTrust::Partial) {
        let bounds: Vec<_> = leaves
            .iter()
            .filter_map(|leaf| region_for_layout_node(regions, leaf)?.bounds)
            .collect();
        if bounds.len() >= 2 {
            let min_x = bounds
                .iter()
                .map(|bounds| bounds.center().0)
                .min()
                .unwrap_or(0);
            let max_x = bounds
                .iter()
                .map(|bounds| bounds.center().0)
                .max()
                .unwrap_or(0);
            let min_y = bounds
                .iter()
                .map(|bounds| bounds.center().1)
                .min()
                .unwrap_or(0);
            let max_y = bounds
                .iter()
                .map(|bounds| bounds.center().1)
                .max()
                .unwrap_or(0);
            if max_x.saturating_sub(min_x) > max_y.saturating_sub(min_y) {
                if leaves.len() > 4 {
                    let mut ordered = leaves;
                    ordered.sort_by_key(|leaf| {
                        region_for_layout_node(regions, leaf)
                            .and_then(|region| region.bounds)
                            .map(|bounds| bounds.center().0)
                            .unwrap_or(i32::MAX)
                    });
                    let right = ordered.split_off(ordered.len().div_ceil(2));
                    let left_weights = vec![1; ordered.len()];
                    let right_weights = vec![1; right.len()];
                    return LayoutNode::HorizontalSplit {
                        children: vec![
                            LayoutNode::VerticalSplit {
                                children: ordered,
                                weights: left_weights,
                            },
                            LayoutNode::VerticalSplit {
                                children: right,
                                weights: right_weights,
                            },
                        ],
                        weights: vec![1, 1],
                    };
                }
                let count = leaves.len();
                return LayoutNode::HorizontalSplit {
                    children: leaves,
                    weights: vec![1; count],
                };
            }
        }
    }
    if leaves.len() > 4 {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (index, leaf) in leaves.into_iter().enumerate() {
            if index % 2 == 0 {
                left.push(leaf);
            } else {
                right.push(leaf);
            }
        }
        let left_weights = vec![1; left.len()];
        let right_weights = vec![1; right.len()];
        return LayoutNode::HorizontalSplit {
            children: vec![
                LayoutNode::VerticalSplit {
                    children: left,
                    weights: left_weights,
                },
                LayoutNode::VerticalSplit {
                    children: right,
                    weights: right_weights,
                },
            ],
            weights: vec![1, 1],
        };
    }
    let count = leaves.len();
    LayoutNode::VerticalSplit {
        children: leaves,
        weights: vec![1; count],
    }
}

fn region_for_layout_node<'a>(
    regions: &'a [SpatialRegion],
    node: &LayoutNode,
) -> Option<&'a SpatialRegion> {
    let LayoutNode::Leaf(id) = node else {
        return None;
    };
    regions.iter().find(|region| region.id == *id)
}

fn build_layout(
    regions: &[SpatialRegion],
    primary: Option<SpatialRegionId>,
    trust: GeometryTrust,
    composition: CompositionKind,
) -> LayoutNode {
    let leaves: Vec<_> = regions
        .iter()
        .filter(|r| {
            !matches!(
                r.priority,
                PresentationPriority::HiddenByDefault | PresentationPriority::Structural
            )
        })
        .map(|r| LayoutNode::Leaf(r.id))
        .collect();
    if leaves.is_empty() {
        return LayoutNode::Stack(Vec::new());
    }
    let Some(primary) = primary else {
        return build_no_primary_layout(regions, leaves, trust, composition);
    };
    if !matches!(trust, GeometryTrust::Consistent | GeometryTrust::Partial) {
        return LayoutNode::Stack(leaves);
    }
    let primary_region = regions.iter().find(|r| r.id == primary);
    let Some(primary_region) = primary_region else {
        return LayoutNode::Stack(leaves);
    };
    let p = primary_region.bounds;
    let primary_space = primary_region.coordinate_space;
    let Some(p) = p else {
        return LayoutNode::Stack(leaves);
    };
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut above = Vec::new();
    let mut below = Vec::new();
    let mut overlays = Vec::new();
    for r in regions.iter().filter(|r| {
        r.id != primary
            && !matches!(
                r.priority,
                PresentationPriority::Structural | PresentationPriority::HiddenByDefault
            )
    }) {
        if r.coordinate_space == CoordinateSpace::Unknown || r.coordinate_space != primary_space {
            continue;
        }
        let Some(b) = r.bounds else {
            continue;
        };
        if b.intersects(p)
            && !p.contains(b)
            && !b.contains(p)
            && matches!(
                r.kind,
                SpatialRegionKind::Dialog | SpatialRegionKind::Overlay
            )
        {
            overlays.push(LayoutNode::Leaf(r.id));
            continue;
        }
        let (cx, cy) = b.center();
        if b.height > p.height / 2 && b.width < p.width / 2 && cx < p.x {
            left.push(LayoutNode::Leaf(r.id));
        } else if b.height > p.height / 2 && b.width < p.width / 2 && cx > p.right() {
            right.push(LayoutNode::Leaf(r.id));
        } else if b.width > p.width / 2 && b.height < p.height / 2 && cy < p.y {
            above.push(LayoutNode::Leaf(r.id));
        } else if b.width > p.width / 2 && b.height < p.height / 2 && cy > p.bottom() {
            below.push(LayoutNode::Leaf(r.id));
        }
    }
    let placed: HashSet<_> = left
        .iter()
        .chain(&right)
        .chain(&above)
        .chain(&below)
        .chain(&overlays)
        .filter_map(|node| match node {
            LayoutNode::Leaf(id) => Some(*id),
            _ => None,
        })
        .chain(std::iter::once(primary))
        .collect();
    below.extend(leaves.iter().filter_map(|node| match node {
        LayoutNode::Leaf(id) if !placed.contains(id) => Some(LayoutNode::Leaf(*id)),
        _ => None,
    }));
    let center = LayoutNode::Leaf(primary);
    let main = if !left.is_empty() || !right.is_empty() {
        let mut c = left;
        c.push(center);
        c.extend(right);
        let weights = c
            .iter()
            .map(|node| u16::from(layout_node_contains(node, primary)) * 3 + 1)
            .collect();
        LayoutNode::HorizontalSplit {
            weights,
            children: c,
        }
    } else {
        center
    };
    let has_vertical = !above.is_empty() || !below.is_empty();
    let mut stack = Vec::new();
    stack.extend(above);
    stack.push(main);
    stack.extend(below);
    let base = if !stack.is_empty() && has_vertical {
        if stack.len() > 1 {
            let weights = stack
                .iter()
                .map(|node| u16::from(layout_node_contains(node, primary)) * 5 + 1)
                .collect();
            LayoutNode::VerticalSplit {
                weights,
                children: stack,
            }
        } else {
            LayoutNode::Stack(stack)
        }
    } else if stack.len() == 1 {
        stack.pop().unwrap()
    } else {
        LayoutNode::Stack(stack)
    };
    if overlays.is_empty() {
        base
    } else {
        LayoutNode::Overlay {
            base: Box::new(base),
            overlays,
        }
    }
}

fn layout_node_contains(node: &LayoutNode, target: SpatialRegionId) -> bool {
    match node {
        LayoutNode::Leaf(id) => *id == target,
        LayoutNode::Stack(children)
        | LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => children
            .iter()
            .any(|child| layout_node_contains(child, target)),
        LayoutNode::Overlay { base, overlays } => {
            layout_node_contains(base, target)
                || overlays
                    .iter()
                    .any(|child| layout_node_contains(child, target))
        }
    }
}

pub fn realize_responsive_layout(
    plan: &TuiLayoutPlan,
    width: u16,
    height: u16,
    active_region: Option<SpatialRegionId>,
) -> ResponsiveComposition {
    let mut candidates = plan
        .regions
        .iter()
        .filter(|region| {
            region.demand != LayoutDemand::Hidden
                && !matches!(
                    region.obligation,
                    PresentationObligation::Structural | PresentationObligation::DiagnosticOnly
                )
                && region.presentation.kind.is_meaningful()
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|region| std::cmp::Reverse(surface_rank(region, active_region)));
    let class = TerminalWidthClass::for_size(width, candidates.len());
    let row_capacity = usize::from(height.saturating_sub(3) / 4).max(1);
    let column_capacity = match class {
        TerminalWidthClass::Wide => 3,
        TerminalWidthClass::Medium => 2,
        TerminalWidthClass::Narrow | TerminalWidthClass::Compact => 1,
    };
    let pane_capacity = row_capacity.saturating_mul(column_capacity).clamp(1, 6);
    let direct_limit = match class {
        TerminalWidthClass::Wide => pane_capacity,
        TerminalWidthClass::Medium => pane_capacity.min(3),
        TerminalWidthClass::Narrow | TerminalWidthClass::Compact => 1,
    };
    let mut selected = Vec::new();
    if let Some(active) = active_region
        && candidates.iter().any(|region| region.id == active)
    {
        selected.push(active);
    }
    if let Some(primary) = candidates
        .iter()
        .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
        .map(|region| region.id)
        && !selected.contains(&primary)
        && selected.len() < direct_limit
    {
        selected.push(primary);
    }
    let can_add_direct = |selected: &[SpatialRegionId], candidate: &SpatialRegion| {
        if width < minimum_viable_direct_width(candidate) {
            return false;
        }
        let used = selected
            .iter()
            .filter_map(|id| plan.regions.iter().find(|region| region.id == *id))
            .map(minimum_viable_direct_height)
            .sum::<u16>();
        let selector = u16::from(selected.len().saturating_add(1) < candidates.len()) * 3;
        used.saturating_add(minimum_viable_direct_height(candidate))
            .saturating_add(selector)
            <= height
    };
    for region in candidates
        .iter()
        .filter(|region| region.visibility == VisibilityGuarantee::Pinned)
    {
        if !selected.contains(&region.id) && can_add_direct(&selected, region) {
            selected.push(region.id);
        }
    }
    for region in &candidates {
        if selected.len() >= direct_limit {
            break;
        }
        if region.obligation == PresentationObligation::Persistent
            && region.demand != LayoutDemand::Minimal
            && !selected.contains(&region.id)
        {
            selected.push(region.id);
        }
    }
    let prefer_direct_limit = match class {
        TerminalWidthClass::Wide | TerminalWidthClass::Medium => direct_limit,
        TerminalWidthClass::Narrow | TerminalWidthClass::Compact => {
            selected.len().saturating_add(1)
        }
    };
    for region in candidates
        .iter()
        .filter(|region| region.visibility == VisibilityGuarantee::PreferDirect)
    {
        if selected.len() >= prefer_direct_limit {
            break;
        }
        if !selected.contains(&region.id) && can_add_direct(&selected, region) {
            selected.push(region.id);
        }
    }
    for region in &candidates {
        if selected.len() >= direct_limit {
            break;
        }
        if !selected.contains(&region.id)
            && region.obligation != PresentationObligation::Discoverable
            && region.demand != LayoutDemand::Minimal
        {
            selected.push(region.id);
        }
    }
    if selected.is_empty()
        && let Some(region) = candidates.first()
    {
        selected.push(region.id);
    }
    let collapsed = candidates
        .iter()
        .filter_map(|region| (!selected.contains(&region.id)).then_some(region.id))
        .collect::<Vec<_>>();
    let forced_collapsed = collapsed
        .iter()
        .filter_map(|id| candidates.iter().find(|region| region.id == *id))
        .filter(|region| region.visibility == VisibilityGuarantee::Pinned)
        .filter(|region| !can_add_direct(&selected, region))
        .map(|region| region.id)
        .collect::<Vec<_>>();
    let root = responsive_root(plan, &selected, class);
    let mut represented = selected.clone();
    represented.extend(collapsed.iter().copied());
    let reasons = vec![
        format!(
            "terminal class = {:?} from {}x{} and minimum viable pane demand",
            class, width, height
        ),
        format!(
            "{} direct surface(s), {} compact/collapsed surface(s)",
            selected.len(),
            collapsed.len()
        ),
        "terminal realization reused spatial topology without scaling GUI coordinates".into(),
    ];
    ResponsiveComposition {
        class,
        root,
        represented,
        collapsed,
        forced_collapsed,
        reasons,
    }
}

fn minimum_viable_direct_height(region: &SpatialRegion) -> u16 {
    match region.demand {
        LayoutDemand::Expand => 8,
        LayoutDemand::Supporting => 5,
        LayoutDemand::Compact => 4,
        LayoutDemand::Minimal => 3,
        LayoutDemand::Hidden => 0,
    }
}

fn minimum_viable_direct_width(region: &SpatialRegion) -> u16 {
    match region.demand {
        LayoutDemand::Expand => 30,
        LayoutDemand::Supporting => 22,
        LayoutDemand::Compact => 20,
        LayoutDemand::Minimal => 14,
        LayoutDemand::Hidden => u16::MAX,
    }
}

fn surface_rank(
    region: &SpatialRegion,
    active_region: Option<SpatialRegionId>,
) -> (bool, u8, u8, u8, usize, std::cmp::Reverse<u64>) {
    let visibility = match region.visibility {
        VisibilityGuarantee::Pinned => 4,
        VisibilityGuarantee::PreferDirect => 3,
        VisibilityGuarantee::Collapsible => 2,
        VisibilityGuarantee::DiscoverableOnly => 1,
    };
    let obligation = match region.obligation {
        PresentationObligation::Persistent => 4,
        PresentationObligation::Contextual => 3,
        PresentationObligation::Discoverable => 2,
        PresentationObligation::Structural => 1,
        PresentationObligation::DiagnosticOnly => 0,
    };
    let demand = match region.demand {
        LayoutDemand::Expand => 5,
        LayoutDemand::Supporting => 4,
        LayoutDemand::Compact => 3,
        LayoutDemand::Minimal => 2,
        LayoutDemand::Hidden => 0,
    };
    (
        Some(region.id) == active_region,
        visibility,
        obligation,
        demand,
        region.presentation.meaningful_items,
        std::cmp::Reverse(region.id.0),
    )
}

fn responsive_root(
    plan: &TuiLayoutPlan,
    selected: &[SpatialRegionId],
    class: TerminalWidthClass,
) -> LayoutNode {
    if selected.is_empty() {
        return LayoutNode::Stack(Vec::new());
    }
    if selected.len() == 1 {
        return LayoutNode::Leaf(selected[0]);
    }
    let primary = selected.iter().copied().find(|id| {
        plan.regions
            .iter()
            .any(|region| region.id == *id && region.kind == SpatialRegionKind::PrimaryContent)
    });
    // The original topology is useful when it is organized around a valid
    // dominant surface.  Without one, preserving the early spatial tree can
    // accidentally preserve arbitrary container nesting as terminal sizing
    // (the equal-grid/oversized-pane failure).  No-primary workspaces are
    // realized below from presentation demand while retaining every selected
    // surface and its source bindings.
    if primary.is_some()
        && matches!(class, TerminalWidthClass::Wide | TerminalWidthClass::Medium)
        && let Some(pruned) = prune_layout(plan, &plan.root, &selected.iter().copied().collect())
    {
        return pruned;
    }
    if let Some(primary) = primary {
        let mut compact = selected
            .iter()
            .copied()
            .filter(|id| *id != primary)
            .map(LayoutNode::Leaf)
            .collect::<Vec<_>>();
        let mut children = vec![LayoutNode::Leaf(primary)];
        children.append(&mut compact);
        let weights = children
            .iter()
            .map(|node| demand_weight(plan, node))
            .collect();
        return LayoutNode::VerticalSplit { children, weights };
    }
    let column_count = usize::from(matches!(
        class,
        TerminalWidthClass::Wide | TerminalWidthClass::Medium
    ))
    .saturating_add(1)
    .min(selected.len());
    let mut columns = vec![Vec::new(); column_count];
    let mut loads = vec![0_u16; column_count];
    for id in selected {
        let weight = demand_weight(plan, &LayoutNode::Leaf(*id)).max(1);
        let column = loads
            .iter()
            .enumerate()
            .min_by_key(|(index, load)| (**load, *index))
            .map(|(index, _)| index)
            .unwrap_or(0);
        columns[column].push(*id);
        loads[column] = loads[column].saturating_add(weight);
    }
    let columns = columns
        .into_iter()
        .filter(|column| !column.is_empty())
        .map(|column| {
            let children = column.into_iter().map(LayoutNode::Leaf).collect::<Vec<_>>();
            let weights = children
                .iter()
                .map(|node| demand_weight(plan, node))
                .collect();
            LayoutNode::VerticalSplit { children, weights }
        })
        .collect::<Vec<_>>();
    if columns.len() == 1 {
        columns.into_iter().next().expect("one column")
    } else {
        LayoutNode::HorizontalSplit {
            weights: columns
                .iter()
                .map(|column| layout_demand_weight(plan, column))
                .collect(),
            children: columns,
        }
    }
}

fn prune_layout(
    plan: &TuiLayoutPlan,
    node: &LayoutNode,
    selected: &HashSet<SpatialRegionId>,
) -> Option<LayoutNode> {
    match node {
        LayoutNode::Leaf(id) => selected.contains(id).then_some(LayoutNode::Leaf(*id)),
        LayoutNode::Stack(children) => {
            let children = children
                .iter()
                .filter_map(|child| prune_layout(plan, child, selected))
                .collect::<Vec<_>>();
            (!children.is_empty()).then_some(LayoutNode::Stack(children))
        }
        LayoutNode::HorizontalSplit { children, .. } => {
            let children = children
                .iter()
                .filter_map(|child| prune_layout(plan, child, selected))
                .collect::<Vec<_>>();
            (!children.is_empty()).then(|| LayoutNode::HorizontalSplit {
                weights: children
                    .iter()
                    .map(|child| layout_demand_weight(plan, child))
                    .collect(),
                children,
            })
        }
        LayoutNode::VerticalSplit { children, .. } => {
            let children = children
                .iter()
                .filter_map(|child| prune_layout(plan, child, selected))
                .collect::<Vec<_>>();
            (!children.is_empty()).then(|| LayoutNode::VerticalSplit {
                weights: children
                    .iter()
                    .map(|child| layout_demand_weight(plan, child))
                    .collect(),
                children,
            })
        }
        LayoutNode::Overlay { base, overlays } => {
            let base = prune_layout(plan, base, selected)?;
            let overlays = overlays
                .iter()
                .filter_map(|child| prune_layout(plan, child, selected))
                .collect();
            Some(LayoutNode::Overlay {
                base: Box::new(base),
                overlays,
            })
        }
    }
}

fn demand_weight(plan: &TuiLayoutPlan, node: &LayoutNode) -> u16 {
    let LayoutNode::Leaf(id) = node else {
        return 1;
    };
    plan.regions
        .iter()
        .find(|region| region.id == *id)
        .map_or(1, |region| match region.demand {
            LayoutDemand::Expand => 5,
            LayoutDemand::Supporting => 3,
            LayoutDemand::Compact => 2,
            LayoutDemand::Minimal => 1,
            LayoutDemand::Hidden => 0,
        })
}

fn layout_demand_weight(plan: &TuiLayoutPlan, node: &LayoutNode) -> u16 {
    match node {
        LayoutNode::Leaf(_) => demand_weight(plan, node),
        LayoutNode::Stack(children)
        | LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => children
            .iter()
            .map(|child| layout_demand_weight(plan, child))
            .max()
            .unwrap_or(1),
        LayoutNode::Overlay { base, overlays } => std::iter::once(base.as_ref())
            .chain(overlays.iter())
            .map(|child| layout_demand_weight(plan, child))
            .max()
            .unwrap_or(1),
    }
}

pub fn audit_presentation_coverage(
    plan: &TuiLayoutPlan,
    composition: &ResponsiveComposition,
) -> PresentationCoverageAudit {
    let represented: HashSet<_> = composition.represented.iter().copied().collect();
    let collapsed: HashSet<_> = composition.collapsed.iter().copied().collect();
    let direct: HashSet<_> = composition
        .represented
        .iter()
        .copied()
        .filter(|id| !collapsed.contains(id))
        .collect();
    let forced_collapsed: HashSet<_> = composition.forced_collapsed.iter().copied().collect();
    let persistent_regions = plan
        .regions
        .iter()
        .filter(|region| region.obligation == PresentationObligation::Persistent)
        .collect::<Vec<_>>();
    let missing = persistent_regions
        .iter()
        .filter_map(|region| (!represented.contains(&region.id)).then_some(region.id))
        .collect();
    let primary = plan
        .regions
        .iter()
        .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
        .map(|region| region.id);
    let category_count = |predicate: &dyn Fn(&SpatialRegion) -> bool| {
        let required = plan
            .regions
            .iter()
            .filter(|region| predicate(region))
            .count();
        let present = plan
            .regions
            .iter()
            .filter(|region| predicate(region) && represented.contains(&region.id))
            .count();
        (required, present)
    };
    let (persistent_inputs, persistent_inputs_represented) = category_count(&|region| {
        region.presentation.kind == RegionPresentationKind::InputSurface
            && region.obligation == PresentationObligation::Persistent
    });
    let (control_surfaces, control_surfaces_represented) = category_count(&|region| {
        region.obligation == PresentationObligation::Persistent
            && matches!(
                region.presentation.kind,
                RegionPresentationKind::ControlGroup | RegionPresentationKind::ChoiceList
            )
    });
    let (tab_contexts, tab_contexts_represented) = category_count(&|region| {
        region.kind == SpatialRegionKind::TabStrip
            && region.obligation == PresentationObligation::Persistent
    });
    let (meaningful_statuses, meaningful_statuses_represented) = category_count(&|region| {
        region.presentation.kind == RegionPresentationKind::Status
            && region.presentation.meaningful_items > 0
    });
    let pinned_regions = plan
        .regions
        .iter()
        .filter(|region| region.visibility == VisibilityGuarantee::Pinned)
        .collect::<Vec<_>>();
    PresentationCoverageAudit {
        persistent: persistent_regions.len(),
        represented: persistent_regions
            .iter()
            .filter(|region| represented.contains(&region.id))
            .count(),
        collapsed_but_reachable: persistent_regions
            .iter()
            .filter(|region| collapsed.contains(&region.id))
            .count(),
        discoverable: plan
            .regions
            .iter()
            .filter(|region| region.obligation == PresentationObligation::Discoverable)
            .filter(|region| represented.contains(&region.id))
            .count(),
        structural_suppressed: plan
            .regions
            .iter()
            .filter(|region| {
                matches!(
                    region.obligation,
                    PresentationObligation::Structural | PresentationObligation::DiagnosticOnly
                ) && !represented.contains(&region.id)
            })
            .count(),
        primary_represented: primary.is_none_or(|id| represented.contains(&id)),
        persistent_inputs,
        persistent_inputs_represented,
        control_surfaces,
        control_surfaces_represented,
        tab_contexts,
        tab_contexts_represented,
        meaningful_statuses,
        meaningful_statuses_represented,
        command_surfaces_discoverable: plan
            .regions
            .iter()
            .filter(|region| region.presentation.kind == RegionPresentationKind::CommandBar)
            .filter(|region| represented.contains(&region.id))
            .count(),
        missing,
        pinned: pinned_regions.len(),
        pinned_directly_represented: pinned_regions
            .iter()
            .filter(|region| direct.contains(&region.id))
            .count(),
        pinned_forced_to_collapse: pinned_regions
            .iter()
            .filter(|region| forced_collapsed.contains(&region.id))
            .count(),
        pinned_improperly_collapsed: pinned_regions
            .iter()
            .filter_map(|region| {
                (collapsed.contains(&region.id) && !forced_collapsed.contains(&region.id))
                    .then_some(region.id)
            })
            .collect(),
    }
}

/// Region-level navigation order derived from the already scope-filtered
/// semantic scene. Background regions blocked by a modal InteractionScope have
/// no represented source in that scene and therefore cannot enter this order.
pub fn region_focus_order(plan: &TuiLayoutPlan, scene: &TuiScene) -> Vec<SpatialRegionId> {
    let visible_sources = scene
        .elements
        .iter()
        .flat_map(|element| element.sources.iter().copied())
        .collect::<HashSet<_>>();
    plan.regions
        .iter()
        .filter(|region| {
            region.demand != LayoutDemand::Hidden
                && !matches!(
                    region.obligation,
                    PresentationObligation::Structural | PresentationObligation::DiagnosticOnly
                )
                && region
                    .presentation
                    .source_nodes
                    .iter()
                    .any(|source| visible_sources.contains(source))
        })
        .map(|region| region.id)
        .collect()
}

pub fn format_presentation_coverage(audit: &PresentationCoverageAudit) -> String {
    format!(
        "Semantic surface coverage:\nPersistent surfaces: {}\nrepresented directly or collapsed: {}\ncollapsed but reachable: {}\ndiscoverable: {}\nprimary task represented: {}\npersistent inputs: {}/{}\ncontrol surfaces: {}/{}\ntab/current contexts: {}/{}\nmeaningful statuses: {}/{}\ncommand surfaces discoverable: {}\nstructural suppressed: {}\nmissing: {:?}\nDirect surface coverage:\nPinned surfaces: {}\ndirectly represented: {}\nforced to collapse: {}\nimproperly collapsed: {:?}\n",
        audit.persistent,
        audit.represented,
        audit.collapsed_but_reachable,
        audit.discoverable,
        audit.primary_represented,
        audit.persistent_inputs_represented,
        audit.persistent_inputs,
        audit.control_surfaces_represented,
        audit.control_surfaces,
        audit.tab_contexts_represented,
        audit.tab_contexts,
        audit.meaningful_statuses_represented,
        audit.meaningful_statuses,
        audit.command_surfaces_discoverable,
        audit.structural_suppressed,
        audit.missing,
        audit.pinned,
        audit.pinned_directly_represented,
        audit.pinned_forced_to_collapse,
        audit.pinned_improperly_collapsed,
    )
}

/// Refine terminal demand from the already-compiled semantic scene.  This does
/// not alter region identity or bindings; it only distinguishes rich realized
/// payload from a temporarily empty presentation surface.
pub fn refine_layout_demands_from_scene(analysis: &mut LayoutAnalysis, scene: &TuiScene) {
    let has_command_surface = analysis.plan.regions.iter().any(|region| {
        region.presentation.kind == RegionPresentationKind::CommandBar
            && region.demand != LayoutDemand::Hidden
    });
    for region in &mut analysis.plan.regions {
        if region.demand == LayoutDemand::Hidden
            || matches!(
                region.presentation.kind,
                RegionPresentationKind::InlineContent
                    | RegionPresentationKind::InputSurface
                    | RegionPresentationKind::GraphicalPlaceholder
                    | RegionPresentationKind::CommandBar
            )
        {
            continue;
        }
        let matching = scene
            .elements
            .iter()
            .filter(|element| {
                element
                    .sources
                    .iter()
                    .any(|source| region.presentation.source_nodes.contains(source))
            })
            .collect::<Vec<_>>();
        let realized = matching
            .iter()
            .filter(|element| {
                !matches!(
                    element.kind,
                    SceneElementKind::Unsupported { .. }
                        | SceneElementKind::CommandHeader { .. }
                        | SceneElementKind::Group { .. }
                )
            })
            .count();
        if realized == 0 {
            let command_only_duplicate = has_command_surface
                && region.presentation.kind == RegionPresentationKind::ControlGroup
                && !matching.is_empty()
                && matching.iter().all(|element| {
                    matches!(
                        element.kind,
                        SceneElementKind::Unsupported { .. }
                            | SceneElementKind::CommandHeader { .. }
                            | SceneElementKind::Group { .. }
                    )
                })
                && matching
                    .iter()
                    .any(|element| matches!(element.kind, SceneElementKind::CommandHeader { .. }));
            if command_only_duplicate {
                region.presentation.kind = RegionPresentationKind::DiagnosticOnly;
                region.presentation.dominant_eligible = false;
                region.priority = PresentationPriority::HiddenByDefault;
                region.importance = LayoutImportance::Structural;
                region.obligation = PresentationObligation::DiagnosticOnly;
                region.demand = LayoutDemand::Hidden;
                region.reasons.push(
                    "command-only aggregate is discoverable through the dedicated command surface"
                        .into(),
                );
                continue;
            }
            region.demand = LayoutDemand::Minimal;
            region.reasons.push(
                "no currently realized user-facing items; collapsed to minimal demand".into(),
            );
        } else if region.demand == LayoutDemand::Expand
            && region.kind != SpatialRegionKind::PrimaryContent
            && realized <= 3
        {
            region.demand = LayoutDemand::Supporting;
            region.reasons.push(format!(
                "{realized} realized user-facing item(s) do not require an expanding workspace pane"
            ));
        } else if realized >= 4
            && matches!(
                region.presentation.kind,
                RegionPresentationKind::Form
                    | RegionPresentationKind::Table
                    | RegionPresentationKind::Navigation
                    | RegionPresentationKind::ChoiceList
                    | RegionPresentationKind::ControlGroup
                    | RegionPresentationKind::WorkspacePane
            )
            && matches!(region.demand, LayoutDemand::Compact | LayoutDemand::Minimal)
        {
            region.demand = LayoutDemand::Supporting;
            region.reasons.push(format!(
                "{realized} realized user-facing items justify supporting demand"
            ));
        }
    }
}

fn choose_primary(
    regions: &mut [SpatialRegion],
    evidence: &SpatialEvidenceIndex,
) -> Option<SpatialRegionId> {
    // Spatial/semantic content classification is only candidacy evidence.
    // Dominance requires a real user-facing RegionPresentation payload.
    for region in regions.iter_mut() {
        if region.kind == SpatialRegionKind::PrimaryContent {
            region.kind = SpatialRegionKind::Unknown;
            region.priority = PresentationPriority::Auxiliary;
            region.importance = LayoutImportance::Compact;
        }
    }
    for region in regions.iter_mut().filter(|region| {
        (matches!(
            region.semantic_kind,
            SemanticRegionKind::Content
                | SemanticRegionKind::OpaqueContent
                | SemanticRegionKind::Form
                | SemanticRegionKind::Selection
        ) || region.presentation.kind == RegionPresentationKind::Empty)
            && !region.presentation.dominant_eligible
    }) {
        region.reasons.push(format!(
            "primary candidate rejected: {:?} presentation is not dominant-capable",
            region.presentation.kind
        ));
    }

    let meaningful_peer_areas: Vec<_> = regions
        .iter()
        .filter(|region| region.presentation.kind.is_meaningful())
        .filter_map(|region| region.bounds.map(|bounds| (region.id, bounds)))
        .collect();
    let all_bounds: Vec<_> = regions
        .iter()
        .filter_map(|region| {
            region
                .bounds
                .map(|bounds| (region.id, region.coordinate_space, bounds))
        })
        .collect();
    let mut candidates: Vec<usize> = regions
        .iter()
        .enumerate()
        .filter(|(_, region)| {
            region.presentation.dominant_eligible
                && region.presentation.kind.is_meaningful()
                && region.priority != PresentationPriority::HiddenByDefault
                && match region.presentation.kind {
                    RegionPresentationKind::InlineContent => true,
                    RegionPresentationKind::GraphicalPlaceholder
                    | RegionPresentationKind::Form
                    | RegionPresentationKind::Table => {
                        let Some(bounds) = region.bounds else {
                            return false;
                        };
                        let area = bounds.area();
                        let peer = meaningful_peer_areas
                            .iter()
                            .filter(|(id, peer_bounds)| {
                                *id != region.id && !peer_bounds.contains(bounds)
                            })
                            .map(|(_, bounds)| bounds.area())
                            .max()
                            .unwrap_or(0);
                        let dominates_peer =
                            peer == 0 || area.saturating_mul(2) >= peer.saturating_mul(3);
                        if region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
                        {
                            let containing = all_bounds
                                .iter()
                                .filter(|(id, space, candidate)| {
                                    *id != region.id
                                        && *space == region.coordinate_space
                                        && candidate.contains(bounds)
                                        && candidate.area() > area
                                })
                                .map(|(_, _, bounds)| bounds.area())
                                .min();
                            dominates_peer
                                && containing
                                    .is_some_and(|container| area.saturating_mul(5) >= container)
                        } else {
                            dominates_peer
                        }
                    }
                    _ => false,
                }
        })
        .map(|(index, _)| index)
        .collect();
    candidates.sort_by_key(|index| {
        let region = &regions[*index];
        let payload_rank = match region.presentation.kind {
            RegionPresentationKind::InlineContent
            | RegionPresentationKind::GraphicalPlaceholder => 3,
            RegionPresentationKind::Table => 2,
            RegionPresentationKind::Form => 1,
            _ => 0,
        };
        (
            payload_rank,
            region.bounds.is_some(),
            region
                .bounds
                .map(|bounds| bounds.area())
                .unwrap_or_default(),
        )
    });
    let chosen_index = candidates.pop()?;
    let chosen = &mut regions[chosen_index];
    chosen.kind = SpatialRegionKind::PrimaryContent;
    chosen.priority = PresentationPriority::Primary;
    chosen.importance = LayoutImportance::Dominant;
    chosen.reasons.push(format!(
        "presentation-capable {:?} selected for dominant allocation",
        chosen.presentation.kind
    ));
    if chosen.bounds.is_none() {
        chosen
            .reasons
            .push("geometry unavailable; meaningful semantic presentation fallback".into());
    }
    if chosen.bounds.is_some()
        && evidence
            .entries
            .values()
            .any(|e| e.trust == GeometryTrust::Consistent)
    {
        chosen
            .reasons
            .push("trusted geometry available for dominance comparison".into());
    }
    Some(chosen.id)
}

fn classify_surrounding(regions: &mut [SpatialRegion], primary: SpatialRegionId) {
    let Some(p) = regions
        .iter()
        .find(|r| r.id == primary)
        .and_then(|r| r.bounds)
    else {
        return;
    };
    let space = regions
        .iter()
        .find(|r| r.id == primary)
        .map(|r| r.coordinate_space)
        .unwrap_or(CoordinateSpace::Unknown);
    for region in regions
        .iter_mut()
        .filter(|r| r.id != primary && r.priority != PresentationPriority::Structural)
    {
        let Some(bounds) = region.bounds else {
            continue;
        };
        if region.coordinate_space == CoordinateSpace::Unknown || region.coordinate_space != space {
            continue;
        }
        if bounds.contains(p) {
            continue;
        }
        let (cx, cy) = bounds.center();
        if bounds.intersects(p)
            && !p.contains(bounds)
            && matches!(
                region.kind,
                SpatialRegionKind::Dialog | SpatialRegionKind::Overlay
            )
        {
            region.priority = PresentationPriority::Secondary;
            region.importance = LayoutImportance::Supporting;
            region
                .reasons
                .push("semantic dialog/overlay overlaps primary content".into());
        } else if bounds.width > p.width / 2 && bounds.height < p.height / 2 && cy < p.y {
            region.kind = if matches!(region.semantic_kind, SemanticRegionKind::CommandSet) {
                SpatialRegionKind::Toolbar
            } else {
                SpatialRegionKind::TabStrip
            };
            region.priority = PresentationPriority::Secondary;
            region.importance = LayoutImportance::Supporting;
            region
                .reasons
                .push("thin horizontal band directly above primary content".into());
        } else if bounds.width > p.width / 2 && bounds.height < p.height / 2 && cy > p.bottom() {
            region.kind = SpatialRegionKind::Status;
            region.priority = PresentationPriority::Secondary;
            region.importance = LayoutImportance::Supporting;
            region
                .reasons
                .push("thin horizontal band below primary content".into());
        } else if bounds.height > p.height / 2 && bounds.width < p.width / 2 && cx < p.x {
            region.kind = SpatialRegionKind::Sidebar;
            region.priority = PresentationPriority::Auxiliary;
            region.importance = LayoutImportance::Compact;
            region
                .reasons
                .push("tall narrow region left of primary content".into());
        } else if bounds.height > p.height / 2 && bounds.width < p.width / 2 && cx > p.right() {
            region.kind = SpatialRegionKind::Auxiliary;
            region.priority = PresentationPriority::Auxiliary;
            region.importance = LayoutImportance::Compact;
            region
                .reasons
                .push("tall narrow region right of primary content".into());
        }
    }
}

fn suppress_untrusted_secondary_graphics(
    regions: &mut [SpatialRegion],
    primary: SpatialRegionId,
    semantic_regions: &[&SemanticRegion],
) {
    let primary_is_trusted_graphical = regions.iter().any(|region| {
        region.id == primary
            && region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && region.bounds.is_some()
            && region.coordinate_space != CoordinateSpace::Unknown
    });
    if !primary_is_trusted_graphical {
        return;
    }
    for region in regions.iter_mut().filter(|region| {
        region.id != primary
            && region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && (region.bounds.is_none() || region.coordinate_space == CoordinateSpace::Unknown)
    }) {
        let has_interaction = region.presentation.source_regions.iter().any(|source| {
            semantic_regions.iter().any(|semantic| {
                semantic.id == *source
                    && (!semantic.interactions.is_empty()
                        || semantic.children.iter().any(|child| {
                            !child.interactions.is_empty() || descendant_interactions(child) > 0
                        }))
            })
        });
        if has_interaction {
            continue;
        }
        region.presentation.kind = RegionPresentationKind::DiagnosticOnly;
        region.presentation.dominant_eligible = false;
        region.presentation.reasons.push(
            "untrusted non-interactive graphical peer is diagnostic beside a trusted graphical primary"
                .into(),
        );
        region.priority = PresentationPriority::HiddenByDefault;
        region.importance = LayoutImportance::Structural;
    }
}

fn overall_trust(index: &SpatialEvidenceIndex) -> GeometryTrust {
    if index.entries.is_empty() {
        return GeometryTrust::Unavailable;
    }
    let valid = index
        .entries
        .values()
        .filter(|e| {
            e.bounds.is_some()
                && matches!(e.trust, GeometryTrust::Consistent | GeometryTrust::Partial)
        })
        .count();
    if valid == 0
        && index
            .entries
            .values()
            .any(|e| e.trust == GeometryTrust::Inconsistent)
    {
        return GeometryTrust::Inconsistent;
    }
    if index
        .entries
        .values()
        .any(|e| e.trust == GeometryTrust::Consistent)
    {
        GeometryTrust::Consistent
    } else if index.entries.values().any(|e| e.bounds.is_some()) {
        GeometryTrust::Partial
    } else {
        GeometryTrust::Unavailable
    }
}

fn kind_for_semantics(kind: SemanticRegionKind) -> SpatialRegionKind {
    match kind {
        SemanticRegionKind::Content | SemanticRegionKind::OpaqueContent => {
            SpatialRegionKind::Unknown
        }
        SemanticRegionKind::CommandSet => SpatialRegionKind::Toolbar,
        SemanticRegionKind::Selection => SpatialRegionKind::Navigation,
        SemanticRegionKind::Status => SpatialRegionKind::Status,
        SemanticRegionKind::Form => SpatialRegionKind::Auxiliary,
        _ => SpatialRegionKind::Unknown,
    }
}
fn priority_for_semantics(kind: SemanticRegionKind) -> PresentationPriority {
    match kind {
        SemanticRegionKind::Content | SemanticRegionKind::OpaqueContent => {
            PresentationPriority::Auxiliary
        }
        SemanticRegionKind::Status | SemanticRegionKind::CommandSet => {
            PresentationPriority::Secondary
        }
        _ => PresentationPriority::Auxiliary,
    }
}

fn importance_for(priority: PresentationPriority) -> LayoutImportance {
    match priority {
        PresentationPriority::Primary => LayoutImportance::Dominant,
        PresentationPriority::Secondary => LayoutImportance::Supporting,
        PresentationPriority::Auxiliary | PresentationPriority::HiddenByDefault => {
            LayoutImportance::Compact
        }
        PresentationPriority::Structural => LayoutImportance::Structural,
    }
}

fn initial_surface_policy(
    region: &SemanticRegion,
    source: Option<&SemanticNode>,
    presentation: &RegionPresentation,
    structural: bool,
) -> (PresentationObligation, LayoutDemand, InteractionPurpose) {
    if structural || presentation.kind == RegionPresentationKind::Structural {
        return (
            PresentationObligation::Structural,
            LayoutDemand::Hidden,
            InteractionPurpose::Unknown,
        );
    }
    if presentation.kind == RegionPresentationKind::DiagnosticOnly {
        return (
            PresentationObligation::DiagnosticOnly,
            LayoutDemand::Hidden,
            InteractionPurpose::Unknown,
        );
    }
    if presentation.kind == RegionPresentationKind::Empty {
        return (
            PresentationObligation::Contextual,
            LayoutDemand::Minimal,
            InteractionPurpose::Unknown,
        );
    }
    let purpose = infer_interaction_purpose(region, source);
    match presentation.kind {
        RegionPresentationKind::InlineContent => (
            PresentationObligation::Persistent,
            LayoutDemand::Expand,
            purpose,
        ),
        RegionPresentationKind::InputSurface => (
            PresentationObligation::Persistent,
            LayoutDemand::Compact,
            purpose,
        ),
        RegionPresentationKind::GraphicalPlaceholder => (
            PresentationObligation::Persistent,
            LayoutDemand::Supporting,
            purpose,
        ),
        RegionPresentationKind::CommandBar => (
            PresentationObligation::Discoverable,
            LayoutDemand::Compact,
            purpose,
        ),
        RegionPresentationKind::Status => (
            PresentationObligation::Contextual,
            LayoutDemand::Compact,
            purpose,
        ),
        RegionPresentationKind::Navigation | RegionPresentationKind::ChoiceList => (
            PresentationObligation::Contextual,
            LayoutDemand::Supporting,
            purpose,
        ),
        RegionPresentationKind::Form
        | RegionPresentationKind::Table
        | RegionPresentationKind::WorkspacePane => (
            PresentationObligation::Contextual,
            if presentation.meaningful_items == 0 {
                LayoutDemand::Minimal
            } else {
                LayoutDemand::Supporting
            },
            purpose,
        ),
        RegionPresentationKind::ControlGroup => (
            PresentationObligation::Contextual,
            LayoutDemand::Compact,
            purpose,
        ),
        RegionPresentationKind::CollapsedSummary => (
            PresentationObligation::Contextual,
            LayoutDemand::Minimal,
            purpose,
        ),
        RegionPresentationKind::Structural
        | RegionPresentationKind::DiagnosticOnly
        | RegionPresentationKind::Empty => unreachable!(),
    }
}

fn infer_interaction_purpose(
    region: &SemanticRegion,
    source: Option<&SemanticNode>,
) -> InteractionPurpose {
    if matches!(
        source.map(|node| &node.role),
        Some(SemanticRole::TabList | SemanticRole::Tab)
    ) {
        return InteractionPurpose::Navigate;
    }
    if matches!(
        source.map(|node| &node.role),
        Some(SemanticRole::List | SemanticRole::Tree | SemanticRole::TreeItem)
    ) {
        return InteractionPurpose::Select;
    }
    let mut text = String::new();
    if let Some(label) = &region.label {
        text.push_str(label);
        text.push(' ');
    }
    if let Some(source) = source {
        if let Some(name) = &source.name {
            text.push_str(name);
            text.push(' ');
        }
        if let Some(description) = &source.description {
            text.push_str(description);
        }
    }
    let words: HashSet<_> = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    if words
        .iter()
        .any(|word| matches!(word.as_str(), "search" | "find" | "query"))
    {
        InteractionPurpose::Search
    } else if words.contains("filter") {
        InteractionPurpose::Filter
    } else if words
        .iter()
        .any(|word| matches!(word.as_str(), "address" | "url" | "navigate"))
    {
        InteractionPurpose::Navigate
    } else if source.is_some_and(|node| {
        node.role == SemanticRole::TextInput
            && node.text_input_kind == Some(TextInputKind::Plain)
            && node.capabilities.contains(&SemanticCapability::EditText)
    }) {
        InteractionPurpose::Input
    } else if region.kind == SemanticRegionKind::Selection {
        InteractionPurpose::Select
    } else {
        InteractionPurpose::Unknown
    }
}

fn infer_input_purpose(title: &str, source: &SemanticNode) -> InteractionPurpose {
    let text = format!(
        "{} {} {}",
        title,
        source.name.as_deref().unwrap_or_default(),
        source.description.as_deref().unwrap_or_default()
    );
    let words = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<HashSet<_>>();
    if words
        .iter()
        .any(|word| matches!(word.as_str(), "search" | "find" | "query"))
    {
        InteractionPurpose::Search
    } else if words.contains("filter") {
        InteractionPurpose::Filter
    } else if words
        .iter()
        .any(|word| matches!(word.as_str(), "address" | "url" | "navigate"))
    {
        InteractionPurpose::Navigate
    } else {
        InteractionPurpose::Input
    }
}

fn collect_node_parents(root: &SemanticNode) -> HashMap<RuntimeNodeId, RuntimeNodeId> {
    fn visit(node: &SemanticNode, output: &mut HashMap<RuntimeNodeId, RuntimeNodeId>) {
        for child in &node.children {
            output.insert(child.runtime_id, node.runtime_id);
            visit(child, output);
        }
    }
    let mut output = HashMap::new();
    visit(root, &mut output);
    output
}

fn descends_from_any(
    node: RuntimeNodeId,
    ancestors: &HashSet<RuntimeNodeId>,
    parents: &HashMap<RuntimeNodeId, RuntimeNodeId>,
) -> bool {
    std::iter::successors(Some(node), |current| parents.get(current).copied())
        .any(|current| ancestors.contains(&current))
}

fn refine_surface_policy(
    regions: &mut [SpatialRegion],
    nodes: &HashMap<RuntimeNodeId, &SemanticNode>,
    root: &SemanticNode,
    primary: Option<SpatialRegionId>,
) {
    let parents = collect_node_parents(root);
    let primary_sources: HashSet<_> = primary
        .and_then(|id| regions.iter().find(|region| region.id == id))
        .map(|region| region.presentation.source_nodes.iter().copied().collect())
        .unwrap_or_default();
    let has_distinguishable_graphical_anchor = regions.iter().any(|region| {
        region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && (region.bounds.is_some()
                || region.semantic_kind == SemanticRegionKind::OpaqueContent
                || region
                    .presentation
                    .source_nodes
                    .iter()
                    .filter_map(|id| nodes.get(id).copied())
                    .any(|node| {
                        node.name
                            .as_deref()
                            .is_some_and(|name| !name.trim().is_empty())
                            || node
                                .description
                                .as_deref()
                                .is_some_and(|description| !description.trim().is_empty())
                            || !node.actions.is_empty()
                    }))
    });
    for region in regions {
        if region.presentation.kind == RegionPresentationKind::Structural
            || region.priority == PresentationPriority::Structural
        {
            region.obligation = PresentationObligation::Structural;
            region.demand = LayoutDemand::Hidden;
            region.visibility = VisibilityGuarantee::DiscoverableOnly;
            continue;
        }
        if region.presentation.kind == RegionPresentationKind::DiagnosticOnly
            || region.priority == PresentationPriority::HiddenByDefault
        {
            region.obligation = PresentationObligation::DiagnosticOnly;
            region.demand = LayoutDemand::Hidden;
            region.visibility = VisibilityGuarantee::DiscoverableOnly;
            continue;
        }
        if region.presentation.kind == RegionPresentationKind::Empty {
            // Focus/current state can raise the visibility of a presentable
            // surface, but it cannot turn a payload-less semantic owner into
            // a persistent surface that responsive composition cannot render.
            region.obligation = PresentationObligation::Contextual;
            region.demand = LayoutDemand::Minimal;
            region.visibility = VisibilityGuarantee::Collapsible;
            continue;
        }
        let source_nodes = region
            .presentation
            .source_nodes
            .iter()
            .filter_map(|id| nodes.get(id).copied())
            .collect::<Vec<_>>();
        let indistinguishable_noninteractive_image = !source_nodes.is_empty()
            && region
                .presentation
                .title
                .eq_ignore_ascii_case(presentation_default_title(
                    RegionPresentationKind::GraphicalPlaceholder,
                ))
            && source_nodes.iter().all(|node| {
                matches!(node.role, SemanticRole::Image | SemanticRole::Video)
                    && !node
                        .actions
                        .iter()
                        .any(|action| !action.name.trim().is_empty())
            });
        let unanchored_image = has_distinguishable_graphical_anchor
            && region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
            && region.bounds.is_none()
            && region.semantic_kind != SemanticRegionKind::OpaqueContent
            && indistinguishable_noninteractive_image;
        if unanchored_image {
            region.presentation.kind = RegionPresentationKind::DiagnosticOnly;
            region.presentation.dominant_eligible = false;
            region.priority = PresentationPriority::HiddenByDefault;
            region.importance = LayoutImportance::Structural;
            region.obligation = PresentationObligation::DiagnosticOnly;
            region.demand = LayoutDemand::Hidden;
            region.visibility = VisibilityGuarantee::DiscoverableOnly;
            region.reasons.push(
                "unlabelled unbounded image is diagnostic beside a distinguishable graphical surface"
                    .into(),
            );
            continue;
        }
        if Some(region.id) == primary {
            region.obligation = PresentationObligation::Persistent;
            region.demand =
                if region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder {
                    LayoutDemand::Supporting
                } else {
                    LayoutDemand::Expand
                };
            region.reasons.push(format!(
                "primary surface policy: {:?} / {:?}",
                region.obligation, region.demand
            ));
            continue;
        }
        let has_current_state = source_nodes.iter().any(|node| {
            node.states
                .iter()
                .any(|state| matches!(state, SemanticState::Focused | SemanticState::Selected))
        });
        let persistent_input = source_nodes.iter().find(|node| {
            is_presentable_single_line_input(node)
                && !descends_from_any(node.runtime_id, &primary_sources, &parents)
                && (region.bounds.is_some() || node.states.contains(&SemanticState::Focused))
        });
        let tab_context = source_nodes.iter().any(|node| {
            node.role == SemanticRole::Tab
                && node.states.contains(&SemanticState::Selected)
                && node
                    .name
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty())
        });
        let tab_sources = source_nodes
            .iter()
            .filter(|node| matches!(node.role, SemanticRole::Tab | SemanticRole::TabList))
            .map(|node| node.runtime_id)
            .collect::<HashSet<_>>();
        let named_tab = source_nodes.iter().any(|node| {
            node.role == SemanticRole::Tab
                && node
                    .name
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty())
        });
        let tab_count = source_nodes
            .iter()
            .filter(|node| node.role == SemanticRole::Tab)
            .count();
        let interactive_tab_surface = source_nodes
            .iter()
            .any(|node| node.role == SemanticRole::Tab && !node.actions.is_empty())
            || (tab_count > 1
                && source_nodes.iter().any(|node| {
                    node.role == SemanticRole::TabList
                        && node
                            .capabilities
                            .contains(&SemanticCapability::SelectChildren)
                }));
        let unnamed_tab_wraps_primary = !tab_sources.is_empty()
            && !named_tab
            && !interactive_tab_surface
            && primary_sources
                .iter()
                .any(|source| descends_from_any(*source, &tab_sources, &parents));
        let semantic_control_band = source_nodes.iter().any(|node| {
            matches!(node.role, SemanticRole::MenuBar)
                || (node.role == SemanticRole::Container
                    && region.presentation.kind == RegionPresentationKind::ControlGroup
                    && region.presentation.meaningful_items > 1)
        });
        if unnamed_tab_wraps_primary {
            region.presentation.kind = RegionPresentationKind::DiagnosticOnly;
            region.presentation.dominant_eligible = false;
            region.priority = PresentationPriority::HiddenByDefault;
            region.importance = LayoutImportance::Structural;
            region.obligation = PresentationObligation::DiagnosticOnly;
            region.demand = LayoutDemand::Hidden;
            region.visibility = VisibilityGuarantee::DiscoverableOnly;
            region.reasons.push(
                "unnamed non-interactive tab wrapper is already represented by its primary descendant"
                    .into(),
            );
        } else if let Some(input) = persistent_input {
            region.presentation.kind = RegionPresentationKind::InputSurface;
            region.purpose = infer_input_purpose(&region.presentation.title, input);
            if matches!(
                region.presentation.title.as_str(),
                "Form" | "Controls" | "Details" | "Commands" | "Navigation"
            ) {
                region.presentation.title = input
                    .name
                    .as_deref()
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        match region.purpose {
                            InteractionPurpose::Search => "Search",
                            InteractionPurpose::Filter => "Filter",
                            InteractionPurpose::Navigate => "Navigation",
                            _ => "Input",
                        }
                        .to_owned()
                    });
            }
            region.obligation = PresentationObligation::Persistent;
            region.demand = LayoutDemand::Compact;
            region.visibility = VisibilityGuarantee::Pinned;
            region.reasons.push(
                "persistent input: editable single-line control outside dominant content subtree"
                    .into(),
            );
        } else if tab_context {
            if let Some(current) = source_nodes.iter().find(|node| {
                node.role == SemanticRole::Tab
                    && node.states.contains(&SemanticState::Selected)
                    && node
                        .name
                        .as_deref()
                        .is_some_and(|name| !name.trim().is_empty())
            }) {
                region.presentation.title =
                    format!("Current · {}", current.name.as_deref().unwrap_or("Tab"));
            }
            region.obligation = PresentationObligation::Persistent;
            region.demand = LayoutDemand::Compact;
            region.visibility = VisibilityGuarantee::Pinned;
            region.purpose = InteractionPurpose::Navigate;
            region
                .reasons
                .push("current tab/context surface is persistently represented".into());
        } else if has_current_state {
            region.obligation = PresentationObligation::Persistent;
            if region.demand == LayoutDemand::Minimal {
                region.demand = LayoutDemand::Compact;
            }
            region.visibility = VisibilityGuarantee::PreferDirect;
            region
                .reasons
                .push("focused/current semantic state raises presentation obligation".into());
        } else if semantic_control_band {
            region.obligation = PresentationObligation::Persistent;
            region.demand = LayoutDemand::Compact;
            region.visibility = VisibilityGuarantee::PreferDirect;
            region
                .reasons
                .push("coherent multi-control semantic surface is persistent".into());
        }
    }
}

fn refine_policy_from_topology(
    regions: &mut [SpatialRegion],
    topology: &SpatialTopology,
    primary: Option<SpatialRegionId>,
) {
    let Some(primary) = primary else {
        return;
    };
    for region in regions.iter_mut().filter(|region| region.id != primary) {
        let peripheral = topology.relates(region.id, primary, TopologyRelationKind::PeripheralTo);
        let band = topology.normalized.get(&region.id).is_some_and(|bounds| {
            (bounds.height <= 180 && bounds.width >= 450)
                || (bounds.width <= 220 && bounds.height >= 450)
        });
        if peripheral
            && band
            && matches!(
                region.presentation.kind,
                RegionPresentationKind::ControlGroup
                    | RegionPresentationKind::CommandBar
                    | RegionPresentationKind::Navigation
                    | RegionPresentationKind::ChoiceList
            )
            && region.presentation.meaningful_items > 0
        {
            if region.obligation != PresentationObligation::Discoverable {
                region.obligation = PresentationObligation::Persistent;
            }
            region.demand = LayoutDemand::Compact;
            if region.visibility != VisibilityGuarantee::Pinned {
                region.visibility = VisibilityGuarantee::PreferDirect;
            }
            region.reasons.push(
                "compact semantic control surface is peripheral to the dominant region".into(),
            );
        }
    }
}
fn is_structural_region(region: &SemanticRegion, source: Option<&&SemanticNode>) -> bool {
    if !region.interactions.is_empty() {
        return false;
    }
    let Some(node) = source else {
        return false;
    };
    match &node.role {
        SemanticRole::Unknown(name) => {
            let name = name.to_ascii_lowercase();
            [
                "separator",
                "split pane",
                "splitpane",
                "resize handle",
                "resizehandle",
                "layered pane",
                "layeredpane",
            ]
            .iter()
            .any(|part| name.contains(part))
        }
        _ => false,
    }
}

fn collect_node_list_with_depth<'a>(
    node: &'a SemanticNode,
    depth: usize,
    output: &mut Vec<(&'a SemanticNode, usize)>,
) {
    output.push((node, depth));
    for child in &node.children {
        collect_node_list_with_depth(child, depth.saturating_add(1), output);
    }
}
fn collect_node_map<'a>(
    root: &'a SemanticNode,
    output: &mut HashMap<RuntimeNodeId, &'a SemanticNode>,
) {
    fn walk<'a>(n: &'a SemanticNode, o: &mut HashMap<RuntimeNodeId, &'a SemanticNode>) {
        o.insert(n.runtime_id, n);
        for c in &n.children {
            walk(c, o);
        }
    }
    walk(root, output);
}
fn flatten_regions<'a>(region: &'a SemanticRegion, output: &mut Vec<&'a SemanticRegion>) {
    output.push(region);
    for child in &region.children {
        flatten_regions(child, output);
    }
}
fn is_candidate(node: &SemanticNode) -> bool {
    !node.children.is_empty()
        || matches!(
            node.role,
            SemanticRole::Document
                | SemanticRole::Window
                | SemanticRole::Dialog
                | SemanticRole::Container
                | SemanticRole::MenuBar
                | SemanticRole::List
                | SemanticRole::Tree
                | SemanticRole::Table
                | SemanticRole::TabList
                | SemanticRole::Image
                | SemanticRole::StatusBar
        )
        || (node.role == SemanticRole::TextInput
            && node
                .states
                .iter()
                .any(|state| matches!(state, crate::semantic::SemanticState::Other(value) if value == "multi-line")))
        || matches!(&node.role, SemanticRole::Unknown(name) if {
            let name = name.to_ascii_lowercase();
            ["drawing", "canvas", "viewport", "graphics", "view"].iter().any(|part| name.contains(part))
        })
}
fn is_spatial_anchor(node: &SemanticNode) -> bool {
    !node.actions.is_empty()
        || node.capabilities.contains(&SemanticCapability::EditText)
        || node
            .states
            .iter()
            .any(|state| matches!(state, SemanticState::Focused | SemanticState::Selected))
}
fn is_high_value_spatial_anchor(node: &SemanticNode) -> bool {
    is_presentable_single_line_input(node)
        || node
            .states
            .iter()
            .any(|state| matches!(state, SemanticState::Focused | SemanticState::Selected))
}
fn candidate_priority(node: &SemanticNode, depth: usize) -> (usize, usize, usize, usize, usize) {
    let role = match node.role {
        SemanticRole::Window | SemanticRole::Dialog => 5,
        SemanticRole::Document | SemanticRole::Image | SemanticRole::Table => 4,
        SemanticRole::MenuBar | SemanticRole::TabList | SemanticRole::Tree => 3,
        SemanticRole::Container | SemanticRole::List | SemanticRole::Form => 2,
        SemanticRole::TextInput => 1,
        _ => 0,
    };
    let current = node
        .states
        .iter()
        .any(|state| matches!(state, SemanticState::Focused | SemanticState::Selected))
        as usize;
    let editable = is_presentable_single_line_input(node) as usize;
    let shallow = usize::MAX.saturating_sub(depth);
    (
        current,
        editable,
        role,
        usize::from(depth <= 2),
        shallow.saturating_add(node.children.len()),
    )
}
fn count_leaves(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf(_) => 1,
        LayoutNode::Stack(c)
        | LayoutNode::HorizontalSplit { children: c, .. }
        | LayoutNode::VerticalSplit { children: c, .. } => c.iter().map(count_leaves).sum(),
        LayoutNode::Overlay { base, overlays } => {
            count_leaves(base) + overlays.iter().map(count_leaves).sum::<usize>()
        }
    }
}

pub fn audit_layout_reachability(
    plan: &TuiLayoutPlan,
    analysis: &super::analyze::RegionAnalysis,
) -> LayoutReachabilityAudit {
    let placed: HashSet<_> = plan_leaves(&plan.root).into_iter().collect();
    let mut actionable = 0;
    let mut unplaced = Vec::new();
    let mut regions = Vec::new();
    flatten_regions(&analysis.root, &mut regions);
    for region in regions {
        if !region.interactions.is_empty() {
            actionable += 1;
            let reachable = plan.regions.iter().any(|spatial| {
                placed.contains(&spatial.id)
                    && spatial.presentation.source_regions.contains(&region.id)
            });
            if !reachable {
                unplaced.push(region.id);
            }
        }
    }
    LayoutReachabilityAudit {
        actionable_regions: actionable,
        placed_regions: actionable.saturating_sub(unplaced.len()),
        unplaced,
    }
}
fn plan_leaves(node: &LayoutNode) -> Vec<SpatialRegionId> {
    match node {
        LayoutNode::Leaf(id) => vec![*id],
        LayoutNode::Stack(c)
        | LayoutNode::HorizontalSplit { children: c, .. }
        | LayoutNode::VerticalSplit { children: c, .. } => c.iter().flat_map(plan_leaves).collect(),
        LayoutNode::Overlay { base, overlays } => {
            let mut out = plan_leaves(base);
            out.extend(overlays.iter().flat_map(plan_leaves));
            out
        }
    }
}

pub fn format_spatial_evidence(index: &SpatialEvidenceIndex) -> String {
    let mut out = format!(
        "SpatialEvidence generation={} nodes={} candidates={} requests={} successes={} failures={} rejected={} elapsed_ms={:.3}\n",
        index.generation.0,
        index.metrics.nodes,
        index.metrics.candidate_nodes,
        index.metrics.geometry_requests,
        index.metrics.geometry_successes,
        index.metrics.geometry_failures,
        index.metrics.geometry_rejected,
        index.metrics.elapsed.as_secs_f64() * 1000.0
    );
    let mut ids: Vec<_> = index.entries.keys().copied().collect();
    ids.sort();
    for id in ids {
        let e = &index.entries[&id];
        out.push_str(&format!(
            "  runtime={} bounds={:?} space={} trust={} visible={} showing={}\n",
            id, e.bounds, e.coordinate_space, e.trust, e.visible, e.showing
        ));
    }
    out
}
pub fn format_layout_plan(analysis: &LayoutAnalysis) -> String {
    fn write(node: &LayoutNode, depth: usize, out: &mut String) {
        out.push_str(&format!("{}{:?}\n", "  ".repeat(depth), node));
    }
    let mut out = format!(
        "LayoutPlan generation={} trust={:?} composition={:?} regions={} leaves={} primary={} structural={} topology_anchors={} topology_pairs={} topology_relations={} surface_ms={:.3} topology_ms={:.3} composition_ms={:.3} layout_ms={:.3}\n",
        analysis.plan.generation.0,
        analysis.plan.geometry_trust,
        analysis.plan.composition,
        analysis.metrics.regions,
        analysis.metrics.leaves,
        analysis.metrics.primary,
        analysis.metrics.structural,
        analysis.plan.topology.anchors,
        analysis.plan.topology.comparable_pairs,
        analysis.plan.topology.relations.len(),
        analysis.metrics.surface_inference.as_secs_f64() * 1000.0,
        analysis.metrics.topology_inference.as_secs_f64() * 1000.0,
        analysis.metrics.composition_planning.as_secs_f64() * 1000.0,
        analysis.metrics.layout_compilation.as_secs_f64() * 1000.0,
    );
    for r in &analysis.plan.regions {
        out.push_str(&format!(
            "  region={} semantic={} kind={:?} priority={:?} obligation={:?} demand={:?} visibility={:?} purpose={:?} bounds={:?} space={} normalized={:?}\n    presentation={:?} title={:?} items={} dominant_eligible={} sources={:?} nodes={:?}\n    presentation_reasons: {}\n    reasons: {}\n",
            r.id.0,
            r.semantic_region,
            r.kind,
            r.priority,
            r.obligation,
            r.demand,
            r.visibility,
            r.purpose,
            r.bounds,
            r.coordinate_space,
            analysis.plan.topology.normalized.get(&r.id),
            r.presentation.kind,
            r.presentation.title,
            r.presentation.meaningful_items,
            r.presentation.dominant_eligible,
            r.presentation.source_regions,
            r.presentation.source_nodes,
            r.presentation.reasons.join("; "),
            r.reasons.join("; ")
        ));
    }
    write(&analysis.plan.root, 1, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        semantic::{
            BackendLocator, DebugInfo, SemanticAction, SemanticCapability, SemanticNode,
            SemanticState, TextInputKind,
        },
        transcompile::{PresentationStrategy, SceneElement, SceneElementId, analyze_regions},
    };
    fn node(
        id: u64,
        role: SemanticRole,
        name: &str,
        geometry: Option<(i32, i32, i32, i32)>,
    ) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.into()),
            description: None,
            value: None,
            text_input_kind: None::<TextInputKind>,
            states: vec![SemanticState::Enabled],
            actions: Vec::<SemanticAction>::new(),
            capabilities: Vec::<SemanticCapability>::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo {
                geometry: geometry.map(|(x, y, w, h)| Geometry {
                    x,
                    y,
                    width: w,
                    height: h,
                }),
                ..Default::default()
            },
        }
    }

    fn actionable(mut node: SemanticNode) -> SemanticNode {
        node.actions.push(SemanticAction {
            index: 0,
            name: "Click".into(),
            description: None,
            keybinding: None,
        });
        node
    }
    #[test]
    fn dominant_content_and_left_sidebar_form_horizontal_plan() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        let mut side = node(1, SemanticRole::List, "nav", Some((0, 0, 200, 700)));
        let content = node(2, SemanticRole::Document, "doc", Some((200, 0, 800, 700)));
        side.index_in_parent = Some(0);
        root.children = vec![side, content];
        let ra = analyze_regions(&root);
        let idx = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(1),
            SpatialProbeBudget { max_candidates: 8 },
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 3);
        let plan = infer_layout_with_presentations(&ra, &root, &idx, Some(&presentation));
        assert_eq!(plan.metrics.primary, 1);
        assert!(matches!(plan.plan.root, LayoutNode::HorizontalSplit { .. }));
        assert!(
            plan.plan
                .regions
                .iter()
                .any(|r| r.kind == SpatialRegionKind::PrimaryContent)
        );
    }

    #[test]
    fn bounded_probe_prioritizes_deep_persistent_input_anchor() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        for id in 1..=8 {
            root.children.push(node(
                id,
                SemanticRole::Container,
                &format!("container {id}"),
                Some((0, id as i32 * 20, 1000, 20)),
            ));
        }
        let mut level_one = node(20, SemanticRole::Container, "", None);
        let mut level_two = node(21, SemanticRole::Container, "", None);
        let mut level_three = node(22, SemanticRole::Container, "", None);
        level_three.children.push(plain_input(
            99,
            "Application input",
            Some((100, 10, 700, 30)),
        ));
        level_two.children.push(level_three);
        level_one.children.push(level_two);
        root.children.push(level_one);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(32),
            SpatialProbeBudget { max_candidates: 3 },
        );
        assert!(evidence.get(RuntimeNodeId::new(99)).is_some());
        assert!(evidence.metrics.candidate_nodes <= 3);
    }

    #[test]
    fn missing_geometry_falls_back_to_semantic_stack() {
        let mut root = node(0, SemanticRole::Window, "app", None);
        root.children
            .push(node(1, SemanticRole::Document, "doc", None));
        let ra = analyze_regions(&root);
        let idx = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(2),
            SpatialProbeBudget::default(),
        );
        let plan = infer_layout(&ra, &root, &idx);
        assert_eq!(plan.plan.geometry_trust, GeometryTrust::Unavailable);
        assert!(matches!(plan.plan.root, LayoutNode::Stack(_)));
    }
    #[test]
    fn stale_generation_is_not_accepted() {
        let mut idx = SpatialEvidenceIndex::from_tree(
            &node(0, SemanticRole::Window, "w", Some((0, 0, 10, 10))),
            ApplicationGenerationId(3),
            SpatialProbeBudget::default(),
        );
        idx.insert(SpatialEvidence::from_geometry(
            RuntimeNodeId::new(99),
            None,
            ApplicationGenerationId(4),
        ));
        assert!(idx.get(RuntimeNodeId::new(99)).is_none());
    }

    #[test]
    fn coordinate_space_mismatch_disables_relative_inference() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        root.children
            .push(node(1, SemanticRole::List, "nav", Some((0, 0, 200, 700))));
        root.children.push(node(
            2,
            SemanticRole::Document,
            "doc",
            Some((200, 0, 800, 700)),
        ));
        let ra = analyze_regions(&root);
        let mut idx = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(5),
            SpatialProbeBudget::default(),
        );
        idx.entries
            .get_mut(&RuntimeNodeId::new(1))
            .unwrap()
            .coordinate_space = CoordinateSpace::Parent;
        let plan = infer_layout(&ra, &root, &idx);
        assert!(matches!(
            plan.plan.root,
            LayoutNode::Leaf(_) | LayoutNode::Stack(_)
        ));
    }

    #[test]
    fn toolbar_above_content_is_a_vertical_split_with_explanation() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 800)));
        root.children.push(node(
            1,
            SemanticRole::MenuBar,
            "tools",
            Some((0, 0, 1000, 60)),
        ));
        root.children.push(node(
            2,
            SemanticRole::Document,
            "doc",
            Some((0, 60, 1000, 740)),
        ));
        let ra = analyze_regions(&root);
        let idx = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(6),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 2);
        let plan = infer_layout_with_presentations(&ra, &root, &idx, Some(&presentation));
        assert!(matches!(plan.plan.root, LayoutNode::VerticalSplit { .. }));
        assert!(
            plan.plan
                .regions
                .iter()
                .any(|r| r.kind == SpatialRegionKind::Toolbar)
        );
    }

    #[test]
    fn structural_unknown_is_retained_for_layout_but_has_no_task_priority() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 400, 300)));
        root.children.push(node(
            1,
            SemanticRole::Unknown("separator".into()),
            "",
            Some((190, 0, 2, 300)),
        ));
        root.children.push(node(
            2,
            SemanticRole::Document,
            "doc",
            Some((0, 0, 400, 300)),
        ));
        let ra = analyze_regions(&root);
        let idx = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(7),
            SpatialProbeBudget::default(),
        );
        let plan = infer_layout(&ra, &root, &idx);
        assert!(plan.plan.regions.iter().any(|r| {
            r.kind == SpatialRegionKind::Structural
                && r.priority == PresentationPriority::Structural
        }));
    }

    #[test]
    fn multiline_text_uses_semantic_primary_fallback_without_geometry() {
        let mut root = node(0, SemanticRole::Window, "app", None);
        let mut document = node(1, SemanticRole::TextInput, "", None);
        document.value = Some("first line\nsecond line".into());
        document.text_input_kind = Some(TextInputKind::Plain);
        root.children.push(document);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(8),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(1), "Document", 1);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let primary = layout
            .plan
            .regions
            .iter()
            .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
            .expect("multiline semantic content should remain visible in the main scene");
        assert_eq!(primary.importance, LayoutImportance::Dominant);
        assert!(
            primary
                .reasons
                .iter()
                .any(|reason| reason.contains("meaningful semantic presentation fallback"))
        );
    }

    #[test]
    fn empty_content_owner_is_not_selected_as_primary() {
        let mut root = node(0, SemanticRole::Window, "workspace", Some((0, 0, 900, 700)));
        let mut empty = node(
            1,
            SemanticRole::Document,
            "empty content owner",
            Some((0, 0, 900, 700)),
        );
        empty.states.push(SemanticState::Focused);
        root.children.push(empty);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(9),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(1), "Document", 0);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        assert_eq!(layout.metrics.primary, 0);
        assert!(layout.plan.regions.iter().any(|region| {
            region.presentation.kind == RegionPresentationKind::Empty
                && region.obligation != PresentationObligation::Persistent
                && region
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("primary candidate rejected"))
        }));
    }

    #[test]
    fn multiple_peer_controls_form_a_valid_no_primary_composition() {
        let mut root = node(0, SemanticRole::Window, "workspace", Some((0, 0, 900, 600)));
        let mut left = node(1, SemanticRole::Table, "Inputs", Some((0, 0, 400, 600)));
        left.children
            .push(node(3, SemanticRole::Text, "Input row", None));
        let mut right = node(2, SemanticRole::Table, "Outputs", Some((500, 0, 400, 600)));
        right
            .children
            .push(node(4, SemanticRole::Text, "Output row", None));
        root.children = vec![left, right];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(10),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(layout.metrics.primary, 0);
        assert_eq!(layout.plan.composition, CompositionKind::MultiPaneWorkspace);
        assert!(matches!(
            layout.plan.root,
            LayoutNode::HorizontalSplit { .. } | LayoutNode::VerticalSplit { .. }
        ));
        let audit = audit_layout_reachability(&layout.plan, &analysis);
        assert!(audit.unplaced.is_empty());
    }

    #[test]
    fn mixed_document_content_prefers_inline_semantics_over_whole_region_placeholder() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 1000, 700)));
        root.children.push(node(
            1,
            SemanticRole::Document,
            "Web Document",
            Some((0, 0, 1000, 700)),
        ));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(11),
            SpatialProbeBudget::default(),
        );
        let presentation = RegionPresentationContext::default().with_mixed_content(
            RuntimeNodeId::new(1),
            "Article",
            6,
            2,
        );
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let primary = layout
            .plan
            .regions
            .iter()
            .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
            .unwrap();
        assert_eq!(
            primary.presentation.kind,
            RegionPresentationKind::InlineContent
        );
    }

    #[test]
    fn text_volume_does_not_override_spatial_dominance() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 1000, 700)));
        root.children.push(node(
            1,
            SemanticRole::Document,
            "small notes",
            Some((0, 0, 150, 120)),
        ));
        root.children.push(node(
            2,
            SemanticRole::Document,
            "main document",
            Some((150, 0, 850, 700)),
        ));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(12),
            SpatialProbeBudget::default(),
        );
        let presentation = RegionPresentationContext::default()
            .with_content(RuntimeNodeId::new(1), "Notes", 500)
            .with_content(RuntimeNodeId::new(2), "Document", 1);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let primary = layout
            .plan
            .regions
            .iter()
            .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
            .unwrap();
        assert!(primary.source_nodes.contains(&RuntimeNodeId::new(2)));
    }

    #[test]
    fn navigation_and_control_detail_choose_navigation_detail_composition() {
        let mut root = node(0, SemanticRole::Application, "app", None);
        let mut navigation = node(1, SemanticRole::List, "Sections", None);
        navigation.children.push(actionable(node(
            2,
            SemanticRole::ListItem,
            "Overview",
            None,
        )));
        root.children = vec![
            navigation,
            actionable(node(3, SemanticRole::Button, "Apply", None)),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(13),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(layout.metrics.primary, 0);
        assert_eq!(layout.plan.composition, CompositionKind::NavigationDetail);
        assert!(matches!(
            layout.plan.root,
            LayoutNode::HorizontalSplit { .. }
        ));
    }

    #[test]
    fn semantic_dialog_produces_dialog_form_without_identity_rule() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 900, 600)));
        let mut dialog = node(
            1,
            SemanticRole::Dialog,
            "Preferences",
            Some((150, 80, 600, 440)),
        );
        dialog.children = vec![
            actionable(node(2, SemanticRole::CheckBox, "Enabled", None)),
            actionable(node(3, SemanticRole::Button, "Apply", None)),
        ];
        root.children.push(dialog);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(14),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(layout.plan.composition, CompositionKind::DialogForm);
        assert!(layout.plan.regions.iter().any(|region| {
            region
                .reasons
                .iter()
                .any(|reason| reason == "role = Dialog")
        }));
    }

    #[test]
    fn a_single_meaningful_control_is_a_control_surface_without_primary() {
        let mut root = node(0, SemanticRole::Application, "app", None);
        root.children
            .push(actionable(node(1, SemanticRole::Button, "Run", None)));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(15),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(layout.metrics.primary, 0);
        assert_eq!(layout.plan.composition, CompositionKind::ControlSurface);
    }

    #[test]
    fn coalescing_preserves_all_source_bindings_and_is_deterministic() {
        let mut root = node(0, SemanticRole::Application, "app", None);
        let mut first = node(1, SemanticRole::List, "First", None);
        first
            .children
            .push(actionable(node(3, SemanticRole::ListItem, "One", None)));
        let mut second = node(2, SemanticRole::List, "Second", None);
        second
            .children
            .push(actionable(node(4, SemanticRole::ListItem, "Two", None)));
        root.children = vec![first, second];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(16),
            SpatialProbeBudget::default(),
        );
        let first_layout = infer_layout(&analysis, &root, &evidence);
        let second_layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(first_layout.plan, second_layout.plan);
        assert_eq!(
            (
                first_layout.metrics.regions,
                first_layout.metrics.primary,
                first_layout.metrics.structural,
                first_layout.metrics.unplaced,
                first_layout.metrics.leaves,
            ),
            (
                second_layout.metrics.regions,
                second_layout.metrics.primary,
                second_layout.metrics.structural,
                second_layout.metrics.unplaced,
                second_layout.metrics.leaves,
            )
        );
        let group = first_layout
            .plan
            .regions
            .iter()
            .find(|region| region.presentation.kind == RegionPresentationKind::ChoiceList)
            .unwrap();
        assert!(
            group
                .presentation
                .source_nodes
                .contains(&RuntimeNodeId::new(1))
        );
        assert!(
            group
                .presentation
                .source_nodes
                .contains(&RuntimeNodeId::new(2))
        );
        assert!(
            audit_layout_reachability(&first_layout.plan, &analysis)
                .unplaced
                .is_empty()
        );
    }

    #[test]
    fn ordinary_intersection_does_not_invent_an_overlay() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 1000, 700)));
        root.children.push(node(
            1,
            SemanticRole::Document,
            "document",
            Some((100, 100, 800, 500)),
        ));
        root.children.push(actionable(node(
            2,
            SemanticRole::Button,
            "partially intersecting control",
            Some((850, 550, 140, 100)),
        )));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(17),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(1), "Document", 2);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        assert!(
            !layout
                .plan
                .regions
                .iter()
                .any(|region| region.kind == SpatialRegionKind::Overlay)
        );
        assert!(!matches!(layout.plan.root, LayoutNode::Overlay { .. }));
    }

    #[test]
    fn untrusted_non_interactive_graphic_does_not_compete_with_trusted_graphical_primary() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 1000, 700)));
        root.children.push(node(
            1,
            SemanticRole::Image,
            "Main visual",
            Some((50, 50, 900, 600)),
        ));
        root.children
            .push(node(2, SemanticRole::Image, "Window icon", None));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(18),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert_eq!(layout.metrics.primary, 1);
        let icon = layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(2)))
            .unwrap();
        assert_eq!(
            icon.presentation.kind,
            RegionPresentationKind::DiagnosticOnly
        );
        assert_eq!(icon.priority, PresentationPriority::HiddenByDefault);
    }

    fn plain_input(id: u64, name: &str, geometry: Option<(i32, i32, i32, i32)>) -> SemanticNode {
        let mut input = node(id, SemanticRole::TextInput, name, geometry);
        input.text_input_kind = Some(TextInputKind::Plain);
        input.states.push(SemanticState::Editable);
        input.capabilities.push(SemanticCapability::EditText);
        input
    }

    fn region_for_source(layout: &LayoutAnalysis, source: u64) -> &SpatialRegion {
        layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(source)))
            .expect("surface")
    }

    #[test]
    fn topology_normalizes_geometry_and_preserves_relative_relationships() {
        let mut root = node(0, SemanticRole::Window, "app", Some((100, 50, 1000, 700)));
        root.children = vec![
            node(
                1,
                SemanticRole::List,
                "Navigation",
                Some((100, 50, 200, 700)),
            ),
            node(
                2,
                SemanticRole::Document,
                "Document",
                Some((300, 50, 800, 700)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(19),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 3);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let navigation = layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(1)))
            .unwrap();
        let document = layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(2)))
            .unwrap();
        assert!(layout.plan.topology.normalized.contains_key(&navigation.id));
        assert!(layout.plan.topology.relates(
            navigation.id,
            document.id,
            TopologyRelationKind::LeftOf
        ));
        assert!(layout.plan.topology.relates(
            navigation.id,
            document.id,
            TopologyRelationKind::AdjacentHorizontal
        ));
    }

    #[test]
    fn top_level_input_is_persistent_but_document_local_input_is_not_promoted() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        let top = plain_input(1, "Destination", Some((0, 0, 1000, 50)));
        let mut document = node(
            2,
            SemanticRole::Document,
            "Document",
            Some((0, 50, 1000, 650)),
        );
        document.children.push(plain_input(
            3,
            "Search this document",
            Some((200, 150, 500, 40)),
        ));
        root.children = vec![top, document];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(20),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let top = layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(1)))
            .unwrap();
        assert_eq!(top.obligation, PresentationObligation::Persistent);
        assert_eq!(top.demand, LayoutDemand::Compact);
        assert_eq!(top.purpose, InteractionPurpose::Input);
        if let Some(local) = layout
            .plan
            .regions
            .iter()
            .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(3)))
        {
            assert_ne!(local.obligation, PresentationObligation::Persistent);
            assert_eq!(local.purpose, InteractionPurpose::Search);
        }
    }

    #[test]
    fn persistent_input_purpose_uses_generic_evidence_and_preserves_uncertainty() {
        let infer = |id, label: &str| {
            let mut root = node(
                0,
                SemanticRole::Window,
                "application",
                Some((0, 0, 800, 600)),
            );
            root.children
                .push(plain_input(id, label, Some((0, 0, 800, 45))));
            let analysis = analyze_regions(&root);
            let evidence = SpatialEvidenceIndex::from_tree(
                &root,
                ApplicationGenerationId(id),
                SpatialProbeBudget::default(),
            );
            infer_layout(&analysis, &root, &evidence)
                .plan
                .regions
                .into_iter()
                .find(|region| {
                    region
                        .presentation
                        .source_nodes
                        .contains(&RuntimeNodeId::new(id))
                        && region.presentation.kind == RegionPresentationKind::InputSurface
                })
                .expect("top-level input remains represented")
                .purpose
        };

        assert_eq!(infer(33, "Search records"), InteractionPurpose::Search);
        assert_eq!(infer(34, "Destination"), InteractionPurpose::Input);
    }

    #[test]
    fn bottom_status_band_remains_compact_and_covered() {
        let mut root = node(
            0,
            SemanticRole::Window,
            "application",
            Some((0, 0, 1000, 700)),
        );
        root.children = vec![
            node(
                1,
                SemanticRole::Document,
                "Document",
                Some((0, 0, 1000, 660)),
            ),
            node(
                2,
                SemanticRole::StatusBar,
                "Ready",
                Some((0, 660, 1000, 40)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(35),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(1), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let status = layout
            .plan
            .regions
            .iter()
            .find(|region| region.presentation.kind == RegionPresentationKind::Status)
            .expect("status surface");
        assert_eq!(status.demand, LayoutDemand::Compact);
        let responsive = realize_responsive_layout(&layout.plan, 80, 24, None);
        let coverage = audit_presentation_coverage(&layout.plan, &responsive);
        assert_eq!(coverage.meaningful_statuses, 1);
        assert_eq!(coverage.meaningful_statuses_represented, 1);
    }

    #[test]
    fn multiple_tabs_preserve_current_context_when_narrow() {
        let mut root = node(
            0,
            SemanticRole::Window,
            "application",
            Some((0, 0, 1000, 700)),
        );
        let mut tabs = node(1, SemanticRole::TabList, "Contexts", Some((0, 0, 1000, 45)));
        for (id, label) in [(2, "First"), (3, "Current"), (4, "Third")] {
            let mut tab = actionable(node(id, SemanticRole::Tab, label, None));
            if id == 3 {
                tab.states.push(SemanticState::Selected);
            }
            tabs.children.push(tab);
        }
        root.children = vec![
            tabs,
            node(
                5,
                SemanticRole::Document,
                "Document",
                Some((0, 45, 1000, 655)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(36),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(5), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let tab_surface = layout
            .plan
            .regions
            .iter()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(1))
            })
            .expect("tab context surface");
        assert_eq!(tab_surface.obligation, PresentationObligation::Persistent);
        assert_eq!(tab_surface.visibility, VisibilityGuarantee::Pinned);
        assert_eq!(tab_surface.presentation.title, "Current · Current");
        let responsive = realize_responsive_layout(&layout.plan, 60, 24, None);
        assert!(plan_leaves(&responsive.root).contains(&tab_surface.id));
        assert!(
            audit_presentation_coverage(&layout.plan, &responsive)
                .missing
                .is_empty()
        );
    }

    #[test]
    fn workspace_aggregation_preserves_persistent_input_and_tab_anchors() {
        let mut root = node(
            0,
            SemanticRole::Application,
            "application",
            Some((0, 0, 1000, 700)),
        );
        let mut window = node(
            1,
            SemanticRole::Window,
            "workspace",
            Some((0, 0, 1000, 700)),
        );
        let mut controls = node(
            2,
            SemanticRole::MenuBar,
            "application controls",
            Some((0, 0, 1000, 50)),
        );
        controls.children = vec![
            actionable(node(3, SemanticRole::Button, "Back", None)),
            plain_input(4, "Destination", Some((120, 5, 700, 40))),
        ];
        let mut tabs = node(
            5,
            SemanticRole::TabList,
            "Contexts",
            Some((0, 50, 1000, 40)),
        );
        let mut current = actionable(node(6, SemanticRole::Tab, "Current", None));
        current.states.push(SemanticState::Selected);
        tabs.children.push(current);
        window.children = vec![
            controls,
            tabs,
            node(
                7,
                SemanticRole::Document,
                "Document",
                Some((0, 90, 1000, 610)),
            ),
        ];
        root.children.push(window);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(27),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(7), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let input = layout
            .plan
            .regions
            .iter()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(4))
            })
            .expect("application input remains a presentation surface");
        assert_eq!(
            input.presentation.kind,
            RegionPresentationKind::InputSurface
        );
        assert_eq!(input.obligation, PresentationObligation::Persistent);
        let tabs = layout
            .plan
            .regions
            .iter()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(5))
            })
            .expect("tab context remains a presentation surface");
        assert_eq!(tabs.obligation, PresentationObligation::Persistent);
        for size in [(140, 40), (80, 30), (60, 24)] {
            let responsive = realize_responsive_layout(&layout.plan, size.0, size.1, None);
            assert!(
                audit_presentation_coverage(&layout.plan, &responsive)
                    .missing
                    .is_empty()
            );
        }
    }

    #[test]
    fn tab_context_and_toolbar_surfaces_are_persistent_and_compact() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        let mut tabs = node(1, SemanticRole::TabList, "Contexts", Some((0, 0, 1000, 40)));
        let mut current = actionable(node(2, SemanticRole::Tab, "Current", None));
        current.states.push(SemanticState::Selected);
        tabs.children.push(current);
        let mut toolbar = node(
            3,
            SemanticRole::MenuBar,
            "Commands",
            Some((0, 40, 1000, 40)),
        );
        toolbar
            .children
            .push(actionable(node(4, SemanticRole::MenuItem, "Open", None)));
        root.children = vec![tabs, toolbar];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(21),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        assert!(layout.plan.regions.iter().any(|region| {
            region.source_nodes.contains(&RuntimeNodeId::new(1))
                && region.obligation == PresentationObligation::Persistent
                && region.demand == LayoutDemand::Compact
        }));
        assert!(layout.plan.regions.iter().any(|region| {
            region.source_nodes.contains(&RuntimeNodeId::new(3))
                && matches!(
                    region.obligation,
                    PresentationObligation::Persistent | PresentationObligation::Discoverable
                )
                && region.demand == LayoutDemand::Compact
        }));
    }

    #[test]
    fn unnamed_noninteractive_tab_wrapper_does_not_duplicate_primary_context() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        let mut tabs = node(1, SemanticRole::TabList, "", Some((0, 0, 1000, 700)));
        let mut current = node(2, SemanticRole::Tab, "", Some((0, 0, 1000, 700)));
        current.states.push(SemanticState::Selected);
        current.children.push(node(
            3,
            SemanticRole::Document,
            "Document",
            Some((0, 0, 1000, 700)),
        ));
        tabs.children.push(current);
        root.children.push(tabs);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(28),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(3), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let tab_wrapper = layout
            .plan
            .regions
            .iter()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(1))
            })
            .expect("tab wrapper remains available to diagnostics");
        assert_eq!(
            tab_wrapper.presentation.kind,
            RegionPresentationKind::DiagnosticOnly
        );
        assert_eq!(tab_wrapper.demand, LayoutDemand::Hidden);
    }

    #[test]
    fn persistent_minimal_surface_collapses_beside_a_viable_primary() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        root.children = vec![
            node(
                1,
                SemanticRole::Document,
                "Document",
                Some((0, 0, 800, 700)),
            ),
            node(
                2,
                SemanticRole::List,
                "Temporarily empty context",
                Some((800, 0, 200, 700)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(29),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(1), "Document", 4);
        let mut layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let secondary = layout
            .plan
            .regions
            .iter_mut()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(2))
            })
            .expect("secondary surface");
        secondary.obligation = PresentationObligation::Persistent;
        secondary.demand = LayoutDemand::Minimal;
        let secondary_id = secondary.id;
        let responsive = realize_responsive_layout(&layout.plan, 140, 40, None);
        assert!(responsive.collapsed.contains(&secondary_id));
        assert!(!plan_leaves(&responsive.root).contains(&secondary_id));
        assert!(
            audit_presentation_coverage(&layout.plan, &responsive)
                .missing
                .is_empty()
        );
    }

    #[test]
    fn responsive_composition_preserves_required_surfaces_at_narrow_width() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        root.children = vec![
            plain_input(1, "Filter", Some((0, 0, 1000, 45))),
            node(
                2,
                SemanticRole::Document,
                "Document",
                Some((0, 45, 1000, 655)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(22),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 8);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let input_id = layout
            .plan
            .regions
            .iter()
            .find(|region| {
                region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(1))
            })
            .expect("persistent input surface")
            .id;
        let wide = realize_responsive_layout(&layout.plan, 140, 40, None);
        let narrow = realize_responsive_layout(&layout.plan, 60, 24, None);
        assert_eq!(wide.class, TerminalWidthClass::Wide);
        assert!(matches!(
            narrow.class,
            TerminalWidthClass::Narrow | TerminalWidthClass::Compact
        ));
        assert!(plan_leaves(&narrow.root).contains(&input_id));
        assert!(
            audit_presentation_coverage(&layout.plan, &wide)
                .missing
                .is_empty()
        );
        assert!(
            audit_presentation_coverage(&layout.plan, &narrow)
                .missing
                .is_empty()
        );
        let audit = audit_presentation_coverage(&layout.plan, &narrow);
        assert!(audit.primary_represented);
        assert_eq!(audit.pinned, 1);
        assert_eq!(audit.pinned_directly_represented, 1);
        assert!(audit.pinned_improperly_collapsed.is_empty());
    }

    #[test]
    fn graphical_placeholder_requests_supporting_not_expand_demand() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 800, 600)));
        root.children.push(node(
            1,
            SemanticRole::Image,
            "Visual",
            Some((50, 50, 700, 500)),
        ));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(23),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        let primary = layout
            .plan
            .regions
            .iter()
            .find(|region| region.kind == SpatialRegionKind::PrimaryContent)
            .unwrap();
        assert_eq!(primary.demand, LayoutDemand::Supporting);
        assert_eq!(primary.obligation, PresentationObligation::Persistent);
    }

    #[test]
    fn unlabelled_unbounded_images_defer_to_distinguishable_graphical_surface() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 800, 600)));
        let drawing = node(
            1,
            SemanticRole::Unknown("drawing area".into()),
            "",
            Some((40, 40, 720, 500)),
        );
        let mut thumbnails = node(2, SemanticRole::Container, "", None);
        thumbnails
            .children
            .push(node(3, SemanticRole::Image, "", None));
        let mut current = node(4, SemanticRole::Image, "", None);
        current.states.push(SemanticState::Selected);
        current.actions.push(SemanticAction {
            index: 0,
            name: String::new(),
            description: None,
            keybinding: None,
        });
        thumbnails.children.push(current);
        root.children = vec![drawing, thumbnails];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(31),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        for source in [RuntimeNodeId::new(3), RuntimeNodeId::new(4)] {
            let image = layout
                .plan
                .regions
                .iter()
                .find(|region| region.presentation.source_nodes.contains(&source))
                .expect("image remains available to diagnostics");
            assert_eq!(
                image.presentation.kind,
                RegionPresentationKind::DiagnosticOnly
            );
            assert_eq!(image.demand, LayoutDemand::Hidden);
        }
        assert!(layout.plan.regions.iter().any(|region| {
            region.presentation.kind == RegionPresentationKind::GraphicalPlaceholder
                && region
                    .presentation
                    .source_nodes
                    .contains(&RuntimeNodeId::new(1))
        }));
    }

    #[test]
    fn scope_filtered_scene_confines_region_navigation() {
        let mut root = node(0, SemanticRole::Application, "app", Some((0, 0, 800, 600)));
        root.children.push(node(
            1,
            SemanticRole::Document,
            "Background",
            Some((0, 0, 800, 600)),
        ));
        let mut dialog = node(
            2,
            SemanticRole::Dialog,
            "Dialog",
            Some((150, 100, 500, 400)),
        );
        dialog
            .children
            .push(actionable(node(3, SemanticRole::Button, "Apply", None)));
        root.children.push(dialog);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(24),
            SpatialProbeBudget::default(),
        );
        let presentation = RegionPresentationContext::default().with_content(
            RuntimeNodeId::new(1),
            "Background",
            3,
        );
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let scene = TuiScene::new(
            "Dialog".into(),
            &root,
            vec![SceneElement {
                id: SceneElementId::new(1),
                kind: SceneElementKind::Button {
                    label: "Apply".into(),
                },
                sources: vec![RuntimeNodeId::new(2), RuntimeNodeId::new(3)],
                binding: None,
                strategy: PresentationStrategy::DirectWidget,
            }],
        );
        let order = region_focus_order(&layout.plan, &scene);
        assert!(!order.is_empty());
        assert!(order.iter().all(|id| {
            layout
                .plan
                .regions
                .iter()
                .find(|region| region.id == *id)
                .is_some_and(|region| {
                    !region.source_nodes.contains(&RuntimeNodeId::new(1))
                        && region
                            .source_nodes
                            .iter()
                            .any(|source| matches!(source.get(), 2 | 3))
                })
        }));
    }

    #[test]
    fn realized_payload_refines_empty_and_rich_pane_demands() {
        let mut root = node(0, SemanticRole::Window, "app", None);
        let mut group = node(1, SemanticRole::Container, "Inspector", None);
        for id in 2..=5 {
            group.children.push(actionable(node(
                id,
                SemanticRole::Button,
                &format!("Control {id}"),
                None,
            )));
        }
        root.children.push(group);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(25),
            SpatialProbeBudget::default(),
        );
        let mut layout = infer_layout(&analysis, &root, &evidence);
        let empty_scene = TuiScene::new("empty".into(), &root, Vec::new());
        refine_layout_demands_from_scene(&mut layout, &empty_scene);
        assert!(layout.plan.regions.iter().any(|region| {
            region.presentation.kind.is_meaningful() && region.demand == LayoutDemand::Minimal
        }));

        let elements = (2..=5)
            .map(|id| SceneElement {
                id: SceneElementId::new(id),
                kind: SceneElementKind::Button {
                    label: format!("Control {id}"),
                },
                sources: vec![RuntimeNodeId::new(id)],
                binding: None,
                strategy: PresentationStrategy::DirectWidget,
            })
            .collect();
        let rich_scene = TuiScene::new("rich".into(), &root, elements);
        let mut rich = infer_layout(&analysis, &root, &evidence);
        refine_layout_demands_from_scene(&mut rich, &rich_scene);
        assert!(rich.plan.regions.iter().any(|region| {
            region.presentation.kind.is_meaningful() && region.demand == LayoutDemand::Supporting
        }));
    }

    #[test]
    fn command_only_aggregate_defers_to_dedicated_command_surface() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 1000, 700)));
        let mut container = node(
            1,
            SemanticRole::Container,
            "application shell",
            Some((0, 0, 1000, 80)),
        );
        let mut commands = node(2, SemanticRole::MenuBar, "Commands", Some((0, 0, 1000, 80)));
        commands
            .children
            .push(actionable(node(3, SemanticRole::MenuItem, "Open", None)));
        container.children.push(commands);
        root.children.push(container);
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(30),
            SpatialProbeBudget::default(),
        );
        let mut layout = infer_layout(&analysis, &root, &evidence);
        let scene = TuiScene::new(
            "commands".into(),
            &root,
            vec![SceneElement {
                id: SceneElementId::new(1),
                kind: SceneElementKind::CommandHeader {
                    label: "Commands (1 available)".into(),
                },
                sources: vec![RuntimeNodeId::new(3)],
                binding: None,
                strategy: PresentationStrategy::CommandList,
            }],
        );
        refine_layout_demands_from_scene(&mut layout, &scene);
        assert!(
            layout.plan.regions.iter().any(|region| {
                region.demand == LayoutDemand::Hidden
                    && region
                        .reasons
                        .iter()
                        .any(|reason| reason.contains("command-only aggregate is discoverable"))
            }),
            "{:#?}",
            layout.plan.regions
        );
        assert!(layout.plan.regions.iter().any(|region| {
            region.presentation.kind == RegionPresentationKind::CommandBar
                && region.demand == LayoutDemand::Compact
        }));
    }

    #[test]
    fn coverage_audit_reports_an_intentionally_dropped_persistent_surface() {
        let mut root = node(0, SemanticRole::Window, "app", Some((0, 0, 800, 600)));
        root.children
            .push(plain_input(1, "Input", Some((0, 0, 800, 40))));
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(26),
            SpatialProbeBudget::default(),
        );
        let layout = infer_layout(&analysis, &root, &evidence);
        let incomplete = ResponsiveComposition {
            class: TerminalWidthClass::Compact,
            root: LayoutNode::Stack(Vec::new()),
            represented: Vec::new(),
            collapsed: Vec::new(),
            forced_collapsed: Vec::new(),
            reasons: Vec::new(),
        };
        let audit = audit_presentation_coverage(&layout.plan, &incomplete);
        assert_eq!(audit.persistent, 1);
        assert_eq!(audit.missing.len(), 1);
    }

    #[test]
    fn geometry_changes_topology_without_changing_surface_semantics() {
        let analyze = |navigation_geometry, document_geometry, generation| {
            let mut root = node(
                0,
                SemanticRole::Window,
                "application",
                Some((0, 0, 1000, 700)),
            );
            root.children = vec![
                node(
                    1,
                    SemanticRole::List,
                    "Navigation",
                    Some(navigation_geometry),
                ),
                node(
                    2,
                    SemanticRole::Document,
                    "Document",
                    Some(document_geometry),
                ),
            ];
            let regions = analyze_regions(&root);
            let evidence = SpatialEvidenceIndex::from_tree(
                &root,
                ApplicationGenerationId(generation),
                SpatialProbeBudget::default(),
            );
            let presentation = RegionPresentationContext::default().with_content(
                RuntimeNodeId::new(2),
                "Document",
                4,
            );
            infer_layout_with_presentations(&regions, &root, &evidence, Some(&presentation))
        };
        let horizontal = analyze((0, 0, 200, 700), (200, 0, 800, 700), 37);
        let vertical = analyze((0, 0, 1000, 100), (0, 100, 1000, 600), 38);
        let horizontal_navigation = region_for_source(&horizontal, 1);
        let horizontal_document = region_for_source(&horizontal, 2);
        let vertical_navigation = region_for_source(&vertical, 1);
        let vertical_document = region_for_source(&vertical, 2);
        assert_eq!(
            horizontal_navigation.presentation.kind,
            vertical_navigation.presentation.kind
        );
        assert_eq!(
            horizontal_document.presentation.kind,
            vertical_document.presentation.kind
        );
        assert!(horizontal.plan.topology.relates(
            horizontal_navigation.id,
            horizontal_document.id,
            TopologyRelationKind::LeftOf
        ));
        assert!(vertical.plan.topology.relates(
            vertical_navigation.id,
            vertical_document.id,
            TopologyRelationKind::Above
        ));
    }

    #[test]
    fn semantics_change_surface_interpretation_without_changing_geometry() {
        let infer = |role, id, label: &str| {
            let mut root = node(
                0,
                SemanticRole::Window,
                "application",
                Some((0, 0, 800, 600)),
            );
            let child = if role == SemanticRole::TextInput {
                plain_input(id, label, Some((0, 560, 800, 40)))
            } else {
                node(id, role, label, Some((0, 560, 800, 40)))
            };
            root.children.push(child);
            let analysis = analyze_regions(&root);
            let evidence = SpatialEvidenceIndex::from_tree(
                &root,
                ApplicationGenerationId(id),
                SpatialProbeBudget::default(),
            );
            infer_layout(&analysis, &root, &evidence)
                .plan
                .regions
                .into_iter()
                .find(|region| region.source_nodes.contains(&RuntimeNodeId::new(id)))
                .expect("surface")
                .presentation
                .kind
        };
        assert_eq!(
            infer(SemanticRole::TextInput, 39, "Query"),
            RegionPresentationKind::InputSurface
        );
        assert_eq!(
            infer(SemanticRole::StatusBar, 40, "Ready"),
            RegionPresentationKind::Status
        );
    }

    #[test]
    fn terminal_resize_preserves_plan_identity_and_surface_bindings() {
        let mut root = node(
            0,
            SemanticRole::Window,
            "application",
            Some((0, 0, 1000, 700)),
        );
        root.children = vec![
            plain_input(1, "Filter", Some((0, 0, 1000, 45))),
            node(
                2,
                SemanticRole::Document,
                "Document",
                Some((0, 45, 1000, 655)),
            ),
        ];
        let analysis = analyze_regions(&root);
        let evidence = SpatialEvidenceIndex::from_tree(
            &root,
            ApplicationGenerationId(41),
            SpatialProbeBudget::default(),
        );
        let presentation =
            RegionPresentationContext::default().with_content(RuntimeNodeId::new(2), "Document", 4);
        let layout =
            infer_layout_with_presentations(&analysis, &root, &evidence, Some(&presentation));
        let original = layout.plan.clone();
        let wide = realize_responsive_layout(&layout.plan, 140, 40, None);
        let narrow = realize_responsive_layout(&layout.plan, 60, 24, None);
        assert_eq!(layout.plan, original);
        assert!(
            audit_presentation_coverage(&layout.plan, &wide)
                .missing
                .is_empty()
        );
        assert!(
            audit_presentation_coverage(&layout.plan, &narrow)
                .missing
                .is_empty()
        );
        assert_eq!(
            layout
                .plan
                .regions
                .iter()
                .map(|region| (&region.id, &region.presentation.source_nodes))
                .collect::<Vec<_>>(),
            original
                .regions
                .iter()
                .map(|region| (&region.id, &region.presentation.source_nodes))
                .collect::<Vec<_>>()
        );
    }
}
