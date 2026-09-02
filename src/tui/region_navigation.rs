use std::collections::HashSet;

use crate::transcompile::{
    LayoutDemand, LayoutNode, SpatialRegion, SpatialRegionId, TuiLayoutPlan, TuiScene,
    region_focus_order,
};

/// A presentation-only, two-level projection of the existing layout tree.
/// It neither changes region identity nor introduces semantic grouping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionNavigator {
    groups: Vec<RegionNavigationGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionNavigationGroup {
    pub title: String,
    pub regions: Vec<RegionNavigationItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionNavigationItem {
    pub id: SpatialRegionId,
    pub title: String,
    pub empty: bool,
}

impl RegionNavigator {
    pub fn derive(plan: &TuiLayoutPlan, scene: &TuiScene) -> Self {
        let focus_order = region_focus_order(plan, scene);
        let eligible = focus_order.iter().copied().collect::<HashSet<_>>();
        if eligible.is_empty() {
            return Self { groups: Vec::new() };
        }

        let roots = meaningful_children(&plan.root, &eligible);
        let mut groups = roots
            .iter()
            .filter_map(|node| group_for_node(plan, node, &eligible))
            .collect::<Vec<_>>();

        // A layout with no trustworthy branch hierarchy remains a flat,
        // single-level navigator in the existing region focus order.
        let grouped = groups
            .iter()
            .map(|group| group.regions.len())
            .sum::<usize>();
        if groups.is_empty() || grouped != eligible.len() {
            groups = focus_order
                .iter()
                .filter_map(|id| plan.regions.iter().find(|region| region.id == *id))
                .map(|region| RegionNavigationGroup {
                    title: region.presentation.title.clone(),
                    regions: vec![item(region)],
                })
                .collect();
        }

        Self { groups }
    }

    pub fn groups(&self) -> &[RegionNavigationGroup] {
        &self.groups
    }

    pub fn region_count(&self) -> usize {
        self.groups.iter().map(|group| group.regions.len()).sum()
    }

    pub fn active_group_index(&self, active: Option<SpatialRegionId>) -> Option<usize> {
        let active = active?;
        self.groups
            .iter()
            .position(|group| group.regions.iter().any(|region| region.id == active))
    }

    pub fn has_subregions(&self) -> bool {
        self.groups.iter().any(|group| group.regions.len() > 1)
    }

    pub fn cycle_major(
        &self,
        active: Option<SpatialRegionId>,
        reverse: bool,
    ) -> Option<SpatialRegionId> {
        if self.groups.is_empty() {
            return None;
        }
        let current = self.active_group_index(active);
        let next = cycle_index(self.groups.len(), current, reverse);
        self.groups[next].regions.first().map(|region| region.id)
    }

    pub fn cycle_subregion(
        &self,
        active: Option<SpatialRegionId>,
        reverse: bool,
    ) -> Option<SpatialRegionId> {
        let group = self.groups.get(self.active_group_index(active)?)?;
        if group.regions.len() < 2 {
            return active;
        }
        let current =
            active.and_then(|active| group.regions.iter().position(|region| region.id == active));
        Some(group.regions[cycle_index(group.regions.len(), current, reverse)].id)
    }
}

fn meaningful_children<'a>(
    root: &'a LayoutNode,
    eligible: &HashSet<SpatialRegionId>,
) -> Vec<&'a LayoutNode> {
    let mut current = root;
    loop {
        let children = node_children(current)
            .into_iter()
            .filter(|child| contains_eligible(child, eligible))
            .collect::<Vec<_>>();
        if children.len() == 1 && !matches!(children[0], LayoutNode::Leaf(_)) {
            current = children[0];
            continue;
        }
        return if children.is_empty() {
            vec![current]
        } else {
            children
        };
    }
}

fn node_children(node: &LayoutNode) -> Vec<&LayoutNode> {
    match node {
        LayoutNode::Leaf(_) => Vec::new(),
        LayoutNode::Stack(children)
        | LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => children.iter().collect(),
        LayoutNode::Overlay { base, overlays } => std::iter::once(base.as_ref())
            .chain(overlays.iter())
            .collect(),
    }
}

fn contains_eligible(node: &LayoutNode, eligible: &HashSet<SpatialRegionId>) -> bool {
    match node {
        LayoutNode::Leaf(id) => eligible.contains(id),
        _ => node_children(node)
            .into_iter()
            .any(|child| contains_eligible(child, eligible)),
    }
}

fn group_for_node(
    plan: &TuiLayoutPlan,
    node: &LayoutNode,
    eligible: &HashSet<SpatialRegionId>,
) -> Option<RegionNavigationGroup> {
    let mut ids = Vec::new();
    collect_leaves(node, eligible, &mut ids);
    let regions = ids
        .iter()
        .filter_map(|id| plan.regions.iter().find(|region| region.id == *id))
        .map(item)
        .collect::<Vec<_>>();
    let title = ids
        .iter()
        .filter_map(|id| plan.regions.iter().find(|region| region.id == *id))
        .max_by_key(|region| group_title_rank(region))
        .map(|region| region.presentation.title.clone())?;
    Some(RegionNavigationGroup { title, regions })
}

fn collect_leaves(
    node: &LayoutNode,
    eligible: &HashSet<SpatialRegionId>,
    output: &mut Vec<SpatialRegionId>,
) {
    match node {
        LayoutNode::Leaf(id) if eligible.contains(id) => output.push(*id),
        LayoutNode::Leaf(_) => {}
        _ => {
            for child in node_children(node) {
                collect_leaves(child, eligible, output);
            }
        }
    }
}

fn group_title_rank(region: &SpatialRegion) -> (bool, u8, usize, std::cmp::Reverse<u64>) {
    let demand = match region.demand {
        LayoutDemand::Expand => 4,
        LayoutDemand::Supporting => 3,
        LayoutDemand::Compact => 2,
        LayoutDemand::Minimal => 1,
        LayoutDemand::Hidden => 0,
    };
    (
        region.demand != LayoutDemand::Minimal,
        demand,
        region.presentation.meaningful_items,
        std::cmp::Reverse(region.id.0),
    )
}

fn item(region: &SpatialRegion) -> RegionNavigationItem {
    RegionNavigationItem {
        id: region.id,
        title: region.presentation.title.clone(),
        empty: region.demand == LayoutDemand::Minimal,
    }
}

fn cycle_index(length: usize, current: Option<usize>, reverse: bool) -> usize {
    if reverse {
        current.map_or(
            length - 1,
            |index| {
                if index == 0 { length - 1 } else { index - 1 }
            },
        )
    } else {
        current.map_or(0, |index| (index + 1) % length)
    }
}
