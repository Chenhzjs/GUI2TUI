use std::collections::{HashMap, HashSet};

use crate::{
    semantic::{
        CollectionCompleteness, RuntimeNodeId, SemanticAction, SemanticCache, SemanticCapability,
        SemanticRole, SemanticState, collection_completeness,
    },
    tui::action::{UiIntent, resolve_action},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticChoice {
    pub owner: RuntimeNodeId,
    pub current: Option<RuntimeNodeId>,
    pub options: ChoiceOptions,
    pub disclosure: DisclosureRequirement,
    pub dismiss: DismissBehavior,
    pub completeness: CollectionCompleteness,
}

impl SemanticChoice {
    pub fn is_interactive(&self) -> bool {
        self.options
            .options()
            .iter()
            .any(|option| option.selection.is_some() && option.enabled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChoiceOptions {
    Available(Vec<ChoiceOption>),
    Partial(Vec<ChoiceOption>),
    Unavailable,
}

impl ChoiceOptions {
    pub fn options(&self) -> &[ChoiceOption] {
        match self {
            Self::Available(options) | Self::Partial(options) => options,
            Self::Unavailable => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    pub runtime_id: RuntimeNodeId,
    pub label: String,
    pub selected: bool,
    pub enabled: bool,
    pub selection: Option<ChoiceSelectionStrategy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChoiceSelectionStrategy {
    ParentSelection {
        parent: RuntimeNodeId,
        child_index: usize,
    },
    ChildSemanticAction {
        child: RuntimeNodeId,
        action: SemanticAction,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisclosureRequirement {
    NotRequired,
    RequiredForDiscovery,
    Optional,
    Unavailable,
}

impl DisclosureRequirement {
    pub const fn permits_production_disclosure(self, dismiss: DismissBehavior) -> bool {
        matches!(self, Self::RequiredForDiscovery | Self::Optional)
            && matches!(
                dismiss,
                DismissBehavior::AutoAfterSelection | DismissBehavior::ExplicitSemanticDismiss
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissBehavior {
    NotApplicable,
    AutoAfterSelection,
    ExplicitSemanticDismiss,
    NoSafeDismiss,
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct ChoiceCatalog {
    by_owner: HashMap<RuntimeNodeId, SemanticChoice>,
}

impl ChoiceCatalog {
    pub fn discover(cache: &SemanticCache) -> Self {
        let mut by_owner = HashMap::new();
        for node in cache.nodes() {
            match node.role {
                SemanticRole::ComboBox | SemanticRole::List => {
                    let choice = discover_owned_choice(cache, node.runtime_id);
                    by_owner.insert(node.runtime_id, choice);
                }
                _ => {}
            }
        }

        // A radio group is expressed by sibling membership, not a toolkit name.
        for parent in cache.nodes() {
            let radios: Vec<_> = parent
                .children
                .iter()
                .filter_map(|id| cache.node(*id))
                .filter(|node| node.role == SemanticRole::RadioButton)
                .map(|node| node.runtime_id)
                .collect();
            if radios.len() > 1 {
                let choice = choice_from_candidates(cache, parent.runtime_id, &radios);
                by_owner.insert(parent.runtime_id, choice);
            }
        }
        Self { by_owner }
    }

    pub fn get(&self, owner: RuntimeNodeId) -> Option<&SemanticChoice> {
        self.by_owner.get(&owner)
    }

    pub fn choices(&self) -> impl Iterator<Item = &SemanticChoice> {
        self.by_owner.values()
    }
}

pub fn format_choices(cache: &SemanticCache, catalog: &ChoiceCatalog) -> String {
    let mut choices: Vec<_> = catalog.choices().collect();
    choices.sort_by_key(|choice| choice.owner);
    let mut output = String::new();
    for choice in choices {
        let owner = cache.node(choice.owner);
        output.push_str(&format!(
            "Choice owner={} role={} name={:?} current={:?} completeness={:?} disclosure={:?} dismiss={:?} interactive={}\n",
            choice.owner,
            owner.map_or_else(|| "Unknown".to_owned(), |node| node.role.to_string()),
            owner.and_then(|node| node.name.as_deref()),
            choice.current,
            choice.completeness,
            choice.disclosure,
            choice.dismiss,
            choice.is_interactive(),
        ));
        match &choice.options {
            ChoiceOptions::Unavailable => output.push_str("  options=Unavailable\n"),
            ChoiceOptions::Available(options) | ChoiceOptions::Partial(options) => {
                for option in options {
                    output.push_str(&format!(
                        "  option={} label={:?} selected={} enabled={} strategy={:?}\n",
                        option.runtime_id,
                        option.label,
                        option.selected,
                        option.enabled,
                        option.selection,
                    ));
                }
            }
        }
    }
    output
}

fn discover_owned_choice(cache: &SemanticCache, owner: RuntimeNodeId) -> SemanticChoice {
    let Some(node) = cache.node(owner) else {
        return unavailable(owner);
    };
    let mut candidates = Vec::new();
    let mut visited = HashSet::new();
    collect_semantic_options(cache, owner, &mut candidates, &mut visited);
    let mut choice = choice_from_candidates(cache, owner, &candidates);
    if matches!(choice.options, ChoiceOptions::Unavailable) {
        let has_disclosure = resolve_action(&node.role, &node.actions, UiIntent::OpenMenu).is_ok()
            || node.children.iter().any(|id| {
                cache.node(*id).is_some_and(|child| {
                    resolve_action(&SemanticRole::ComboBox, &child.actions, UiIntent::OpenMenu)
                        .is_ok()
                })
            });
        choice.disclosure = if has_disclosure {
            DisclosureRequirement::RequiredForDiscovery
        } else {
            DisclosureRequirement::Unavailable
        };
        choice.dismiss = if has_disclosure {
            DismissBehavior::NoSafeDismiss
        } else {
            DismissBehavior::Unknown
        };
    }
    choice
}

fn collect_semantic_options(
    cache: &SemanticCache,
    id: RuntimeNodeId,
    output: &mut Vec<RuntimeNodeId>,
    visited: &mut HashSet<RuntimeNodeId>,
) {
    if !visited.insert(id) {
        return;
    }
    let Some(node) = cache.node(id) else { return };
    for child_id in &node.children {
        let Some(child) = cache.node(*child_id) else {
            continue;
        };
        if matches!(
            child.role,
            SemanticRole::ListItem | SemanticRole::MenuItem | SemanticRole::RadioButton
        ) && semantic_option_label(cache, child).is_some()
        {
            output.push(*child_id);
        } else {
            collect_semantic_options(cache, *child_id, output, visited);
        }
    }
}

fn choice_from_candidates(
    cache: &SemanticCache,
    owner: RuntimeNodeId,
    candidates: &[RuntimeNodeId],
) -> SemanticChoice {
    let mut options = Vec::new();
    let mut completeness = cache
        .node(owner)
        .map(collection_completeness)
        .unwrap_or(CollectionCompleteness::Unknown);
    for id in candidates {
        let Some(node) = cache.node(*id) else {
            continue;
        };
        let Some(label) = semantic_option_label(cache, node) else {
            continue;
        };
        let selection = child_selection_strategy(cache, *id);
        let enabled = semantically_enabled(&node.states)
            || node
                .parent
                .and_then(|parent| cache.node(parent))
                .is_some_and(|parent| semantically_enabled(&parent.states));
        if let Some(parent) = node.parent.and_then(|id| cache.node(id)) {
            completeness = merge_completeness(completeness, collection_completeness(parent));
        }
        options.push(ChoiceOption {
            runtime_id: *id,
            label,
            selected: node.states.contains(&SemanticState::Selected)
                || node.states.contains(&SemanticState::Checked),
            enabled,
            selection,
        });
    }
    if options.is_empty() {
        return unavailable(owner);
    }
    let current = options
        .iter()
        .find(|option| option.selected)
        .or_else(|| {
            cache.node(owner).and_then(|owner_node| {
                owner_node
                    .name
                    .as_deref()
                    .and_then(|name| options.iter().find(|option| option.label == name))
            })
        })
        .map(|option| option.runtime_id);
    let options = if completeness == CollectionCompleteness::Complete {
        ChoiceOptions::Available(options)
    } else {
        ChoiceOptions::Partial(options)
    };
    SemanticChoice {
        owner,
        current,
        options,
        disclosure: DisclosureRequirement::NotRequired,
        dismiss: DismissBehavior::NotApplicable,
        completeness,
    }
}

fn semantically_enabled(states: &[SemanticState]) -> bool {
    states.iter().any(|state| {
        matches!(state, SemanticState::Enabled)
            || matches!(state, SemanticState::Other(value) if value == "sensitive")
    })
}

fn semantic_option_label(
    cache: &SemanticCache,
    node: &crate::semantic::CachedSemanticNode,
) -> Option<String> {
    node.name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            let mut labels = Vec::new();
            collect_cached_text_labels(cache, node, &mut labels);
            labels.sort();
            labels.dedup();
            (labels.len() == 1).then(|| labels.remove(0))
        })
}

fn collect_cached_text_labels(
    cache: &SemanticCache,
    node: &crate::semantic::CachedSemanticNode,
    labels: &mut Vec<String>,
) {
    for child_id in &node.children {
        let Some(child) = cache.node(*child_id) else {
            continue;
        };
        if matches!(child.role, SemanticRole::Label | SemanticRole::Text) {
            if let Some(label) = child.name.clone().or_else(|| child.value.clone())
                && !label.trim().is_empty()
            {
                labels.push(label);
            }
        } else if !matches!(
            child.role,
            SemanticRole::ListItem | SemanticRole::MenuItem | SemanticRole::RadioButton
        ) {
            collect_cached_text_labels(cache, child, labels);
        }
    }
}

fn child_selection_strategy(
    cache: &SemanticCache,
    child: RuntimeNodeId,
) -> Option<ChoiceSelectionStrategy> {
    let node = cache.node(child)?;
    let intent = match node.role {
        SemanticRole::ListItem => UiIntent::Select,
        SemanticRole::RadioButton => UiIntent::Toggle,
        SemanticRole::MenuItem => UiIntent::Activate,
        _ => return None,
    };
    if let Ok(action) = resolve_action(&node.role, &node.actions, intent) {
        return Some(ChoiceSelectionStrategy::ChildSemanticAction {
            child,
            action: action.clone(),
        });
    }
    let parent = node.parent?;
    let parent_node = cache.node(parent)?;
    if parent_node
        .capabilities
        .contains(&SemanticCapability::SelectChildren)
        && parent_node
            .states
            .iter()
            .any(|state| matches!(state, SemanticState::Other(value) if value == "showing"))
    {
        return node
            .index_in_parent
            .map(|child_index| ChoiceSelectionStrategy::ParentSelection {
                parent,
                child_index,
            });
    }
    None
}

fn unavailable(owner: RuntimeNodeId) -> SemanticChoice {
    SemanticChoice {
        owner,
        current: None,
        options: ChoiceOptions::Unavailable,
        disclosure: DisclosureRequirement::Unavailable,
        dismiss: DismissBehavior::Unknown,
        completeness: CollectionCompleteness::Unknown,
    }
}

fn merge_completeness(
    left: CollectionCompleteness,
    right: CollectionCompleteness,
) -> CollectionCompleteness {
    match (left, right) {
        (CollectionCompleteness::Unknown, _) | (_, CollectionCompleteness::Unknown) => {
            CollectionCompleteness::Unknown
        }
        (CollectionCompleteness::PartialRealized, _)
        | (_, CollectionCompleteness::PartialRealized) => CollectionCompleteness::PartialRealized,
        _ => CollectionCompleteness::Complete,
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, DebugInfo, SemanticNode, SemanticState, TextInputKind, TreeTruncation,
    };

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: (!name.is_empty()).then(|| name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None::<TextInputKind>,
            states: vec![SemanticState::Enabled],
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::<TreeTruncation>::new(),
            debug: DebugInfo::default(),
        }
    }

    fn action(name: &str) -> SemanticAction {
        SemanticAction {
            index: 0,
            name: name.to_owned(),
            description: None,
            keybinding: None,
        }
    }

    #[test]
    fn exposed_combo_children_form_interactive_choice_without_disclosure() {
        let mut combo = node(1, SemanticRole::ComboBox, "Demo choice");
        let mut list = node(2, SemanticRole::List, "Options");
        let mut alpha = node(3, SemanticRole::ListItem, "Alpha");
        alpha.index_in_parent = Some(0);
        alpha.states.push(SemanticState::Selected);
        alpha.actions.push(action("Toggle"));
        let mut beta = node(4, SemanticRole::ListItem, "Beta");
        beta.index_in_parent = Some(1);
        beta.actions.push(action("Toggle"));
        list.children = vec![alpha, beta];
        combo.children.push(list);
        let cache = SemanticCache::from_snapshot(combo).unwrap();
        let choice = ChoiceCatalog::discover(&cache)
            .get(cache.root_id())
            .unwrap()
            .clone();
        assert!(choice.is_interactive());
        assert_eq!(choice.options.options().len(), 2);
        assert_eq!(choice.disclosure, DisclosureRequirement::NotRequired);
        assert_eq!(choice.dismiss, DismissBehavior::NotApplicable);
        assert!(matches!(
            choice.options.options()[1].selection,
            Some(ChoiceSelectionStrategy::ChildSemanticAction { .. })
        ));
    }

    #[test]
    fn parent_selection_is_a_safe_choice_strategy() {
        let mut combo = node(1, SemanticRole::ComboBox, "Browser choice");
        let mut menu = node(2, SemanticRole::Menu, "Options");
        menu.capabilities.push(SemanticCapability::SelectChildren);
        menu.states.push(SemanticState::Other("showing".to_owned()));
        let mut beta = node(3, SemanticRole::MenuItem, "Beta");
        beta.index_in_parent = Some(7);
        beta.actions.push(action(""));
        menu.children.push(beta);
        combo.children.push(menu);
        let cache = SemanticCache::from_snapshot(combo).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert!(choice.is_interactive());
        assert!(matches!(
            choice.options.options()[0].selection,
            Some(ChoiceSelectionStrategy::ParentSelection { child_index: 7, .. })
        ));
    }

    #[test]
    fn unavailable_and_anonymous_options_are_read_only_without_guessing() {
        let mut empty = node(1, SemanticRole::ComboBox, "GTK choice");
        let mut disclosure = node(2, SemanticRole::ToggleButton, "Alpha");
        disclosure.actions.push(action("Click"));
        empty.children.push(disclosure);
        let cache = SemanticCache::from_snapshot(empty).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert_eq!(choice.options, ChoiceOptions::Unavailable);
        assert!(!choice.is_interactive());
        assert_eq!(
            choice.disclosure,
            DisclosureRequirement::RequiredForDiscovery
        );
        assert_eq!(choice.dismiss, DismissBehavior::NoSafeDismiss);

        let mut combo = node(10, SemanticRole::ComboBox, "Unsafe");
        let mut menu = node(11, SemanticRole::Menu, "Options");
        let mut item = node(12, SemanticRole::MenuItem, "Alpha");
        item.actions.push(action(""));
        menu.children.push(item);
        combo.children.push(menu);
        let cache = SemanticCache::from_snapshot(combo).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert!(!choice.is_interactive());
        assert!(choice.options.options()[0].selection.is_none());
    }

    #[test]
    fn hidden_parent_selection_is_not_claimed_as_a_safe_choice() {
        let mut combo = node(1, SemanticRole::ComboBox, "Browser choice");
        let mut menu = node(2, SemanticRole::Menu, "Options");
        menu.capabilities.push(SemanticCapability::SelectChildren);
        let mut beta = node(3, SemanticRole::MenuItem, "Beta");
        beta.index_in_parent = Some(1);
        beta.actions.push(action(""));
        menu.children.push(beta);
        combo.children.push(menu);
        let cache = SemanticCache::from_snapshot(combo).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert!(!choice.is_interactive());
        assert!(choice.options.options()[0].selection.is_none());
    }

    #[test]
    fn managed_collection_is_partial_not_empty_or_complete() {
        let mut list = node(1, SemanticRole::List, "Items");
        list.states
            .push(SemanticState::Other("manages-descendants".to_owned()));
        let mut alpha = node(2, SemanticRole::ListItem, "Alpha");
        alpha.actions.push(action("Toggle"));
        list.children.push(alpha);
        let cache = SemanticCache::from_snapshot(list).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert!(matches!(choice.options, ChoiceOptions::Partial(_)));
        assert_eq!(choice.completeness, CollectionCompleteness::PartialRealized);
    }

    #[test]
    fn radio_siblings_share_the_choice_contract() {
        let mut group = node(1, SemanticRole::Container, "Theme");
        let mut light = node(2, SemanticRole::RadioButton, "Light");
        light.states.push(SemanticState::Checked);
        light.actions.push(action("Toggle"));
        let mut dark = node(3, SemanticRole::RadioButton, "Dark");
        dark.actions.push(action("Toggle"));
        group.children = vec![light, dark];
        let cache = SemanticCache::from_snapshot(group).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert!(choice.is_interactive());
        assert_eq!(choice.options.options().len(), 2);
        assert!(choice.current.is_some());
    }

    #[test]
    fn unnamed_list_items_use_a_unique_descendant_text_label() {
        let mut list = node(1, SemanticRole::List, "Items");
        list.capabilities.push(SemanticCapability::SelectChildren);
        list.states.push(SemanticState::Other("showing".to_owned()));
        let mut item = node(2, SemanticRole::ListItem, "");
        item.index_in_parent = Some(0);
        let mut wrapper = node(3, SemanticRole::Container, "");
        wrapper.children.push(node(4, SemanticRole::Label, "Alpha"));
        item.children.push(wrapper);
        list.children.push(item);
        let cache = SemanticCache::from_snapshot(list).unwrap();
        let catalog = ChoiceCatalog::discover(&cache);
        let choice = catalog.get(cache.root_id()).unwrap();
        assert_eq!(choice.options.options()[0].label, "Alpha");
        assert!(matches!(
            choice.options.options()[0].selection,
            Some(ChoiceSelectionStrategy::ParentSelection { child_index: 0, .. })
        ));
    }

    #[test]
    fn disclosure_requires_an_observed_safe_dismiss_contract() {
        assert!(
            !DisclosureRequirement::NotRequired
                .permits_production_disclosure(DismissBehavior::NotApplicable)
        );
        assert!(
            DisclosureRequirement::RequiredForDiscovery
                .permits_production_disclosure(DismissBehavior::AutoAfterSelection)
        );
        assert!(
            DisclosureRequirement::Optional
                .permits_production_disclosure(DismissBehavior::ExplicitSemanticDismiss)
        );
        assert!(
            !DisclosureRequirement::RequiredForDiscovery
                .permits_production_disclosure(DismissBehavior::NoSafeDismiss)
        );
        assert!(
            !DisclosureRequirement::RequiredForDiscovery
                .permits_production_disclosure(DismissBehavior::Unknown)
        );
        assert!(
            !DisclosureRequirement::Unavailable
                .permits_production_disclosure(DismissBehavior::AutoAfterSelection)
        );
    }
}
