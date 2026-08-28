use crate::semantic::{BackendLocator, RuntimeNodeId};

use super::view_model::TuiViewModel;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusModel {
    current: Option<RuntimeNodeId>,
}

impl FocusModel {
    pub fn current(&self) -> Option<RuntimeNodeId> {
        self.current
    }

    pub fn set(&mut self, view: &TuiViewModel, id: RuntimeNodeId) -> bool {
        if view
            .element(id)
            .is_some_and(|element| element.is_focusable())
        {
            self.current = Some(id);
            true
        } else {
            false
        }
    }

    pub fn reconcile(&mut self, view: &TuiViewModel, preferred_locator: Option<&BackendLocator>) {
        let focusable = view.focusable_ids();
        self.current = preferred_locator
            .and_then(|locator| view.runtime_id_for_locator(locator))
            .or_else(|| focusable.first().copied());
    }

    pub fn next(&mut self, view: &TuiViewModel) {
        let ids = view.focusable_ids();
        self.current = cycle(&ids, self.current, 1);
    }

    pub fn previous(&mut self, view: &TuiViewModel) {
        let ids = view.focusable_ids();
        self.current = cycle(&ids, self.current, -1);
    }
}

fn cycle(
    ids: &[RuntimeNodeId],
    current: Option<RuntimeNodeId>,
    direction: i8,
) -> Option<RuntimeNodeId> {
    if ids.is_empty() {
        return None;
    }
    let current_index = current.and_then(|current| ids.iter().position(|id| *id == current));
    let next_index = match (current_index, direction) {
        (Some(0), -1) | (None, -1) => ids.len() - 1,
        (Some(index), -1) => index - 1,
        (Some(index), _) => (index + 1) % ids.len(),
        (None, _) => 0,
    };
    Some(ids[next_index])
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub offset: u16,
}

impl Viewport {
    pub fn ensure_visible(&mut self, item_top: u16, item_height: u16, viewport_height: u16) {
        if viewport_height == 0 {
            return;
        }
        if item_top < self.offset {
            self.offset = item_top;
        } else {
            let item_bottom = item_top.saturating_add(item_height);
            let visible_bottom = self.offset.saturating_add(viewport_height);
            if item_bottom > visible_bottom {
                self.offset = item_bottom.saturating_sub(viewport_height);
            }
        }
    }

    pub fn scroll_lines(&mut self, delta: i16, content_height: u16, viewport_height: u16) {
        let max_offset = content_height.saturating_sub(viewport_height);
        self.offset = self.offset.saturating_add_signed(delta).min(max_offset);
    }

    pub fn scroll_pages(&mut self, pages: i16, content_height: u16, viewport_height: u16) {
        let delta = pages.saturating_mul(viewport_height.min(i16::MAX as u16) as i16);
        self.scroll_lines(delta, content_height, viewport_height);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        semantic::{BackendLocator, DebugInfo, SemanticNode, SemanticRole},
        tui::view_model::TuiViewModel,
    };

    use super::*;

    fn node(id: u64, role: SemanticRole) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(format!("node {id}")),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn focus_skips_labels_and_wraps_both_directions() {
        let mut root = node(0, SemanticRole::Window);
        root.children = vec![
            node(1, SemanticRole::Label),
            node(2, SemanticRole::Button),
            node(3, SemanticRole::Label),
            node(4, SemanticRole::CheckBox),
        ];
        let view = TuiViewModel::from_snapshot(&root);
        let mut focus = FocusModel::default();
        focus.reconcile(&view, None);
        assert_eq!(focus.current(), Some(RuntimeNodeId::new(2)));
        focus.next(&view);
        assert_eq!(focus.current(), Some(RuntimeNodeId::new(4)));
        focus.next(&view);
        assert_eq!(focus.current(), Some(RuntimeNodeId::new(2)));
        focus.previous(&view);
        assert_eq!(focus.current(), Some(RuntimeNodeId::new(4)));
    }

    #[test]
    fn focus_outside_viewport_scrolls_into_view() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(12, 2, 5);
        assert_eq!(viewport.offset, 9);
        viewport.ensure_visible(3, 1, 5);
        assert_eq!(viewport.offset, 3);
    }
}
