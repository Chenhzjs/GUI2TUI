//! Bounded local navigation over the existing Human TUI. Never activates a GUI.
use super::*;
use crate::transcompile::document;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn matches_text(value: &str, query: &str, exact: bool) -> bool {
    if exact {
        value.to_lowercase() == query.to_lowercase()
    } else {
        value.to_lowercase().contains(&query.to_lowercase())
    }
}

// A public name is searchable independently of its role/path decoration.
fn matches_palette_text(
    label: &str,
    title: &str,
    public_name: Option<&str>,
    query: &str,
    exact: bool,
) -> bool {
    matches_text(label, query, exact)
        || matches_text(title, query, exact)
        || public_name.is_some_and(|name| matches_text(name, query, exact))
}

fn matches_current_palette_entry(
    cache: &SemanticCache,
    scopes: &InteractionScopes,
    entry: &crate::tui::palette::PaletteEntry,
    query: &str,
    context: Option<&str>,
    exact: bool,
) -> bool {
    let Some((target, locator)) = entry.runtime_target() else {
        return false;
    };
    let Some(node) = cache.node(target).filter(|node| {
        node.backend_locator == *locator
            && scopes.allows_node(target)
            && !document::secret_source(cache, target)
    }) else {
        return false;
    };
    matches_palette_text(
        &entry.label,
        &entry.title,
        node.name.as_deref(),
        query,
        exact,
    ) && context.is_none_or(|q| {
        matches_text(&entry.label, q, false) || matches_text(&entry.title, q, false)
    })
}

impl TuiApplication {
    pub(super) fn refresh_tui_palette_query(&mut self) {
        if let Some(palette) = self.command_palette.as_mut()
            && let Some((query, context, exact)) = palette.navigation_query()
        {
            // Re-evaluate the literal query on current sources, never retain old IDs.
            palette.retain_entries(|entry| {
                matches_current_palette_entry(
                    &self.cache,
                    &self.scopes,
                    entry,
                    &query,
                    context.as_deref(),
                    exact,
                )
            });
        }
    }
    /// Mirror leaving the Human palette before an explicitly requested GUI use.
    pub fn agent_tui_prepare_execution(&mut self) {
        self.command_palette = None;
    }

