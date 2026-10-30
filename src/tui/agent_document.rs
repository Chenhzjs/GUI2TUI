//! Full observation adapter. All organization lives in transcompile::document;
//! publication and invocation stay within this TuiApplication's existing owner.
use super::*;
use crate::{
    semantic::RelationState,
    transcompile::document::{self, DocumentInput, MetadataRead, TuiDocument, ValueDomain},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Reuse Human palette wording as non-callable discovery for the exact focus.
fn focused_operation_discovery(
    cache: &SemanticCache,
    scopes: &InteractionScopes,
    document: &TuiDocument,
    binding: &SceneBinding,
) -> Option<Value> {
    let target = binding.runtime_id;
    let node = cache.node(target)?;
    if node.backend_locator != binding.backend_locator
        || document.nodes.get(&target)?.backend_locator != binding.backend_locator
        || !scopes.allows_node(target)
        || document::secret_source(cache, target)
    {
        return None;
    }
    let queries: Vec<_> = public_action_entries_for_document(document)
        .into_iter()
        .filter(|(id, locator, _, operation)| {
            *id == target
                && *locator == binding.backend_locator
                && document::wire::agent_supports(operation)
        })
        .map(|(_, _, label, operation)| {
            let mut hint = document::wire::operation_kind(&operation);
            // This is the existing full Human row label, not a new alias or ID.
            hint["query"] = json!(label);
            hint["exact"] = json!(true);
            hint
        })
        .collect();
    Some(json!({"execution_authority":false,"find_action_queries":queries}))
}

impl TuiApplication {
    /// Experimental Human viewport projection of this owner's current document.
    /// This is deliberately partial presentation, never a complete document.
    pub fn agent_render_tui_text(
        &mut self,
        observed: &AgentObservation,
        width: u16,
        height: u16,
    ) -> Result<Value, String> {
        use ratatui::{Terminal, backend::TestBackend, text::Span};
        use std::collections::BTreeSet;

        if !(20..=240).contains(&width) || !(8..=120).contains(&height) {
            return Err("Experimental TUI view requires 20..240 columns and 8..120 rows".into());
        }
        if observed.data["version"] != document::wire::DOCUMENT_VERSION
            || observed.data["session"] != json!(self.runtime.id)
            || observed.data["generation"] != json!(self.runtime.generation())
            || observed.data["observation_id"] != json!(self.document.observation.observation_id)
        {
            return Err("TUI view requires this owner's current full observation".into());
        }
        let inventory = observed.data["operations"]
            .as_array()
            .ok_or("Missing current operation inventory")?;
        self.project_document_if_needed();
        if self
            .command_palette
            .as_ref()
            .is_some_and(|p| p.is_public_actions())
        {
            let old = self.command_palette.take().unwrap();
            let mut fresh = self.public_action_palette();
            fresh.restore_query_and_exact_selection(&old);
            self.command_palette = Some(fresh);
        } else if let Some(palette) = self.command_palette.as_mut() {
            palette.refresh_commands(self.commands.clone(), self.scopes.active());
        }
        self.refresh_tui_palette_query();
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).map_err(|error| error.to_string())?;
        terminal
            .draw(|frame| self.render(frame))
            .map_err(|error| error.to_string())?;
        let buffer = terminal.backend().buffer();
        let mut lines = Vec::new();
        let mut seen = BTreeSet::new();
        let mut visible = Vec::new();
        for y in 0..height {
            let mut line = String::new();
            let mut x = 0;
            while x < width {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                x = x.saturating_add(Span::raw(symbol).width().max(1) as u16);
            }
            lines.push(line.trim_end().to_owned());
            // Palette overlays cover the base controls. Their old hit regions
            // must not expose hidden controls as if they were still visible.
            if self.command_palette.is_some() || self.content_view.is_some() {
                continue;
            }
            for x in 0..width {
                let Some(hit) = self.hit_map.hit(x, y) else {
                    continue;
                };
                if !seen.insert(hit.scene_id) {
                    continue;
                }
                let Some(element) = self.scene.element(hit.scene_id) else {
                    continue;
                };
                let Some(binding) = &element.binding else {
                    continue;
                };
                if document::secret_source(&self.cache, binding.runtime_id) {
                    continue;
                }
                // References come from the original current operation inventory;
                // a terminal rectangle never manufactures an operation.
                let operations: Vec<_> = inventory
                    .iter()
                    .filter(|op| {
                        op["target"] == json!(binding.runtime_id.get())
                            && op["agent_invocable"] == true
                    })
                    .cloned()
                    .collect();
                visible.push(json!({"scene":hit.scene_id.get(),
                    "target":binding.runtime_id.get(),"label":element.label(),
                    "operations":operations}));
            }
        }
        let palette_state = self.command_palette.as_ref().map(|palette| {
            let selected = palette.entries().get(palette.selected());
            let areas =
                crate::tui::renderer::palette_areas(crate::tui::renderer::main_content_area(
                    ratatui::layout::Rect::new(0, 0, width, height),
                ));
            let range = crate::tui::renderer::palette_result_range(
                areas[3],
                palette.entries().len(),
                palette.selected(),
            );
            for (index, entry) in palette
                .entries()
                .iter()
                .enumerate()
                .skip(range.start)
                .take(range.len())
            {
                if let Some((target, locator)) = entry.runtime_target()
                    && self
                        .cache
                        .node(target)
                        .is_some_and(|node| node.backend_locator == *locator)
                    && !document::secret_source(&self.cache, target)
                {
                    let operations: Vec<_> = inventory
                        .iter()
                        .filter(|op| {
                            op["target"] == json!(target.get()) && op["agent_invocable"] == true
                        })
                        .cloned()
                        .collect();
                    visible.push(
                        json!({"palette_selected":index==palette.selected(),"palette_index":index,
                        "target":target.get(),"label":entry.label,"operations":operations}),
                    );
                }
            }
            json!({"query":palette.query(),"selected_index":palette.selected(),
                "entries":palette.entries().len(),"visible_indices":range,
                "selected_label":selected.map(|entry| &entry.label)})
        });
        let focused = self
            .command_palette
            .is_none()
            .then(|| {
                self.content_view
                    .is_none()
                    .then(|| self.focus.current())
                    .flatten()
            })
            .flatten()
            .and_then(|id| self.scene.element(id))
            .and_then(|element| {
                let binding = element.binding.as_ref()?;
                let discovery = focused_operation_discovery(
                    &self.cache,
                    &self.scopes,
                    &self.document,
                    binding,
                )?;
                Some(json!({"scene":element.id.get(),"label":element.label(),
                "target":binding.runtime_id.get(),"operation_discovery":discovery}))
            });
        let reader = self.content_view.as_ref().map(|v| {
            json!({"root":v.root.get(),
            "mode":format!("{:?}",v.mode),"position":v.position.get(),"line_offset":v.reader_scroll,
            "query":v.query,"matches":v.results.len(),"selected_result":v.result_selected,
            "headings":self.content.model(v.root).map(|m|m.navigation.headings.len()),
            "coverage":"public indexed content; current Human viewport only"})
        });
        let mut actions = vec![
            "next_control",
            "previous_control",
            "scroll_down",
            "scroll_up",
            "open_commands",
            "find_command",
            "open_actions",
            "find_action",
            "find_control",
            "open_reader",
        ];
        if let Some(view) = &self.content_view {
            actions.extend(["close_reader", "open_outline", "find_text"]);
            if (view.mode == ContentViewMode::Search && !view.results.is_empty())
                || (view.mode == ContentViewMode::Outline
                    && self
                        .content
                        .model(view.root)
                        .is_some_and(|m| !m.navigation.headings.is_empty()))
            {
                actions.push("choose_content");
            }
        } else {
            actions.extend([
                "next_region",
                "previous_region",
                "next_pane",
                "previous_pane",
                "close_commands",
            ]);
        }
        Ok(json!({"version":"gui2tui.agent.tui-text.experimental.v4",
            "observation_id":observed.data["observation_id"],
            "session":observed.data["session"],"generation":observed.data["generation"],
            "interaction_scope":observed.data["interaction_scope"],
            "coverage":"human_viewport_only; offscreen, folded and other content may be absent",
            "complete_document":false,"columns":width,"rows":height,
            "text":lines.join("\n"),"visible_bindings":visible,
            "focused":focused,"palette":palette_state,"reader":reader,
            "navigation_tools":{"command":"tui_navigate",
                "actions":actions,
                "reader":"open_reader(query/context optional) selects current public content; find_text searches locally; choose_content follows the selected outline/search result; all read-only",
                "find":"query/context are literal label/title/path text, not task explanations; exact, max_steps (1..256); focuses only; never activates"},
            "execution_authority":"existing G2T current observation and operation registry only"}))
    }

    pub async fn agent_observe_document(
        &mut self,
        observation_id: u64,
    ) -> Result<AgentObservation, String> {
        let cancel = self.refresh_cancel.clone();
        let ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::SnapshotAcquisition,
                cancel.clone(),
            )
            .map_err(|e| e.to_string())?;
        let result = self
            .collect_complete_document(observation_id, &cancel)
            .await;
        // Complete before publishing cache, scope, document or an executable ID.
        self.runtime.complete(&ticket).map_err(|e| e.to_string())?;
        let (cache, scopes, document, mut timings) = result?;
        self.cache = cache;
        self.scopes = scopes;
        self.document = document;
        self.content.adopt_document(
            &self.cache,
            &self.document.content,
            &self.document.tables,
            &self.document.virtual_collections,
        );
        let reader_started = Instant::now();
        if self.content_view.is_some() || self.content_return.is_some() {
            let ticket = self
                .runtime
                .begin(
                    crate::runtime::OperationKind::SnapshotAcquisition,
                    cancel.clone(),
                )
                .map_err(|e| e.to_string())?;
            self.agent_reconcile_reader().await;
            self.runtime.complete(&ticket).map_err(|e| e.to_string())?;
        }
        timings["tui_reader_refresh_ms"] = json!(reader_started.elapsed().as_secs_f64() * 1000.0);
        let export_started = Instant::now();
        let refs = self
            .document
            .nodes
            .values()
            .map(|node| {
                self.runtime
                    .source_locator_ref(&node.backend_locator)
                    .map(|reference| (node.runtime_id, reference))
            })
            .collect::<Option<BTreeMap<_, _>>>()
            .ok_or("Current public source identities are unavailable")?;
        let mut data = document::wire::to_agent_document_with_source_refs(&self.document, &refs)
            .map_err(str::to_owned)?;
        timings["agent_value_export_ms"] = json!(export_started.elapsed().as_secs_f64() * 1000.0);
        data["timings"] = timings;
        self.document_agent_observation(data)
    }

    async fn collect_complete_document(
        &self,
        observation_id: u64,
        cancel: &crate::modality::CancellationToken,
    ) -> Result<(SemanticCache, InteractionScopes, TuiDocument, Value), String> {
        let started = Instant::now();
        let mut identity = document_identity(&self.runtime, observation_id);
        let bootstrap = load_snapshot(
            &self.backend,
            &self.application_locator,
            self.inspect_options,
            BootstrapStrategy::Walk,
        )
        .await
        .map_err(|e| e.to_string())?;
        if !self
            .runtime
            .validates_application(&bootstrap.root.backend_locator)
        {
            return Err("Application generation changed".into());
        }
        let discovery_ms = started.elapsed().as_secs_f64() * 1000.0;
        let arena_started = Instant::now();
        let mut cache = self.cache.clone();
        cache
            .full_refresh(bootstrap.root)
            .map_err(|e| e.to_string())?;
        let arena_ms = arena_started.elapsed().as_secs_f64() * 1000.0;
        let qualification_started = Instant::now();
        qualify_complete_text_capabilities(&self.backend, &mut cache).await;
        let qualification_ms = qualification_started.elapsed().as_secs_f64() * 1000.0;
        let relations_started = Instant::now();
        let mut relation_rpcs = 0;
        let mut unavailable = 0;
        // Scope boundary relations establish popup ownership before materializing
        // content. Each newly legal source is then queried, without a set cap.
        let scopes = loop {
            if cancel.is_cancelled() {
                return Err("Document observation cancelled".into());
            }
            let scopes = InteractionScopes::analyze(
                &cache,
                &crate::semantic::RelationalSemanticGraph::new(&cache),
            );
            let mut pending: Vec<_> = cache
                .nodes()
                .filter(|n| {
                    !document::secret_source(&cache, n.runtime_id)
                        && (scopes.allows_node(n.runtime_id)
                            || scopes
                                .scope(crate::transcompile::InteractionScopeId(n.runtime_id))
                                .is_some())
                        && matches!(
                            cache.relation_state(n.runtime_id),
                            Some(RelationState::Unknown)
                        )
                })
                .map(|n| n.runtime_id)
                .collect();
            pending.sort();
            if pending.is_empty() {
                break scopes;
            }
            // Bound existing relation workers, while draining every source.
            for chunk in pending.chunks(32) {
                if cancel.is_cancelled() {
                    return Err("Document observation cancelled".into());
                }
                let metrics = self.backend.enrich_relations(&mut cache, chunk).await;
                relation_rpcs += metrics.rpc_count;
                unavailable += metrics.unavailable_nodes;
                // A failed worker must not leave an Unknown node in a retry loop.
                for id in chunk {
                    if matches!(cache.relation_state(*id), Some(RelationState::Unknown)) {
                        let _ = cache.mark_relations_unavailable(*id);
                    }
                }
            }
        };
        let relations_ms = relations_started.elapsed().as_secs_f64() * 1000.0;
        let selection_started = Instant::now();
        enrich_choice_selections(&self.backend, &mut cache, Some(cancel)).await;
        if cancel.is_cancelled() {
            return Err("Document observation cancelled".into());
        }
        let selection_ms = selection_started.elapsed().as_secs_f64() * 1000.0;
        let (reads, text_metrics) =
            crate::content::materialize::read_public_texts(&self.backend, &cache, &scopes, cancel)
                .await?;
        let values_started = Instant::now();
        let mut values = BTreeMap::new();
        let mut value_sources: Vec<_> = cache
            .nodes()
            .filter(|n| {
                scopes.allows_node(n.runtime_id)
                    && !document::secret_source(&cache, n.runtime_id)
                    && n.debug.interfaces.iter().any(|i| i == "Value")
            })
            .map(|n| (n.runtime_id, n.backend_locator.clone()))
            .collect();
        value_sources.sort_by_key(|(id, _)| *id);
        for (id, locator) in value_sources {
            if cancel.is_cancelled() {
                return Err("Document observation cancelled".into());
            }
            let domain = match self.backend.read_public_value_domain(&locator).await {
                Ok((current, min, max, step))
                    if [current, min, max, step].iter().all(|v| v.is_finite()) =>
                {
                    ValueDomain::Known {
                        current,
                        min,
                        max,
                        step,
                    }
                }
                _ => ValueDomain::Unavailable,
            };
            values.insert(id, domain);
        }
        let values_ms = values_started.elapsed().as_secs_f64() * 1000.0;
        let metadata_started = Instant::now();
        let mut metadata = BTreeMap::new();
        let roots = crate::content::ContentCatalog::document_roots(&cache);
        for id in &roots {
            let node = cache.node(*id).expect("source root");
            if !scopes.allows_node(*id)
                || document::secret_source(&cache, *id)
                || !node.debug.interfaces.iter().any(|i| i == "Document")
            {
                continue;
            }
            if cancel.is_cancelled() {
                return Err("Document observation cancelled".into());
            }
            let read = match self.backend.probe_document(&node.backend_locator).await {
                Ok(probe) => MetadataRead::Known(probe),
                Err(BackendError::SecretContentDisabled(_)) => {
                    return Err("Document became sensitive; observe again".into());
                }
                Err(_) => MetadataRead::Unavailable,
            };
            metadata.insert(*id, read);
        }
        let metadata_ms = metadata_started.elapsed().as_secs_f64() * 1000.0;

        if cancel.is_cancelled() {
            return Err("Document observation cancelled".into());
        }
        let table_started = Instant::now();
        let table_positions = collect_table_positions(&self.backend, &cache, cancel)
            .await
            .map_err(|e| e.to_string())?;
        let table_positions_ms = table_started.elapsed().as_secs_f64() * 1000.0;
        let public_apps = self
            .backend
            .applications()
            .await
            .map_err(|_| "Application owner check failed".to_owned())?;
        if !public_apps
            .iter()
            .any(|app| app.backend_locator == self.application_locator)
        {
            return Err("Application owner disappeared".into());
        }
        identity.finished_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let organize_started = Instant::now();
        let operations = document::public_operations(&cache, &scopes, &values);
        let roots = crate::content::ContentCatalog::document_roots(&cache);
        let mut document = document::build_document(&DocumentInput {
            observation: identity,
            cache: &cache,
            scopes: &scopes,
            redacted: &Default::default(),
            text_reads: &reads,
            value_domains: &values,
            metadata_reads: &metadata,
            table_positions: &table_positions,
            content_roots: &roots,
            operations: &operations,
        })
        .map_err(|e| e.to_string())?;
        let annotation_started = Instant::now();
        document::annotations::apply(&mut document, self.decision_annotation_mode);
        let annotation_ms = annotation_started.elapsed().as_secs_f64() * 1000.0;
        let organization_ms = organize_started.elapsed().as_secs_f64() * 1000.0;
        let timings = json!({"ax_walk_ms":discovery_ms,"ax_walk_read_groups":bootstrap.metrics.walk,"arena_ms":arena_ms,"capability_qualification_ms":qualification_ms,
            "selection_ms":selection_ms,"relations_ms":relations_ms,"relation_requests":relation_rpcs,"relations_unavailable":unavailable,
            "full_text":text_metrics,"value_domains_ms":values_ms,"document_metadata_ms":metadata_ms,"value_sources":values.len(),
            "table_positions_ms":table_positions_ms,"table_position_sources":table_positions.len(),"shared_organization_ms":organization_ms,"human_scene_layout_ms":0,"human_scene_layout_performed":false,
            "collection_and_organization_ms":started.elapsed().as_secs_f64()*1000.0,
            "decision_annotations_ms":annotation_ms,"provider_model_decision_ms":null});
        Ok((cache, scopes, document, timings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{DebugInfo, RelationalSemanticGraph, SemanticNode, TextInputKind};
    use std::collections::HashSet;

    fn fixture(modal: bool) -> (SemanticCache, InteractionScopes, TuiDocument, SceneBinding) {
        let node = |path: &str, role, kind| SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.70", path),
            index_in_parent: None,
            role,
            name: None,
            description: None,
            value: None,
            text_input_kind: kind,
            states: vec![SemanticState::Enabled],
            actions: Vec::new(),
            capabilities: vec![SemanticCapability::EditComplexText],
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        };
        let mut root = node("/window", SemanticRole::Window, None);
        root.capabilities.clear();
        let public = node("/body", SemanticRole::TextInput, Some(TextInputKind::Plain));
        let mut password = node(
            "/password",
            SemanticRole::TextInput,
            Some(TextInputKind::Password),
        );
        let mut child = node("/password/child", SemanticRole::TextInput, None);
        child.name = Some("secret-marker".into());
        password.children.push(child);
        root.children = vec![public, password];
        if modal {
            let mut dialog = node("/dialog", SemanticRole::Dialog, None);
            dialog.states.extend([
                SemanticState::Other("modal".into()),
                SemanticState::Other("showing".into()),
                SemanticState::Other("visible".into()),
            ]);
            root.children.push(dialog);
        }
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        let target = cache
            .runtime_id(&BackendLocator::new(":1.70", "/body"))
            .unwrap();
        let operations = document::public_operations(&cache, &scopes, &BTreeMap::new());
        let document = document::build_document(&DocumentInput {
            observation: document::DocumentObservation {
                session: crate::runtime::RuntimeSessionId::default(),
                generation: crate::runtime::ApplicationGenerationId(1),
                observation_id: 1,
                started_at_ms: 1,
                finished_at_ms: 1,
            },
            cache: &cache,
            scopes: &scopes,
            redacted: &HashSet::new(),
            text_reads: &BTreeMap::new(),
            value_domains: &BTreeMap::new(),
            metadata_reads: &BTreeMap::new(),
            table_positions: &BTreeMap::new(),
            content_roots: &[],
            operations: &operations,
        })
        .unwrap();
        let binding = SceneBinding {
            runtime_id: target,
            backend_locator: cache.node(target).unwrap().backend_locator.clone(),
            semantic_role: SemanticRole::TextInput,
            actions: Vec::new(),
            capability: InteractionCapability::BrowseContent,
            default_intent: UiIntent::BeginRead,
        };
        (cache, scopes, document, binding)
    }

    #[test]
    fn focus_discovery_reuses_human_operations_without_callable_ids() {
        let (cache, scopes, doc, binding) = fixture(false);
        let hint = focused_operation_discovery(&cache, &scopes, &doc, &binding).unwrap();
        assert_eq!(hint["execution_authority"], false);
        let queries = hint["find_action_queries"].as_array().unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0]["kind"], "replace_text");
        assert_eq!(queries[0]["multiline"], true);
        assert_eq!(queries[0]["parameter"], "text");
        assert_eq!(queries[0]["exact"], true);
        let expected = public_action_entries_for_document(&doc)
            .into_iter()
            .find(|entry| {
                entry.0 == binding.runtime_id
                    && matches!(
                        entry.3,
                        crate::tui::palette::PublicOperation::EditText { .. }
                    )
            })
            .unwrap()
            .2;
        assert_eq!(queries[0]["query"], expected);
        assert!(!hint.to_string().contains("op-"));
        assert!(!hint.to_string().contains("secret-marker"));
    }

    #[test]
    fn focus_discovery_rejects_stale_missing_secret_and_modal_targets() {
        let (cache, scopes, doc, binding) = fixture(false);
        let mut invalid = binding.clone();
        invalid.backend_locator = BackendLocator::new(":1.70", "/different");
        assert!(focused_operation_discovery(&cache, &scopes, &doc, &invalid).is_none());
        invalid.runtime_id = RuntimeNodeId::new(u64::MAX);
        assert!(focused_operation_discovery(&cache, &scopes, &doc, &invalid).is_none());
        for path in ["/password", "/password/child"] {
            invalid.runtime_id = cache
                .runtime_id(&BackendLocator::new(":1.70", path))
                .unwrap();
            invalid.backend_locator = cache
                .node(invalid.runtime_id)
                .unwrap()
                .backend_locator
                .clone();
            assert!(focused_operation_discovery(&cache, &scopes, &doc, &invalid).is_none());
        }
        let (cache, scopes, doc, binding) = fixture(true);
        assert!(!scopes.allows_node(binding.runtime_id));
        assert!(focused_operation_discovery(&cache, &scopes, &doc, &binding).is_none());
    }
}
