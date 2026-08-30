use crate::{
    semantic::{BackendLocator, RuntimeNodeId},
    transcompile::{SceneElement, SceneElementId, TuiScene},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusModel {
    current: Option<SceneElementId>,
}

impl FocusModel {
    pub fn current(&self) -> Option<SceneElementId> {
        self.current
    }

    pub fn set(&mut self, scene: &TuiScene, id: SceneElementId) -> bool {
        if scene
            .element(id)
            .is_some_and(|element| element.is_focusable())
        {
            self.current = Some(id);
            true
        } else {
            false
        }
    }

    pub fn reconcile(&mut self, scene: &TuiScene, preferred_locator: Option<&BackendLocator>) {
        self.reconcile_identity(scene, None, preferred_locator);
    }

    pub fn reconcile_identity(
        &mut self,
        scene: &TuiScene,
        preferred_runtime: Option<RuntimeNodeId>,
        preferred_locator: Option<&BackendLocator>,
    ) {
        let focusable = scene.focusable_ids();
        self.current = preferred_runtime
            .and_then(|id| scene.scene_id_for_runtime(id))
            .filter(|id| scene.element(*id).is_some_and(SceneElement::is_focusable))
            .or_else(|| preferred_locator.and_then(|locator| scene.scene_id_for_locator(locator)))
            .or_else(|| focusable.first().copied());
    }

    pub fn next(&mut self, scene: &TuiScene) {
        let ids = scene.focusable_ids();
        self.current = cycle(&ids, self.current, 1);
    }

    pub fn previous(&mut self, scene: &TuiScene) {
        let ids = scene.focusable_ids();
        self.current = cycle(&ids, self.current, -1);
    }
}

fn cycle(
    ids: &[SceneElementId],
    current: Option<SceneElementId>,
    direction: i8,
) -> Option<SceneElementId> {
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
        semantic::{BackendLocator, DebugInfo, RuntimeNodeId, SemanticNode, SemanticRole},
        transcompile::compile_legacy_scene,
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
        let scene = compile_legacy_scene(&root);
        let button = scene.scene_id_for_runtime(RuntimeNodeId::new(2)).unwrap();
        let checkbox = scene.scene_id_for_runtime(RuntimeNodeId::new(4)).unwrap();
        let mut focus = FocusModel::default();
        focus.reconcile(&scene, None);
        assert_eq!(focus.current(), Some(button));
        focus.next(&scene);
        assert_eq!(focus.current(), Some(checkbox));
        focus.next(&scene);
        assert_eq!(focus.current(), Some(button));
        focus.previous(&scene);
        assert_eq!(focus.current(), Some(checkbox));
    }

    #[test]
    fn focus_outside_viewport_scrolls_into_view() {
        let mut viewport = Viewport::default();
        viewport.ensure_visible(12, 2, 5);
        assert_eq!(viewport.offset, 9);
        viewport.ensure_visible(3, 1, 5);
        assert_eq!(viewport.offset, 3);
    }

    #[test]
    fn runtime_identity_restores_focus_when_scene_id_or_locator_changes() {
        use crate::transcompile::{
            PresentationStrategy, SceneBinding, SceneElement, SceneElementKind,
        };
        use crate::tui::action::{InteractionCapability, UiIntent};

        let root = node(0, SemanticRole::Window);
        let runtime_id = RuntimeNodeId::new(77);
        let scene = TuiScene::new(
            "regenerated".to_owned(),
            &root,
            vec![SceneElement {
                id: SceneElementId::new(900),
                kind: SceneElementKind::Button {
                    label: "Restore".to_owned(),
                },
                sources: vec![runtime_id],
                binding: Some(SceneBinding {
                    runtime_id,
                    backend_locator: BackendLocator::new(":1.new", "/replacement"),
                    semantic_role: SemanticRole::Button,
                    actions: Vec::new(),
                    capability: InteractionCapability::Activate,
                    default_intent: UiIntent::Activate,
                }),
                strategy: PresentationStrategy::DirectWidget,
            }],
        );
        let mut focus = FocusModel::default();
        focus.reconcile_identity(
            &scene,
            Some(runtime_id),
            Some(&BackendLocator::new(":1.old", "/stale")),
        );
        assert_eq!(focus.current(), Some(SceneElementId::new(900)));
    }
}
