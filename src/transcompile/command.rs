use std::collections::{HashMap, HashSet};

use crate::{
    semantic::{RuntimeNodeId, SemanticCache, SemanticRole, SemanticState},
    tui::action::{InteractionCapability, UiIntent, interaction_capability},
};

use super::scope::{InteractionScopeId, InteractionScopes};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticCommand {
    pub source: RuntimeNodeId,
    pub label: String,
    pub scope: InteractionScopeId,
    pub intent: UiIntent,
    pub enabled: bool,
    pub visible: bool,
    pub shortcut: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandEntry {
    Group(CommandGroup),
    Command(SemanticCommand),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandGroup {
    pub source: RuntimeNodeId,
    pub label: String,
    pub scope: InteractionScopeId,
    pub children: Vec<CommandEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandHierarchy {
    pub root: CommandGroup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedCommand<'a> {
    pub command: &'a SemanticCommand,
    pub score: i32,
    pub reasons: Vec<&'static str>,
    pub path: Vec<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnreachableCommand {
    pub source: RuntimeNodeId,
    pub role: SemanticRole,
    pub name: Option<String>,
    pub intent: UiIntent,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReachabilityAudit {
    pub safe_leaves: usize,
    pub reachable: usize,
    pub structural_reveal_omitted: usize,
    pub unsafe_or_unresolved: usize,
    pub unreachable: usize,
    pub unreachable_commands: Vec<UnreachableCommand>,
}

impl CommandHierarchy {
    pub fn build(cache: &SemanticCache, scopes: &InteractionScopes) -> Self {
        let root_id = cache.root_id();
        let root_scope = scopes.scope_for_node(root_id).unwrap_or(scopes.root());
        let mut root = CommandGroup {
            source: root_id,
            label: cache
                .node(root_id)
                .and_then(|node| node.name.clone())
                .unwrap_or_else(|| "Application commands".to_owned()),
            scope: root_scope,
            children: Vec::new(),
        };
        collect_command_entries(cache, scopes, root_id, &mut root.children);
        Self { root }
    }

    pub fn search<'a>(
        &'a self,
        query: &str,
        scope: InteractionScopeId,
        global: bool,
        recent: &HashMap<RuntimeNodeId, u32>,
    ) -> Vec<RankedCommand<'a>> {
        let query = query.to_lowercase();
        let mut output = Vec::new();
        flatten_ranked(
            &self.root,
            &mut Vec::new(),
            &query,
            scope,
            global,
            recent,
            &mut output,
        );
        output.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.command.label.cmp(&right.command.label))
                .then_with(|| left.command.source.cmp(&right.command.source))
        });
        output
    }

    pub fn audit(&self, cache: &SemanticCache) -> ReachabilityAudit {
        let reachable: HashSet<_> = all_commands(&self.root)
            .map(|command| command.source)
            .collect();
        let mut audit = ReachabilityAudit {
            reachable: reachable.len(),
            ..Default::default()
        };
        for node in cache.nodes() {
            let parent_capabilities = node
                .parent
                .and_then(|parent| cache.node(parent))
                .map_or(&[][..], |parent| parent.capabilities.as_slice());
            let capability = interaction_capability(
                &node.role,
                &node.actions,
                &node.capabilities,
                parent_capabilities,
            );
            let structural_reveal = capability != InteractionCapability::None
                && has_safe_command_descendant(cache, node.runtime_id);
            if structural_reveal {
                audit.structural_reveal_omitted += 1;
                continue;
            }
            if capability != InteractionCapability::None {
                audit.safe_leaves += 1;
                if !reachable.contains(&node.runtime_id) && is_command_role(&node.role) {
                    audit.unreachable += 1;
                    audit.unreachable_commands.push(UnreachableCommand {
                        source: node.runtime_id,
                        role: node.role.clone(),
                        name: node.name.clone(),
                        intent: capability_intent(capability)
                            .expect("non-empty command capability maps to an intent"),
                        reason: "safe semantic leaf was not indexed by CommandHierarchy",
                    });
                }
            } else if !node.actions.is_empty() && is_command_role(&node.role) {
                audit.unsafe_or_unresolved += 1;
            }
        }
        audit.reachable = audit.safe_leaves.saturating_sub(audit.unreachable);
        audit
    }
}

