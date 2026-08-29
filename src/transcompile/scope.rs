use std::{collections::HashMap, fmt};

use crate::semantic::{
    BackendLocator, RelationalSemanticGraph, RuntimeNodeId, SemanticCache, SemanticRole,
    SemanticState,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InteractionScopeId(pub RuntimeNodeId);

impl fmt::Display for InteractionScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionScopeKind {
    Application,
    Window,
    Dialog,
    ModalDialog,
    Popup,
    MenuPopup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionScope {
    pub id: InteractionScopeId,
    pub root: RuntimeNodeId,
    pub backend_locator: BackendLocator,
    pub parent: Option<InteractionScopeId>,
    pub children: Vec<InteractionScopeId>,
    pub kind: InteractionScopeKind,
    pub label: Option<String>,
    pub active: bool,
    pub modal: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionScopes {
    root: InteractionScopeId,
    active: InteractionScopeId,
    scopes: HashMap<InteractionScopeId, InteractionScope>,
    node_scope: HashMap<RuntimeNodeId, InteractionScopeId>,
}

impl InteractionScopes {
    pub fn analyze(cache: &SemanticCache, graph: &RelationalSemanticGraph<'_>) -> Self {
        let root = InteractionScopeId(cache.root_id());
        let mut scopes = HashMap::new();
        let mut boundaries = Vec::new();
        for node in cache.nodes() {
            if let Some(kind) = scope_kind(
                cache,
                node.runtime_id,
                node.role.clone(),
                node.states.as_slice(),
                graph,
            ) {
                boundaries.push((node.runtime_id, kind));
            }
        }
        if !boundaries.iter().any(|(id, _)| *id == cache.root_id()) {
            boundaries.push((cache.root_id(), InteractionScopeKind::Application));
        }
        for (id, kind) in &boundaries {
            let mut parent = nearest_scope_parent(cache, *id, &boundaries).map(InteractionScopeId);
            if *kind == InteractionScopeKind::ModalDialog && parent == Some(root) {
                let windows: Vec<_> = boundaries
                    .iter()
                    .filter(|(_, candidate_kind)| *candidate_kind == InteractionScopeKind::Window)
                    .map(|(candidate, _)| InteractionScopeId(*candidate))
                    .collect();
                if let [window] = windows.as_slice() {
                    parent = Some(*window);
                }
            }
            let node = cache.node(*id).expect("scope boundary must exist");
            scopes.insert(
                InteractionScopeId(*id),
                InteractionScope {
                    id: InteractionScopeId(*id),
                    root: *id,
                    backend_locator: node.backend_locator.clone(),
                    parent,
                    children: Vec::new(),
                    kind: *kind,
                    label: node.name.clone(),
                    active: false,
                    modal: *kind == InteractionScopeKind::ModalDialog,
                },
            );
        }
        let edges: Vec<_> = scopes
            .values()
            .filter_map(|scope| scope.parent.map(|parent| (parent, scope.id)))
            .collect();
        for (parent, child) in edges {
            if let Some(scope) = scopes.get_mut(&parent) {
                scope.children.push(child);
            }
        }
        let boundary_ids: HashMap<_, _> = boundaries
            .iter()
            .map(|(id, _)| (*id, InteractionScopeId(*id)))
            .collect();
        let mut node_scope = HashMap::new();
        for node in cache.nodes() {
            let scope = nearest_scope(cache, node.runtime_id, &boundary_ids).unwrap_or(root);
            node_scope.insert(node.runtime_id, scope);
        }
        let focused = cache
            .nodes()
            .find(|node| node.states.contains(&SemanticState::Focused))
            .and_then(|node| node_scope.get(&node.runtime_id).copied());
        let modal = scopes
            .values()
            .filter(|scope| scope.modal)
            .max_by_key(|scope| scope_depth(&scopes, scope.id))
            .map(|scope| scope.id);
        let popup = scopes
            .values()
            .filter(|scope| {
                matches!(
                    scope.kind,
                    InteractionScopeKind::Popup | InteractionScopeKind::MenuPopup
                )
            })
            .max_by_key(|scope| scope_depth(&scopes, scope.id))
            .map(|scope| scope.id);
        let active = modal.or(popup).or(focused).unwrap_or_else(|| {
            scopes
                .values()
                .filter(|scope| scope.kind == InteractionScopeKind::Window)
                .max_by_key(|scope| scope_depth(&scopes, scope.id))
                .map_or(root, |scope| scope.id)
        });
        if let Some(scope) = scopes.get_mut(&active) {
            scope.active = true;
        }
        Self {
            root,
            active,
            scopes,
            node_scope,
        }
    }

    pub fn root(&self) -> InteractionScopeId {
        self.root
    }

    pub fn active(&self) -> InteractionScopeId {
        self.active
    }

    pub fn scope(&self, id: InteractionScopeId) -> Option<&InteractionScope> {
        self.scopes.get(&id)
    }

    pub fn scopes(&self) -> impl Iterator<Item = &InteractionScope> {
        self.scopes.values()
    }

    pub fn scope_for_node(&self, id: RuntimeNodeId) -> Option<InteractionScopeId> {
        self.node_scope.get(&id).copied()
    }

    pub fn allows_node(&self, id: RuntimeNodeId) -> bool {
        let Some(node_scope) = self.scope_for_node(id) else {
            return false;
        };
        let confines_interaction = self.scopes.get(&self.active).is_some_and(|scope| {
            scope.modal
                || matches!(
                    scope.kind,
                    InteractionScopeKind::Popup | InteractionScopeKind::MenuPopup
                )
        });
        !confines_interaction || is_descendant_or_same(&self.scopes, node_scope, self.active)
    }
}

fn scope_kind(
    cache: &SemanticCache,
    id: RuntimeNodeId,
    role: SemanticRole,
    states: &[SemanticState],
    graph: &RelationalSemanticGraph<'_>,
) -> Option<InteractionScopeKind> {
    let modal = states
        .iter()
        .any(|state| matches!(state, SemanticState::Other(value) if value == "modal"));
    // Several AT-SPI implementations expose an application-modal dialog as an
    // active Dialog without publishing the optional `modal` state. Treating an
    // active dialog as a modal boundary is the conservative choice: it prevents
    // commands from escaping to the background window while the dialog owns
    // interaction. This is protocol/state based and not toolkit specific.
    let active_dialog = role == SemanticRole::Dialog
        && states.iter().any(|state| {
            matches!(state, SemanticState::Focused)
                || matches!(state, SemanticState::Other(value) if value == "active")
        });
    match role {
        SemanticRole::Application => Some(InteractionScopeKind::Application),
        SemanticRole::Dialog | SemanticRole::Window if modal || active_dialog => {
            Some(InteractionScopeKind::ModalDialog)
        }
        SemanticRole::Dialog => Some(InteractionScopeKind::Dialog),
        SemanticRole::Window if graph.popup_owner(id).is_some() => {
            Some(InteractionScopeKind::Popup)
        }
        SemanticRole::Window => Some(InteractionScopeKind::Window),
        SemanticRole::Menu if graph.popup_owner(id).is_some() => {
            Some(InteractionScopeKind::MenuPopup)
        }
        SemanticRole::List
            if cache
                .node(id)
                .and_then(|node| node.parent)
                .and_then(|parent| cache.node(parent))
                .is_some_and(|parent| {
                    parent.role == SemanticRole::ComboBox
                        && parent.states.contains(&SemanticState::Expanded)
                }) =>
        {
            Some(InteractionScopeKind::Popup)
        }
        SemanticRole::Container
            if cache
                .node(id)
                .and_then(|node| node.parent.map(|parent| (node, parent)))
                .and_then(|(node, parent)| cache.node(parent).map(|parent| (node, parent)))
                .is_some_and(|(node, parent)| {
                    parent.role == SemanticRole::ComboBox
                        && parent.children.len() > 1
                        && parent.children.first().copied() != Some(node.runtime_id)
                }) =>
        {
            Some(InteractionScopeKind::Popup)
        }
        _ => None,
    }
}

fn nearest_scope_parent(
    cache: &SemanticCache,
    id: RuntimeNodeId,
    boundaries: &[(RuntimeNodeId, InteractionScopeKind)],
) -> Option<RuntimeNodeId> {
    let mut parent = cache.node(id)?.parent;
    while let Some(id) = parent {
        if boundaries.iter().any(|(candidate, _)| *candidate == id) {
            return Some(id);
        }
        parent = cache.node(id).and_then(|node| node.parent);
    }
    None
}

fn nearest_scope(
    cache: &SemanticCache,
    mut id: RuntimeNodeId,
    boundaries: &HashMap<RuntimeNodeId, InteractionScopeId>,
) -> Option<InteractionScopeId> {
    loop {
        if let Some(scope) = boundaries.get(&id) {
            return Some(*scope);
        }
        id = cache.node(id)?.parent?;
    }
}

fn scope_depth(
    scopes: &HashMap<InteractionScopeId, InteractionScope>,
    mut id: InteractionScopeId,
) -> usize {
    let mut depth = 0;
    while let Some(parent) = scopes.get(&id).and_then(|scope| scope.parent) {
        depth += 1;
        id = parent;
    }
    depth
}

fn is_descendant_or_same(
    scopes: &HashMap<InteractionScopeId, InteractionScope>,
    mut candidate: InteractionScopeId,
    ancestor: InteractionScopeId,
) -> bool {
    loop {
        if candidate == ancestor {
            return true;
        }
        let Some(parent) = scopes.get(&candidate).and_then(|scope| scope.parent) else {
            return false;
        };
        candidate = parent;
    }
}

pub fn format_scopes(scopes: &InteractionScopes) -> String {
    fn write_scope(
        id: InteractionScopeId,
        scopes: &InteractionScopes,
        depth: usize,
        output: &mut String,
    ) {
        let scope = scopes.scope(id).expect("indexed scope");
        output.push_str(&format!(
            "{}{:?} {:?} [root={} locator={}]{}\n",
            "  ".repeat(depth),
            scope.kind,
            scope.label,
            scope.root,
            scope.backend_locator,
            if scope.active { " [ACTIVE]" } else { "" }
        ));
        for child in &scope.children {
            write_scope(*child, scopes, depth + 1, output);
        }
    }
    let mut output = String::new();
    write_scope(scopes.root, scopes, 0, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, BackendRelation, DebugInfo, SemanticNode, SemanticRelationKind,
    };

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn modal_scope_blocks_background_and_closing_restores_window_scope() {
        let mut app = node(0, SemanticRole::Application, "App");
        let mut window = node(1, SemanticRole::Window, "Main");
        let background = node(2, SemanticRole::Button, "Background");
        let mut dialog = node(3, SemanticRole::Dialog, "Confirm");
        dialog.states.push(SemanticState::Other("modal".to_owned()));
        let dialog_button = node(4, SemanticRole::Button, "Close");
        dialog.children.push(dialog_button);
        window.children = vec![background, dialog];
        app.children.push(window);
        let mut cache = SemanticCache::from_snapshot(app).unwrap();
        let background_id = cache
            .nodes()
            .find(|node| node.name.as_deref() == Some("Background"))
            .unwrap()
            .runtime_id;
        let close_id = cache
            .nodes()
            .find(|node| node.name.as_deref() == Some("Close"))
            .unwrap()
            .runtime_id;
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        assert_eq!(
            scopes.scope(scopes.active()).unwrap().kind,
            InteractionScopeKind::ModalDialog
        );
        assert!(!scopes.allows_node(background_id));
        assert!(scopes.allows_node(close_id));

        let dialog_locator = cache
            .nodes()
            .find(|node| node.role == SemanticRole::Dialog)
            .unwrap()
            .backend_locator
            .clone();
        let replacement = node(5, SemanticRole::Window, "Main");
        let window_locator = cache
            .nodes()
            .find(|node| node.role == SemanticRole::Window)
            .unwrap()
            .backend_locator
            .clone();
        let _ = dialog_locator;
        cache.replace_subtree(&window_locator, replacement).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        assert_eq!(
            scopes.scope(scopes.active()).unwrap().kind,
            InteractionScopeKind::Window
        );
    }

    #[test]
    fn popup_relation_creates_temporary_popup_scope() {
        let mut app = node(0, SemanticRole::Application, "App");
        let mut window = node(1, SemanticRole::Window, "Main");
        let mut menu = node(3, SemanticRole::Menu, "Options");
        menu.states.push(SemanticState::Focused);
        window.children = vec![node(2, SemanticRole::ComboBox, "Choice"), menu];
        app.children.push(window);
        let mut cache = SemanticCache::from_snapshot(app).unwrap();
        let combo = cache
            .nodes()
            .find(|node| node.role == SemanticRole::ComboBox)
            .unwrap();
        let combo_locator = combo.backend_locator.clone();
        let menu = cache
            .nodes()
            .find(|node| node.role == SemanticRole::Menu)
            .unwrap()
            .runtime_id;
        cache
            .set_relations(
                menu,
                vec![BackendRelation {
                    kind: SemanticRelationKind::PopupFor,
                    targets: vec![combo_locator],
                }],
            )
            .unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        assert!(
            scopes.scopes().any(|scope| {
                scope.root == menu && scope.kind == InteractionScopeKind::MenuPopup
            })
        );
        assert_eq!(scopes.scope(scopes.active()).unwrap().root, menu);
        assert!(!scopes.allows_node(RuntimeNodeId::new(2)));
        assert!(scopes.allows_node(menu));
    }
}