    // Keep the explicit protocol fields at this adapter boundary.
    #[allow(clippy::too_many_arguments)]
    pub async fn agent_tui_navigate(
        &mut self,
        observed: &AgentObservation,
        action: &str,
        query: Option<&str>,
        context: Option<&str>,
        exact: bool,
        max_steps: u16,
        width: u16,
        height: u16,
    ) -> Result<Value, String> {
        // Validate source owner and dimensions before changing local state.
        self.agent_render_tui_text(observed, width, height)?;
        if !(1..=256).contains(&max_steps) {
            return Err("TUI find requires 1..256 local steps".into());
        }
        if self.edit_session.is_some()
            || self.choice_overlay.is_some()
            || self.modality_view.is_some()
            || self.help_visible.is_some()
        {
            return Err(
                "Close the current edit/choice/help/modality view before local navigation".into(),
            );
        }
        if self.command_palette.is_some()
            && matches!(
                action,
                "next_region" | "previous_region" | "next_pane" | "previous_pane"
            )
        {
            return Err("Close the current TUI palette before changing scene regions".into());
        }
        let before = self.tui_navigation_state();
        let mut result = json!({"status":"moved","steps":1});
        let reader_command = self.content_view.as_ref().and_then(|view| match action {
            "close_reader" => Some(ContentViewCommand::Close),
            "open_outline" => Some(ContentViewCommand::OpenOutline),
            "choose_content" => match view.mode {
                ContentViewMode::Outline => Some(ContentViewCommand::ChooseOutline),
                ContentViewMode::Search => Some(ContentViewCommand::ChooseSearch),
                _ => None,
            },
            "next_control" | "previous_control" | "scroll_down" | "scroll_up" => {
                let forward = matches!(action, "next_control" | "scroll_down");
                let amount = if action.starts_with("scroll_") { 10 } else { 1 };
                let delta = if forward { amount } else { -amount };
                match view.mode {
                    ContentViewMode::Reader => Some(ContentViewCommand::ScrollReader(delta)),
                    ContentViewMode::Outline => Some(ContentViewCommand::OutlineMove(delta)),
                    ContentViewMode::Search => Some(ContentViewCommand::SearchMove(delta)),
                    _ => None,
                }
            }
            _ => None,
        });
        if let Some(command) = reader_command {
            let ticket = self
                .runtime
                .begin(
                    crate::runtime::OperationKind::SnapshotAcquisition,
                    self.refresh_cancel.clone(),
                )
                .map_err(|e| e.to_string())?;
            self.handle_content_view_command(command).await;
            self.runtime.complete(&ticket).map_err(|e| e.to_string())?;
        } else if action == "open_reader" {
            result = self.tui_open_reader(query, context, exact).await?;
        } else if action == "find_text" {
            let query = query
                .filter(|q| !q.trim().is_empty())
                .ok_or("Content search requires a nonempty query")?;
            let view = self
                .content_view
                .as_mut()
                .ok_or("Open a Reader before searching its text")?;
            view.query = query.to_owned();
            self.handle_content_view_command(ContentViewCommand::OpenSearch)
                .await;
            let count = self
                .content_view
                .as_ref()
                .map(|v| v.results.len())
                .unwrap_or(0);
            result = json!({"status":if count==0{"not_found"}else{"searched"},"steps":1,
                "matches":count,"coverage":"indexed public labels and available text; not future or unexposed content"});
        } else {
            if self.content_view.is_some() {
                if matches!(
                    action,
                    "open_actions"
                        | "find_action"
                        | "open_commands"
                        | "find_command"
                        | "find_control"
                ) {
                    self.handle_content_view_command(ContentViewCommand::Close)
                        .await;
                } else {
                    return Err(
                        "Use Reader browsing tools or close_reader before changing scene regions"
                            .into(),
                    );
                }
            }
            match action {
                "open_commands" | "open_actions" => {
                    if action == "open_actions" {
                        self.command_palette = Some(self.public_action_palette());
                    } else {
                        self.handle_intent(UiIntent::OpenCommandPalette).await;
                    }
                    if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
                        // A supplied public query uses the same finder, never an unfiltered inventory.
                        result = self.tui_find_command(query, context, exact, max_steps);
                    } else if context.is_some() {
                        return Err("Palette context requires a nonempty public query".into());
                    }
                }
                "close_commands" => {
                    self.agent_tui_prepare_execution();
                }
                "next_control" | "previous_control" if self.command_palette.is_some() => {
                    self.command_palette
                        .as_mut()
                        .unwrap()
                        .step_selection(action == "next_control");
                }
                "next_control" => {
                    self.handle_intent(UiIntent::FocusNext).await;
                }
                "previous_control" => {
                    self.handle_intent(UiIntent::FocusPrevious).await;
                }
                "next_region" => {
                    self.handle_intent(UiIntent::RegionNext).await;
                }
                "previous_region" => {
                    self.handle_intent(UiIntent::RegionPrevious).await;
                }
                "next_pane" => {
                    self.handle_intent(UiIntent::SubregionNext).await;
                }
                "previous_pane" => {
                    self.handle_intent(UiIntent::SubregionPrevious).await;
                }
                "scroll_down" => {
                    if let Some(palette) = self.command_palette.as_mut() {
                        palette.scroll_details(4);
                    } else {
                        self.handle_intent(UiIntent::ScrollPages(1)).await;
                    }
                }
                "scroll_up" => {
                    if let Some(palette) = self.command_palette.as_mut() {
                        palette.scroll_details(-4);
                    } else {
                        self.handle_intent(UiIntent::ScrollPages(-1)).await;
                    }
                }
                "find_control" | "find_command" | "find_action" => {
                    let query = query
                        .filter(|q| !q.trim().is_empty())
                        .ok_or("TUI find requires a nonempty query")?;
                    result = if action == "find_command" || action == "find_action" {
                        if action == "find_action" {
                            self.command_palette = Some(self.public_action_palette());
                        } else {
                            self.handle_intent(UiIntent::OpenCommandPalette).await;
                        }
                        self.tui_find_command(query, context, exact, max_steps)
                    } else {
                        self.command_palette = None;
                        self.tui_find_control(query, context, exact, max_steps)
                    };
                }
                _ => {
                    return Err(
                        "Unsupported local TUI navigation action; activation is separate".into(),
                    );
                }
            }
        }
        result["version"] = json!("gui2tui.agent.tui-navigation.experimental.v2");
        result["action"] = json!(action);
        result["query"] = json!(query);
        result["gui_write"] = json!(false);
        result["activation"] = json!("not_attempted");
        let view = self.agent_render_tui_text(observed, width, height)?;
        if result["status"] == "moved" && before == self.tui_navigation_state() {
            result["status"] = json!("no_progress");
        }
        Ok(json!({"navigation":result,"tui_view":view}))
    }

    /// Refresh Reader payload and scope before publishing another TUI observation.
    pub(super) async fn agent_reconcile_reader(&mut self) {
        let legal = |root: RuntimeNodeId| {
            self.scopes.allows_node(root)
                && !document::secret_source(&self.cache, root)
                && self.content.model(root).is_some()
        };
        if self.content_return.as_ref().is_some_and(|v| !legal(v.root)) {
            self.content_return = None;
        }
        if self.content_view.as_ref().is_some_and(|v| !legal(v.root)) {
            self.content_view = None;
            self.status =
                "Reader source disappeared or left the current public scope; Reader closed".into();
            return;
        }
        let Some((root, query, selected)) = self
            .content_view
            .as_ref()
            .map(|v| (v.root, v.query.clone(), v.result_selected))
        else {
            return;
        };
        let order = self.content.model(root).unwrap().reading_order();
        if order.is_empty() {
            self.content_view = None;
            return;
        }
        let results = self.content.search(root, &query);
        if let Some(view) = self.content_view.as_mut() {
            if !order.contains(&view.position) {
                view.position = order[0];
                view.reader_scroll = 0;
            }
            view.outline_selected = view.outline_selected.min(
                self.content
                    .model(root)
                    .unwrap()
                    .navigation
                    .headings
                    .len()
                    .saturating_sub(1),
            );
            view.results = results;
            view.result_selected = selected.min(view.results.len().saturating_sub(1));
            view.reader_blocks.clear();
        }
        self.refresh_content_view().await;
    }

    async fn tui_open_reader(
        &mut self,
        query: Option<&str>,
        context: Option<&str>,
        exact: bool,
    ) -> Result<Value, String> {
        if query.is_none() && context.is_none() && self.content_view.is_some() {
            self.content_view.as_mut().unwrap().mode = ContentViewMode::Reader;
            self.refresh_content_view().await;
            return Ok(json!({"status":"opened","steps":1}));
        }
        let focused = self
            .focus
            .current()
            .and_then(|id| self.scene.element(id))
            .and_then(|e| e.binding.as_ref())
            .map(|b| b.runtime_id);
        let mut found = self
            .content
            .catalog()
            .models()
            .filter_map(|model| {
                let root = model.root;
                if !self.scopes.allows_node(root) || document::secret_source(&self.cache, root) {
                    return None;
                }
                let label = self.document.content_label(root).0;
                if query.is_some_and(|q| !matches_text(&label, q, exact)) {
                    return None;
                }
                if let Some(q) = context {
                    let mut id = Some(root);
                    let mut matched = false;
                    while let Some(current) = id {
                        let Some(node) = self.document.nodes.get(&current) else {
                            break;
                        };
                        matched |= node
                            .name
                            .as_deref()
                            .is_some_and(|name| matches_text(name, q, false));
                        id = node.parent;
                    }
                    if !matched {
                        return None;
                    }
                }
                Some((root, label))
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|(id, _)| *id);
        if query.is_none()
            && let Some(target) = focused
            && found.iter().any(|(id, _)| *id == target)
        {
            found.retain(|(id, _)| *id == target);
        }
        let matches = found
            .iter()
            .map(|(root, label)| json!({"root":root.get(),"label":label}))
            .collect::<Vec<_>>();
        if found.len() != 1 {
            return Ok(
                json!({"status":if found.is_empty(){"unavailable"}else{"ambiguous"},"steps":0,"matches":matches}),
            );
        }
        let root = found[0].0;
        let ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::SnapshotAcquisition,
                self.refresh_cancel.clone(),
            )
            .map_err(|e| e.to_string())?;
        self.command_palette = None;
        let opened = self.begin_reader_root(root, self.focus.current()).await;
        self.runtime.complete(&ticket).map_err(|e| e.to_string())?;
        Ok(
            json!({"status":if opened{"opened"}else{"unavailable"},"steps":1,"root":root.get(),"matches":matches}),
        )
    }

    fn tui_navigation_state(&self) -> Value {
        json!({"focus":self.focus.current().map(SceneElementId::get),
            "region":format!("{:?}",self.active_region),"scroll":self.viewport.offset,
            "reader":self.content_view.as_ref().map(|v|json!({"root":v.root.get(),"position":v.position.get(),
                "mode":format!("{:?}",v.mode),"line_offset":v.reader_scroll,"query":v.query,
                "outline_selected":v.outline_selected,"result_selected":v.result_selected})),
            "palette":self.command_palette.as_ref().map(|p|json!({"query":p.query(),
                "selected":p.selected(),"count":p.entries().len(),
                "detail_scroll":p.detail_scroll(),
                "label":p.entries().get(p.selected()).map(|e|&e.label)}))})
    }

    fn tui_find_command(
        &mut self,
        query: &str,
        context: Option<&str>,
        exact: bool,
        max_steps: u16,
    ) -> Value {
        let cache = &self.cache;
        let scopes = &self.scopes;
        let palette = self.command_palette.as_mut().unwrap();
        palette.set_navigation_query(query.to_owned(), context.map(str::to_owned), exact);
        palette.retain_entries(|entry| {
            matches_current_palette_entry(cache, scopes, entry, query, context, exact)
        });
        let found: Vec<_> = palette
            .entries()
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let (target, locator) = entry.runtime_target()?;
                Some((index, target, locator.clone(), entry.label.clone()))
            })
            .collect();
        let matches: Vec<_> = found
            .iter()
            .map(|(_, target, _, label)| json!({"target":target.get(),"label":label}))
            .collect();
        if found.len() != 1 {
            return json!({"status":if found.is_empty(){"not_found"}else{"ambiguous"},
                "selection_confirmed":false,"steps":0,"matches":matches});
        }
        let (index, target, locator, label) = &found[0];
        if self
            .cache
            .node(*target)
            .is_none_or(|node| node.backend_locator != *locator)
            || !self.scopes.allows_node(*target)
        {
            return json!({"status":"target_changed","selection_confirmed":false,"steps":0});
        }
        let mut steps = 0;
        let mut visited = BTreeSet::new();
        while palette.selected() != *index && steps < max_steps {
            if !visited.insert(palette.selected()) {
                break;
            }
            palette.step_selection(true);
            steps += 1;
        }
        json!({"status":if palette.selected()==*index{"focused"}else if steps==max_steps{"step_limit"}else{"no_progress"},
            "selection_confirmed":palette.selected()==*index,"steps":steps,
            "target":target.get(),"label":label,"matches":matches})
    }

    fn tui_control_context(&self, element: &SceneElement) -> Vec<String> {
        self.layout_analysis
            .as_ref()
            .map(|layout| {
                layout
                    .plan
                    .regions
                    .iter()
                    .filter(|region| {
                        element
                            .sources
                            .iter()
                            .any(|id| region.presentation.source_nodes.contains(id))
                    })
                    .map(|region| region.presentation.title.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn tui_reveal_focus(&mut self) {
        let Some(id) = self.focus.current() else {
            return;
        };
        let Some(element) = self.scene.element(id) else {
            return;
        };
        let region = self.layout_analysis.as_ref().and_then(|layout| {
            layout
                .plan
                .regions
                .iter()
                .find(|region| {
                    element
                        .sources
                        .iter()
                        .any(|id| region.presentation.source_nodes.contains(id))
                })
                .map(|region| region.id)
        });
        if let Some(region) = region {
            self.activate_region(region);
            self.focus.set(&self.scene, id);
        }
        self.ensure_focus_visible();
    }

    fn tui_find_control(
        &mut self,
        query: &str,
        context: Option<&str>,
        exact: bool,
        max_steps: u16,
    ) -> Value {
        let mut seen_targets = BTreeSet::new();
        let found: Vec<_> = self
            .scene
            .elements
            .iter()
            .filter_map(|element| {
                let binding = element.binding.as_ref()?;
                if !element.is_focusable()
                    || document::secret_source(&self.cache, binding.runtime_id)
                    || !matches_text(element.label(), query, exact)
                {
                    return None;
                }
                let regions = self.tui_control_context(element);
                if context.is_some_and(|q| !regions.iter().any(|r| matches_text(r, q, false))) {
                    return None;
                }
                if !seen_targets.insert(binding.runtime_id) {
                    return None;
                }
                Some((
                    element.id,
                    binding.runtime_id,
                    element.label().to_owned(),
                    regions,
                ))
            })
            .collect();
        let matches: Vec<_> = found.iter().map(|(scene,target,label,regions)|
            json!({"scene":scene.get(),"target":target.get(),"label":label,"regions":regions})).collect();
        if found.len() != 1 {
            return json!({"status":if found.is_empty(){"not_found"}else{"ambiguous"},
                "selection_confirmed":false,"steps":0,"matches":matches});
        }
        let (scene, target, label, _) = &found[0];
        let mut steps = 0;
        let mut visited = BTreeSet::new();
        let mut trace = Vec::new();
        while self.focus.current() != Some(*scene) && steps < max_steps {
            if !visited.insert(self.focus.current()) {
                break;
            }
            // Existing Human FocusModel order, not a guessed GUI key sequence.
            self.focus.next(&self.scene);
            self.tui_reveal_focus();
            steps += 1;
            trace.push(self.focus.current().map(SceneElementId::get));
        }
        self.tui_reveal_focus();
        json!({"status":if self.focus.current()==Some(*scene){"focused"}else if steps==max_steps{"step_limit"}else{"no_progress"},
            "selection_confirmed":self.focus.current()==Some(*scene),"steps":steps,
            "target":target.get(),"label":label,"matches":matches,"local_trace":trace})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_public_name_is_not_lost_to_role_and_path_decoration() {
        let label = "Mode selection [ToggleButton] #8 · Workspace › Action: click";
        let title = "[ToggleButton] Mode selection";
        assert!(!matches_text(label, "Mode selection", true));
        assert!(!matches_text(title, "Mode selection", true));
        assert!(matches_palette_text(
            label,
            title,
            Some("Mode selection"),
            "MODE SELECTION",
            true
        ));
        assert!(!matches_palette_text(
            label,
            title,
            Some("Mode selection"),
            "Mode",
            true
        ));
        assert!(matches_palette_text(
            label,
            title,
            None,
            "Action: click",
            false
        ));
        assert!(!matches_palette_text(
            label,
            title,
            None,
            "Mode selection",
            true
        ));
    }

    #[test]
    fn equal_public_names_remain_multiple_matches_and_decorated_context_is_retained() {
        let labels = [
            "Remove [Button] #12 · Report A › Action: click",
            "Remove [Button] #15 · Report B › Action: click",
        ];
        let found: Vec<_> = labels
            .iter()
            .filter(|label| {
                matches_palette_text(label, "[Button] Remove", Some("Remove"), "Remove", true)
            })
            .collect();
        assert_eq!(found.len(), 2); // Existing caller must report ambiguity, not pick by ID/order.
        let contextual: Vec<_> = found
            .into_iter()
            .filter(|label| matches_text(label, "Report B", false))
            .collect();
        assert_eq!(contextual, vec![&labels[1]]);
    }
}