fn has_safe_command_descendant(cache: &SemanticCache, id: RuntimeNodeId) -> bool {
    let Some(node) = cache.node(id) else {
        return false;
    };
    node.children.iter().any(|child_id| {
        let Some(child) = cache.node(*child_id) else {
            return false;
        };
        let parent_capabilities = node.capabilities.as_slice();
        (is_command_role(&child.role)
            && interaction_capability(
                &child.role,
                &child.actions,
                &child.capabilities,
                parent_capabilities,
            ) != InteractionCapability::None)
            || has_safe_command_descendant(cache, *child_id)
    })
}

fn collect_command_entries(
    cache: &SemanticCache,
    scopes: &InteractionScopes,
    id: RuntimeNodeId,
    output: &mut Vec<CommandEntry>,
) {
    let Some(node) = cache.node(id) else { return };
    for child_id in &node.children {
        let Some(child) = cache.node(*child_id) else {
            continue;
        };
        let scope = scopes.scope_for_node(*child_id).unwrap_or(scopes.root());
        let parent_capabilities = node.capabilities.as_slice();
        let capability = interaction_capability(
            &child.role,
            &child.actions,
            &child.capabilities,
            parent_capabilities,
        );
        if is_command_container(&child.role)
            || child.children.iter().any(|nested| {
                cache
                    .node(*nested)
                    .is_some_and(|node| is_command_role(&node.role))
            })
        {
            let mut group = CommandGroup {
                source: child.runtime_id,
                label: child.name.clone().unwrap_or_else(|| child.role.to_string()),
                scope,
                children: Vec::new(),
            };
            collect_command_entries(cache, scopes, child.runtime_id, &mut group.children);
            if !group.children.is_empty() {
                output.push(CommandEntry::Group(group));
                continue;
            }
        }
        if is_command_role(&child.role)
            && let Some(intent) = capability_intent(capability)
        {
            output.push(CommandEntry::Command(SemanticCommand {
                source: child.runtime_id,
                label: child
                    .name
                    .clone()
                    .unwrap_or_else(|| child.role.to_string()),
                scope,
                intent,
                enabled: !child.states.iter().any(
                    |state| matches!(state, SemanticState::Other(value) if value == "disabled")
                ),
                visible: child.states.iter().any(|state| matches!(state, SemanticState::Other(value) if value == "showing" || value == "visible"))
                    || !child.states.iter().any(|state| matches!(state, SemanticState::Other(value) if value == "hidden")),
                shortcut: child.actions.iter().find_map(|action| action.keybinding.clone()),
            }));
        } else {
            collect_command_entries(cache, scopes, child.runtime_id, output);
        }
    }
}

fn flatten_ranked<'a>(
    group: &'a CommandGroup,
    path: &mut Vec<&'a str>,
    query: &str,
    scope: InteractionScopeId,
    global: bool,
    recent: &HashMap<RuntimeNodeId, u32>,
    output: &mut Vec<RankedCommand<'a>>,
) {
    path.push(&group.label);
    for child in &group.children {
        match child {
            CommandEntry::Group(group) => {
                flatten_ranked(group, path, query, scope, global, recent, output)
            }
            CommandEntry::Command(command) => {
                if (!global && command.scope != scope)
                    || (!query.is_empty()
                        && !command.label.to_lowercase().contains(query)
                        && !path.iter().any(|part| part.to_lowercase().contains(query)))
                {
                    continue;
                }
                let mut score = 0;
                let mut reasons = Vec::new();
                if command.scope == scope {
                    score += 50;
                    reasons.push("current-scope +50");
                }
                if command.enabled {
                    score += 10;
                    reasons.push("enabled +10");
                }
                if command.visible {
                    score += 5;
                    reasons.push("visible +5");
                }
                if !query.is_empty() && command.label.to_lowercase() == query {
                    score += 20;
                    reasons.push("exact-query +20");
                }
                if recent.get(&command.source).copied().unwrap_or(0) > 0 {
                    score += 3;
                    reasons.push("recent-use +3");
                }
                output.push(RankedCommand {
                    command,
                    score,
                    reasons,
                    path: path.clone(),
                });
            }
        }
    }
    path.pop();
}

fn all_commands(group: &CommandGroup) -> impl Iterator<Item = &SemanticCommand> {
    group.children.iter().flat_map(|entry| match entry {
        CommandEntry::Command(command) => vec![command],
        CommandEntry::Group(group) => all_commands(group).collect(),
    })
}

fn is_command_role(role: &SemanticRole) -> bool {
    matches!(
        role,
        SemanticRole::MenuItem
            | SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::RadioButton
    )
}

