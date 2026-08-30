use crate::semantic::{
    CollectionCompleteness, RuntimeNodeId, SemanticCache, SemanticRole, SemanticState,
    collection_completeness,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualCollectionModel {
    pub owner: RuntimeNodeId,
    pub completeness: CollectionCompleteness,
    pub realized_items: Vec<RuntimeNodeId>,
    pub active_descendant: Option<RuntimeNodeId>,
    pub selected_items: Vec<RuntimeNodeId>,
    pub known_total: Option<usize>,
}

impl VirtualCollectionModel {
    pub fn analyze(cache: &SemanticCache, owner: RuntimeNodeId) -> Option<Self> {
        let node = cache.node(owner)?;
        if !matches!(
            node.role,
            SemanticRole::List | SemanticRole::Tree | SemanticRole::Table
        ) {
            return None;
        }
        let realized_items = node.children.clone();
        let selected_items = realized_items
            .iter()
            .filter(|id| {
                cache.node(**id).is_some_and(|child| {
                    child.states.contains(&SemanticState::Selected)
                        || child.states.contains(&SemanticState::Checked)
                })
            })
            .copied()
            .collect();
        Some(Self {
            owner,
            completeness: collection_completeness(node),
            realized_items,
            active_descendant: None,
            selected_items,
            // AT-SPI child count is a realized-object count for managed
            // descendants and is not a generic logical-total contract.
            known_total: None,
        })
    }

    pub fn apply_active_descendant(&mut self, descendant: Option<RuntimeNodeId>) {
        self.active_descendant = descendant;
        if let Some(id) = descendant
            && !self.realized_items.contains(&id)
        {
            self.realized_items.push(id);
        }
    }

    pub fn rebuild_realized(&mut self, cache: &SemanticCache) {
        if let Some(fresh) = Self::analyze(cache, self.owner) {
            let active = self.active_descendant;
            *self = fresh;
            self.apply_active_descendant(active.filter(|id| cache.node(*id).is_some()));
        }
    }
}

pub fn analyze_virtual_collections(cache: &SemanticCache) -> Vec<VirtualCollectionModel> {
    let mut models: Vec<_> = cache
        .nodes()
        .filter(|node| {
            matches!(
                node.role,
                SemanticRole::List | SemanticRole::Tree | SemanticRole::Table
            ) && collection_completeness(node) != CollectionCompleteness::Complete
        })
        .filter_map(|node| VirtualCollectionModel::analyze(cache, node.runtime_id))
        .collect();
    models.sort_by_key(|model| model.owner);
    models
}

pub fn format_virtual_collections(
    cache: &SemanticCache,
    models: &[VirtualCollectionModel],
) -> String {
    let mut output = String::new();
    for model in models {
        let owner = cache.node(model.owner);
        output.push_str(&format!(
            "VirtualCollection owner={} role={} name={:?} completeness={:?} realized={} selected={} active={:?} known_total={:?}\n",
            model.owner,
            owner.map_or_else(|| "Unknown".to_owned(), |node| node.role.to_string()),
            owner.and_then(|node| node.name.as_deref()),
            model.completeness,
            model.realized_items.len(),
            model.selected_items.len(),
            model.active_descendant,
            model.known_total,
        ));
        for item in &model.realized_items {
            let node = cache.node(*item);
            output.push_str(&format!(
                "  realized={} role={} name={:?} selected={}\n",
                item,
                node.map_or_else(|| "Unknown".to_owned(), |node| node.role.to_string()),
                node.and_then(|node| node.name.as_deref()),
                model.selected_items.contains(item),
            ));
        }
        if model.completeness != CollectionCompleteness::Complete {
            output.push_str(
                "  note=more items may exist outside the realized accessibility window\n",
            );
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, DebugInfo, SemanticNode};

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.4", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
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
    fn partial_collection_never_fabricates_total_or_items() {
        let mut list = node(1, SemanticRole::List, "Results");
        list.states
            .push(SemanticState::Other("manages-descendants".to_owned()));
        list.children
            .push(node(2, SemanticRole::ListItem, "Result 100"));
        list.children
            .push(node(3, SemanticRole::ListItem, "Result 101"));
        let cache = SemanticCache::from_snapshot(list).unwrap();
        let model = VirtualCollectionModel::analyze(&cache, cache.root_id()).unwrap();
        assert_eq!(model.completeness, CollectionCompleteness::PartialRealized);
        assert_eq!(model.realized_items.len(), 2);
        assert_eq!(model.known_total, None);
    }

    #[test]
    fn active_descendant_can_be_realized_without_claiming_completeness() {
        let mut list = node(1, SemanticRole::List, "Results");
        list.states
            .push(SemanticState::Other("manages-descendants".to_owned()));
        let cache = SemanticCache::from_snapshot(list).unwrap();
        let mut model = VirtualCollectionModel::analyze(&cache, cache.root_id()).unwrap();
        let active = RuntimeNodeId::new(99);
        model.apply_active_descendant(Some(active));
        assert_eq!(model.active_descendant, Some(active));
        assert!(model.realized_items.contains(&active));
        assert_eq!(model.known_total, None);
    }
}
