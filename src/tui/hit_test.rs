use ratatui::layout::Rect;

use crate::semantic::RuntimeNodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitInteraction {
    Focus,
    Activate,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitRegion {
    pub runtime_id: RuntimeNodeId,
    pub rect: Rect,
    pub interaction: HitInteraction,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HitMap {
    regions: Vec<HitRegion>,
}

impl HitMap {
    pub fn replace(&mut self, regions: Vec<HitRegion>) {
        self.regions = regions;
    }

    pub fn hit(&self, x: u16, y: u16) -> Option<HitRegion> {
        self.regions
            .iter()
            .rev()
            .find(|region| contains(region.rect, x, y))
            .copied()
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing_uses_terminal_rectangles_and_runtime_ids() {
        let expected = RuntimeNodeId::new(7);
        let mut map = HitMap::default();
        map.replace(vec![HitRegion {
            runtime_id: expected,
            rect: Rect::new(4, 3, 12, 2),
            interaction: HitInteraction::Activate,
        }]);

        assert_eq!(map.hit(4, 3).unwrap().runtime_id, expected);
        assert_eq!(map.hit(15, 4).unwrap().runtime_id, expected);
        assert!(map.hit(16, 4).is_none());
        assert!(map.hit(5, 5).is_none());
    }
}