fn is_command_container(role: &SemanticRole) -> bool {
    matches!(role, SemanticRole::MenuBar | SemanticRole::Menu)
        || matches!(role, SemanticRole::Unknown(value) if value == "tool bar" || value == "toolbar")
}

fn capability_intent(capability: InteractionCapability) -> Option<UiIntent> {
    match capability {
        InteractionCapability::Activate => Some(UiIntent::Activate),
        InteractionCapability::Toggle => Some(UiIntent::Toggle),
        InteractionCapability::Select => Some(UiIntent::Select),
        InteractionCapability::Choose => Some(UiIntent::BeginChoice),
        InteractionCapability::OpenMenu => Some(UiIntent::OpenMenu),
        InteractionCapability::EditText | InteractionCapability::None => None,
    }
}

pub fn format_commands(
    hierarchy: &CommandHierarchy,
    scopes: &InteractionScopes,
    query: &str,
    global: bool,
) -> String {
    let mut output = format!("Current scope: {}\n", scopes.active());
    for ranked in hierarchy.search(query, scopes.active(), global, &HashMap::new()) {
        output.push_str(&format!(
            "{}\n  path={} enabled={} visible={} score={} reasons={}\n",
            ranked.command.label,
            ranked.path.join(" > "),
            ranked.command.enabled,
            ranked.command.visible,
            ranked.score,
            ranked.reasons.join(", ")
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, DebugInfo, RelationalSemanticGraph, SemanticAction, SemanticNode,
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
            states: vec![
                SemanticState::Enabled,
                SemanticState::Other("showing".to_owned()),
            ],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        }
    }

    fn command(id: u64, name: &str) -> SemanticNode {
        let mut node = node(id, SemanticRole::MenuItem, name);
        node.actions.push(SemanticAction {
            index: 0,
            name: "Press".to_owned(),
            description: None,
            keybinding: Some(format!("Alt+{id}")),
        });
        node
    }

    fn hierarchy() -> (SemanticCache, InteractionScopes, CommandHierarchy) {
        let mut app = node(0, SemanticRole::Application, "App");
        let mut window = node(1, SemanticRole::Window, "Main");
        let mut menu_bar = node(2, SemanticRole::MenuBar, "Menus");
        let mut file = node(3, SemanticRole::Menu, "File");
        file.children = vec![command(4, "Open"), command(5, "Duplicate")];
        let mut edit = node(6, SemanticRole::Menu, "Edit");
        edit.children = vec![command(7, "Duplicate")];
        menu_bar.children = vec![file, edit];
        window.children.push(menu_bar);
        app.children.push(window);
        let cache = SemanticCache::from_snapshot(app).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        let hierarchy = CommandHierarchy::build(&cache, &scopes);
        (cache, scopes, hierarchy)
    }

    #[test]
    fn hierarchy_is_canonical_and_flat_search_preserves_duplicate_names() {
        let (_cache, scopes, hierarchy) = hierarchy();
        assert!(matches!(hierarchy.root.children[0], CommandEntry::Group(_)));
        let results = hierarchy.search("duplicate", scopes.active(), true, &HashMap::new());
        assert_eq!(results.len(), 2);
        assert_ne!(results[0].command.source, results[1].command.source);
        assert_ne!(results[0].path, results[1].path);
    }

    #[test]
    fn scope_filter_precedes_explainable_ranking_and_search_can_expand() {
        let (_cache, scopes, hierarchy) = hierarchy();
        let scoped = hierarchy.search("", scopes.active(), false, &HashMap::new());
        assert_eq!(scoped.len(), 3);
        assert!(scoped.iter().all(|ranked| ranked.score >= 65));
        assert!(scoped[0].reasons.contains(&"current-scope +50"));
    }

    #[test]
    fn anonymous_actions_are_unsafe_and_not_counted_as_reachable_commands() {
        let mut app = node(0, SemanticRole::Application, "App");
        let mut window = node(1, SemanticRole::Window, "Main");
        let mut anonymous = node(2, SemanticRole::Button, "Browser button");
        anonymous.actions.push(SemanticAction {
            index: 0,
            name: String::new(),
            description: None,
            keybinding: None,
        });
        window.children.push(anonymous);
        app.children.push(window);
        let cache = SemanticCache::from_snapshot(app).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        let hierarchy = CommandHierarchy::build(&cache, &scopes);
        let audit = hierarchy.audit(&cache);
        assert_eq!(audit.safe_leaves, 0);
        assert_eq!(audit.unsafe_or_unresolved, 1);
    }
}
