use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::{
    backend::{
        AtspiBackend, BackendError, BootstrapStrategy, EventDelivery, EventSubscription,
        InspectOptions,
    },
    content::{
        ContentCacheBudget, ContentCompleteness, ContentRuntime, MaterializationBudget,
        SearchBudget, SearchState, TextCapabilityStatus,
    },
    events::{DirtyScope, NormalizedEvent, coalesce_dirty_scopes},
    semantic::{
        BackendLocator, LARGE_TREE_RELATION_CANDIDATE_LIMIT, RelationPriorityContext,
        RuntimeNodeId, SemanticAction, SemanticCache, SemanticCapability, SemanticRole,
        SemanticState, schedule_on_demand_relations, schedule_relation_candidates,
    },
    transcompile::{
        ChoiceCatalog, ChoiceOptions, CommandHierarchy, InteractionScopeId, InteractionScopes,
        LayoutAnalysis, PresentationMode, RegionPresentationContext, RegionPresentationKind,
        SceneBinding, SceneElement, SceneElementId, SceneElementKind, SpatialEvidenceIndex,
        SpatialProbeBudget, SpatialRegion, SpatialRegionId, TuiScene, analyze_regions,
        analyze_regions_with_graph, compile_legacy_scene, compile_scene, compress_content_scene,
        infer_layout_with_presentations, refine_layout_demands_from_scene, region_focus_order,
    },
};

use super::{
    action::{InteractionCapability, UiIntent},
    choice_overlay::{ChoiceOverlay, ChoiceOverlayOutcome},
    content_view::{ContentViewCommand, ContentViewMode, ContentViewState, move_index},
    edit::{EditCommand, EditSession, key_to_edit_command},
    external_text::{ExternalTextSession, HandlerOutcome},
    focus::{FocusModel, Viewport},
    hit_test::{HitInteraction, HitMap},
    input::{MouseIntent, key_to_intent},
    operation::{
        BackendOperation, SemanticOperation, resolve_backend_operation,
        resolve_cached_node_operation, resolve_choice_backend_operation,
    },
    palette::{CommandPalette, PaletteOutcome},
    region_navigation::RegionNavigator,
    renderer::{
        ChoiceRender, ContentRender, InlineContentRender, PaletteRender, RenderContext, render,
    },
    transition::{
        ConditionRefresh, OperationAuthority, TransitionCondition, TransitionEvaluation,
        TransitionObservation, TransitionOutcome, TransitionReport,
    },
};

pub struct TuiApplication {
    capture_task:
        Option<tokio::task::JoinHandle<Result<CaptureCompletion, crate::runtime::RuntimeError>>>,
    capture_ticket: Option<crate::runtime::OperationTicket>,
    runtime: crate::runtime::RuntimeSession,
    modality_ticket: Option<crate::runtime::OperationTicket>,
    runtime_status_visible: bool,
    help_visible: Option<super::help::HelpContext>,
    help_scroll: u16,
    modality_socket: Option<std::path::PathBuf>,
    modality_view: Option<super::modality_view::ModalityView>,
    modality_task: Option<tokio::task::JoinHandle<String>>,
    modality_cancel: crate::modality::CancellationToken,
    materialized_artifacts: Vec<crate::modality::materialize::MaterializedArtifact>,
    backend: AtspiBackend,
    app_selector: String,
    application_locator: crate::semantic::BackendLocator,
    inspect_options: InspectOptions,
    bootstrap_strategy: BootstrapStrategy,
    settle_delay: Duration,
    cache: SemanticCache,
    event_subscription: EventSubscription,
    event_stream_available: bool,
    presentation_mode: PresentationMode,
    spatial_layout: bool,
    layout_analysis: Option<LayoutAnalysis>,
    spatial_evidence: Option<SpatialEvidenceIndex>,
    active_region: Option<SpatialRegionId>,
    inline_materialized_extent: usize,
    scene: TuiScene,
    scopes: InteractionScopes,
    commands: CommandHierarchy,
    choices: ChoiceCatalog,
    recent_commands: HashMap<RuntimeNodeId, u32>,
    scope_focus_history: HashMap<InteractionScopeId, FocusAnchor>,
    focus: FocusModel,
    viewport: Viewport,
    viewport_height: u16,
    viewport_width: u16,
    hit_map: HitMap,
    status: String,
    application_available: bool,
    backend_available: bool,
    external_text_handler_available: bool,
    edit_session: Option<EditSession>,
    external_text_requested: bool,
    command_palette: Option<CommandPalette>,
    choice_overlay: Option<ChoiceOverlay>,
    content: ContentRuntime,
    content_view: Option<ContentViewState>,
    content_return: Option<ContentViewState>,
    reader_stale_fallbacks: u64,
}

#[derive(Clone, Debug)]
struct FocusAnchor {
    scene_id: SceneElementId,
    runtime_id: RuntimeNodeId,
    locator: BackendLocator,
}

struct CaptureCompletion {
    candidate: crate::modality::ModalityCandidate,
    modality: crate::modality::ExternalModality,
    artifact: crate::modality::materialize::MaterializedArtifact,
}

struct ActionObservationResult {
    report: TransitionReport,
    invocation_accepted: bool,
    rejection: Option<BackendError>,
}

impl TuiApplication {
    pub fn is_available(&self) -> bool {
        self.application_available && self.backend_available
    }
    fn desired_inline_materialization_extent(&self) -> usize {
        inline_materialization_budget(self.viewport_height, self.viewport.offset).visible_blocks
    }

    async fn refresh_inline_materialization(&mut self) {
        if !self.spatial_layout {
            return;
        }
        let budget = inline_materialization_budget(self.viewport_height, self.viewport.offset);
        if budget.visible_blocks <= self.inline_materialized_extent {
            return;
        }
        let roots = self
            .content
            .catalog()
            .models()
            .filter(|model| model.scope_class == crate::content::ContentScopeClass::Primary)
            .filter_map(|model| {
                model
                    .reading_order()
                    .first()
                    .copied()
                    .map(|position| (model.root, position))
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for (root, position) in roots {
            changed |= self
                .content
                .materialize_viewport(&self.backend, &self.cache, root, position, budget)
                .await
                .is_ok();
        }
        self.inline_materialized_extent = budget.visible_blocks;
        if changed {
            self.replan_spatial_layout();
        }
    }
    pub fn has_pending_work(&self) -> bool {
        self.capture_task.is_some()
            || self.modality_task.is_some()
            || (self.spatial_layout
                && self.desired_inline_materialization_extent() > self.inline_materialized_extent)
            || self
                .content_view
                .as_ref()
                .and_then(|v| v.full_search.as_ref())
                .is_some_and(|s| s.state == SearchState::Running)
    }
    pub fn runtime_status(&self) -> serde_json::Value {
        let mut status = self.runtime.status();
        status["event_queue_depth"] = self.event_subscription.queue_depth().into();
        status["event_queue_capacity"] = self.event_subscription.capacity().into();
        status["events"] = self.event_subscription.statistics();
        status["temporary_artifacts"] = self.materialized_artifacts.len().into();
        status["cache_nodes"] = self.cache.node_count().into();
        status["full_snapshots"] = self.cache.full_snapshot_count().into();
        status["focused_scene"] = self.focus.current().map(SceneElementId::get).into();
        status["focused_runtime"] = self
            .focus
            .current()
            .and_then(|id| self.scene.element(id))
            .and_then(|el| el.binding.as_ref())
            .map(|b| b.runtime_id.get())
            .into();
        status["reader"] = self
            .content_view
            .as_ref()
            .map(|view| {
                serde_json::json!({
                    "root_runtime": view.root.get(),
                    "position": view.position.get(),
                    "mode": format!("{:?}", view.mode),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        status["reader_stale_fallbacks"] = self.reader_stale_fallbacks.into();
        let text_capabilities = self.content.text_capability_counts();
        status["text_capabilities"] = serde_json::json!({
            "unsupported": text_capabilities[0],
            "declared": text_capabilities[1],
            "verified": text_capabilities[2],
            "quarantined": text_capabilities[3],
        });
        status
    }

    /// Surface a recoverable product-shell error without mutating semantic
    /// cache state. Used when an explicitly registered launcher fails.
    pub fn set_shell_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    pub fn take_external_text_request(&mut self) -> bool {
        std::mem::take(&mut self.external_text_requested)
    }

    pub fn set_terminal_attached(&mut self, attached: bool) {
        self.runtime.set_terminal_attached(attached);
        // Preserve semantic focus/Reader offsets; the next draw recomputes
        // rectangles for the current terminal dimensions, not old screen rows.
        self.hit_map = HitMap::default();
        tracing::debug!(status = %self.runtime_status(), "terminal attachment changed");
    }

    pub fn begin_terminal_reattach(&mut self) {
        self.runtime.begin_terminal_reattach();
    }

    fn application_gone(&mut self) {
        self.application_available = false;
        self.event_subscription.close();
        self.runtime.invalidate_application();
        self.modality_cancel.cancel();
        if let Some(task) = self.capture_task.take() {
            task.abort();
        }
        self.capture_ticket = None;
        self.modality_ticket = None;
        if let Some(task) = self.modality_task.take() {
            task.abort();
        }
        self.edit_session = None;
        self.external_text_requested = false;
        self.content_view = None;
        self.content_return = None;
        self.modality_view = None;
        self.choice_overlay = None;
        self.command_palette = None;
        self.materialized_artifacts.clear();
        self.hit_map = HitMap::default();
        self.status = "Application is no longer available. Tasks discarded. F5: search again; b: applications; d: diagnostics; q: quit.".into();
        tracing::debug!(status = %self.runtime_status(), "application generation invalidated");
    }

    /// Explicit user selection of the same name. Never reconciles old/new
    /// caches, content, quarantine, scopes or command/choice bindings.
    async fn open_fresh_generation(&mut self) {
        // Application restart invalidates object locators, not a healthy
        // accessibility-bus connection. Reuse it; otherwise every generation
        // leaves another zbus executor thread/fd until the process runtime
        // shuts down. A real bus loss follows the separate reconnect path.
        let backend = self.backend.clone();
        match Self::new(
            backend,
            self.app_selector.clone(),
            self.inspect_options,
            self.settle_delay,
            self.bootstrap_strategy,
            self.event_subscription.capacity(),
            self.presentation_mode,
            self.spatial_layout,
            (
                self.viewport_width.saturating_add(2),
                self.viewport_height.saturating_add(3),
            ),
            self.external_text_handler_available,
        )
        .await
        {
            Ok(mut fresh) => {
                fresh.configure_modality_client(self.modality_socket.clone());
                let mut runtime = std::mem::take(&mut self.runtime);
                runtime.open_application(fresh.application_locator.clone());
                fresh.runtime = runtime;
                *self = fresh;
                tracing::debug!(status = %self.runtime_status(), "opened fresh application generation");
            }
            Err(_) => {
                self.status =
                    "Application is not available yet. F5 retries explicitly; q quits.".into()
            }
        }
    }

    pub async fn select_fresh_application(&mut self, name: String) {
        self.application_gone();
        self.app_selector = name;
        if self.backend_available {
            self.open_fresh_generation().await;
        } else {
            self.reconnect_backend().await;
        }
    }

    pub fn configure_modality_client(&mut self, socket: Option<std::path::PathBuf>) {
        self.runtime.endpoint_profile = socket
            .as_ref()
            .map(|p| crate::runtime::EndpointProfileId(p.to_string_lossy().into_owned()));
        self.modality_socket = socket;
    }

    async fn begin_modality(&mut self) {
        let candidates = crate::modality::ModalityResolver::discover(&self.cache)
            .into_iter()
            .filter(|candidate| self.scopes.allows_node(candidate.owner))
            .filter(|candidate| {
                self.content
                    .catalog()
                    .owning_root(candidate.owner)
                    .and_then(|root| self.content.catalog().get(root))
                    .is_none_or(|model| {
                        model.scope_class != crate::content::ContentScopeClass::BackgroundSecondary
                    })
            })
            .collect();
        if self.modality_socket.is_some() {
            self.runtime
                .set_endpoint(crate::runtime::EndpointState::Connecting);
        }
        let capabilities = if let Some(socket) = self.modality_socket.clone() {
            tokio::task::spawn_blocking(move || crate::modality::wire::capabilities(&socket))
                .await
                .ok()
                .and_then(Result::ok)
        } else {
            None
        };
        self.runtime.set_endpoint(if capabilities.is_some() {
            crate::runtime::EndpointState::Available
        } else if self.modality_socket.is_some() {
            crate::runtime::EndpointState::Disconnected
        } else {
            crate::runtime::EndpointState::Unavailable
        });
        self.modality_view = Some(super::modality_view::ModalityView {
            candidates,
            selected: 0,
            resolved: None,
            capabilities,
        });
        self.resolve_selected_modality().await;
    }

    async fn resolve_selected_modality(&mut self) {
        let Some(view) = &self.modality_view else {
            return;
        };
        let Some(candidate) = view.candidates.get(view.selected).cloned() else {
            return;
        };
        let mut modality = crate::modality::runtime::resolve_atspi(&self.backend, &candidate).await;
        let Some(view) = &mut self.modality_view else {
            return;
        };
        modality.negotiate(view.capabilities.as_ref());
        view.resolved = Some(modality);
        self.status = "Resolved from accessibility metadata only; no GUI action invoked".to_owned();
    }

    fn handoff_selected_modality(&mut self) {
        if self.modality_task.is_some() {
            self.status = "A local handoff is already pending approval".to_owned();
            return;
        }
        let Some(view) = &self.modality_view else {
            return;
        };
        let Some(modality) = view.resolved.clone() else {
            return;
        };
        let valid_target = view.candidates.get(view.selected).is_some_and(|candidate| {
            self.cache
                .node(candidate.owner)
                .is_some_and(|node| node.backend_locator == candidate.locator)
                && self.scopes.allows_node(candidate.owner)
        });
        if !valid_target {
            self.status =
                "Resource control was replaced or left active scope; reopen F4".to_owned();
            return;
        }
        if !modality.capabilities.reference_handoff {
            self.status = match &modality.resolution {
                crate::modality::ModalityResource::ReferencedResource(resource) => format!(
                    "Headless reference: {:?}; payload_bytes=0",
                    crate::modality::redact_reference(&resource.reference)
                ),
                _ => "No reference Open available; m explicitly materializes an unresolved Image"
                    .into(),
            };
            return;
        }
        let Some(socket) = self.modality_socket.clone() else {
            return;
        };
        let crate::modality::ModalityResolution::ReferencedResource(resource) = modality.resolution
        else {
            return;
        };
        self.modality_cancel = Default::default();
        let cancel = self.modality_cancel.clone();
        self.modality_ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::ReferenceHandoff,
                cancel.clone(),
            )
            .ok();
        if self.modality_ticket.is_none() {
            return;
        }
        self.modality_task = Some(tokio::task::spawn_blocking(move || {
            match crate::modality::wire::send_reference_cancellable(
                &socket,
                modality.kind,
                resource,
                &cancel,
            ) {
                Ok(crate::modality::wire::Response::Opened { artifact_bytes, .. }) => format!(
                    "Local handler accepted resource; reference-only; artifact_bytes={artifact_bytes}"
                ),
                Ok(crate::modality::wire::Response::Failed { reason, .. }) => {
                    crate::modality::wire::user_failure_message(&reason).to_owned()
                }
                Ok(_) => "Unexpected local viewer response; no Open confirmed".to_owned(),
                Err(_) => "Local modality client unavailable; GUI unchanged".to_owned(),
            }
        }));
        self.status = "Awaiting user authorization in local broker; TUI remains usable".to_owned();
    }

    async fn materialize_selected_modality(&mut self) {
        use crate::modality::{acquisition::ModalityMetrics, materialize::ArtifactMaterializer};
        if self.capture_task.is_some() || self.modality_task.is_some() {
            self.status = "A modality operation is already pending".into();
            return;
        }
        let Some(candidate) = self
            .modality_view
            .as_ref()
            .and_then(|v| v.candidates.get(v.selected))
            .cloned()
        else {
            return;
        };
        if !self.scopes.allows_node(candidate.owner)
            || !self
                .cache
                .node(candidate.owner)
                .is_some_and(|n| n.backend_locator == candidate.locator)
        {
            self.status = "Visual control changed; reopen F4".into();
            return;
        }
        self.materialized_artifacts.retain(|a| !a.expired());
        if self.materialized_artifacts.len() >= 8 {
            self.status = "Session artifact limit reached; wait for expiry or restart TUI".into();
            return;
        }
        // Resolve again to prefer any newly available reference over capture.
        let mut modality = crate::modality::runtime::resolve_atspi(&self.backend, &candidate).await;
        if let crate::modality::ModalityResource::ReferencedResource(resource) =
            &modality.resolution
        {
            self.status = format!(
                "Reference {:?}; payload_bytes=0; no capture",
                crate::modality::redact_reference(&resource.reference)
            );
            return;
        }
        self.modality_cancel = Default::default();
        let cancel = self.modality_cancel.clone();
        self.capture_ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::SnapshotAcquisition,
                cancel.clone(),
            )
            .ok();
        if self.capture_ticket.is_none() {
            return;
        }
        let artifact_owner = self
            .capture_ticket
            .as_ref()
            .map(|ticket| (ticket.session_id(), ticket.operation_id()));
        let backend = self.backend.clone();
        self.capture_task = Some(tokio::spawn(async move {
            let mut metrics = ModalityMetrics::default();
            let (snapshot, bytes) = crate::modality::runtime::acquire_snapshot(
                &backend,
                &candidate,
                &modality.resolution,
                std::sync::Arc::new(crate::backend::static_visual::HostStaticVisualProvider),
                cancel.clone(),
                &mut metrics,
            )
            .await
            .map_err(|_| crate::runtime::RuntimeError::ResourceUnavailable)?;
            let artifact = ArtifactMaterializer::materialize_owned(
                snapshot.descriptor.clone(),
                &bytes[..],
                Some((snapshot.region, snapshot.quality)),
                Duration::from_secs(300),
                true,
                &cancel,
                artifact_owner,
            )
            .map_err(|_| crate::runtime::RuntimeError::ResourceUnavailable)?;
            if cancel.is_cancelled() {
                return Err(crate::runtime::RuntimeError::Cancelled);
            }
            metrics.headless_materialization = 1;
            modality.resolution = crate::modality::ModalityResource::RenderedSnapshot(snapshot);
            tracing::debug!(?metrics, "explicit static modality request");
            Ok(CaptureCompletion {
                candidate,
                modality,
                artifact,
            })
        }));
        self.status = "Acquiring one explicit static frame; TUI remains usable".into();
    }

    fn open_materialized_same_host(&mut self) {
        if self.modality_task.is_some() {
            self.status = "A handoff is already pending".into();
            return;
        }
        let Some(socket) = self.modality_socket.clone() else {
            self.status = "Headless: artifact remains on this host; no endpoint required".into();
            return;
        };
        let Some(crate::modality::ExternalModality {
            resolution: crate::modality::ModalityResource::RenderedSnapshot(snapshot),
            ..
        }) = self
            .modality_view
            .as_ref()
            .and_then(|v| v.resolved.as_ref())
        else {
            return;
        };
        let Some(artifact) = self
            .materialized_artifacts
            .iter()
            .rev()
            .find(|a| !a.expired() && a.metadata.descriptor.hash == snapshot.descriptor.hash)
        else {
            self.status = "Artifact expired; explicitly materialize again".into();
            return;
        };
        let resource = crate::modality::runtime::materialized_reference(artifact);
        #[cfg(debug_assertions)]
        let test_transfer = std::env::var_os("GUI2TUI_TEST_TRANSFER_PROGRESS").map(|progress| {
            (
                artifact.path(),
                artifact.metadata.descriptor.clone(),
                std::path::PathBuf::from(progress),
            )
        });
        self.modality_cancel = Default::default();
        let cancel = self.modality_cancel.clone();
        self.modality_ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::ReferenceHandoff,
                cancel.clone(),
            )
            .ok();
        if self.modality_ticket.is_none() {
            return;
        }
        self.modality_task = Some(tokio::task::spawn_blocking(move || {
            // Debug-only pacing of the existing artifact protocol, for a real
            // TUI/broker crash test. Production same-host handoff stays zero-copy.
            #[cfg(debug_assertions)]
            if let Some((path, descriptor, progress)) = test_transfer {
                return match crate::modality::wire::debug_paced_artifact(
                    &socket, &path, descriptor, &progress, &cancel,
                ) {
                    Ok((crate::modality::wire::Response::Opened { .. }, _)) => {
                        "Test artifact viewer accepted RenderedSnapshot".into()
                    }
                    _ => {
                        "EndpointLost: artifact transfer failed; partial artifact not opened".into()
                    }
                };
            }
            match crate::modality::wire::send_reference_cancellable(
                &socket,
                crate::modality::ModalityKind::Image,
                resource,
                &cancel,
            ) {
                Ok(crate::modality::wire::Response::Opened {
                    artifact_bytes: 0, ..
                }) => "Same-host viewer accepted RenderedSnapshot; network_payload_bytes=0".into(),
                Ok(crate::modality::wire::Response::Failed { reason, .. }) => {
                    crate::modality::wire::user_failure_message(&reason).to_owned()
                }
                _ => "Same-host viewer unavailable; no pixels transported".into(),
            }
        }));
        self.status = "Awaiting same-host viewer authorization (local path only)".into();
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        backend: AtspiBackend,
        app_selector: String,
        inspect_options: InspectOptions,
        settle_delay: Duration,
        bootstrap_strategy: BootstrapStrategy,
        event_buffer_capacity: usize,
        presentation_mode: PresentationMode,
        spatial_layout: bool,
        initial_terminal_size: (u16, u16),
        external_text_handler_available: bool,
    ) -> Result<Self, BackendError> {
        let started = Instant::now();
        let applications = backend.applications().await?;
        let application =
            AtspiBackend::select_application(&applications, Some(&app_selector), None)?.clone();
        let application_locator = application.backend_locator.clone();
        let mut event_subscription = backend
            .subscribe_events(&application, event_buffer_capacity)
            .await?;
        // Registration/cache-population signals precede the bootstrap boundary;
        // any subsequent event is buffered and replayed.
        // Registration is complete when subscribe_events returns. Yield once
        // so already-ready cache residency signals can enter the bounded
        // queue, but do not impose a fixed startup delay; later residency
        // echoes are replayed and filtered against the bootstrap baseline.
        tokio::task::yield_now().await;
        while event_subscription.try_recv().is_ok() {}
        let _ = event_subscription.take_resync();
        let bootstrap = backend
            .bootstrap_application(&application, inspect_options, bootstrap_strategy)
            .await?;
        let bootstrap_elapsed = started.elapsed();
        let mut cache = SemanticCache::from_snapshot(bootstrap.root)
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let arena_elapsed = started.elapsed();
        enrich_relational_cache(&backend, &mut cache, presentation_mode).await?;
        let relations_elapsed = started.elapsed();
        let mut content = ContentRuntime::new(&cache, ContentCacheBudget::default());
        if spatial_layout {
            // The spatial main scene gets a small inline semantic viewport.
            // Reuse ContentRuntime's bounded reader substrate; this is not a
            // second extraction path and never reads unbounded document text.
            let roots: Vec<_> = content
                .catalog()
                .models()
                .filter(|model| model.scope_class == crate::content::ContentScopeClass::Primary)
                .filter_map(|model| {
                    model
                        .reading_order()
                        .first()
                        .copied()
                        .map(|id| (model.root, id))
                })
                .collect();
            let budget =
                inline_materialization_budget(initial_terminal_size.1.saturating_sub(3).max(1), 0);
            for (root, position) in roots {
                let _ = content
                    .materialize_viewport(&backend, &cache, root, position, budget)
                    .await;
            }
        }
        qualify_complex_text_capabilities(&backend, &mut cache, &content).await;
        let content_elapsed = started.elapsed();
        let (scene, scopes, commands, choices) =
            build_contextual_view(&cache, presentation_mode, content.catalog())?;
        tracing::debug!(
            bootstrap_ms = bootstrap_elapsed.as_secs_f64() * 1000.0,
            arena_ms = (arena_elapsed - bootstrap_elapsed).as_secs_f64() * 1000.0,
            relations_ms = (relations_elapsed - arena_elapsed).as_secs_f64() * 1000.0,
            content_ms = (content_elapsed - relations_elapsed).as_secs_f64() * 1000.0,
            scene_ms = (started.elapsed() - content_elapsed).as_secs_f64() * 1000.0,
            "initial semantic TUI pipeline breakdown"
        );
        let snapshot_ms = started.elapsed().as_millis();
        if let Some(reason) = bootstrap.metrics.fallback_reason.as_deref() {
            tracing::debug!(
                bootstrap_strategy = %bootstrap.metrics.strategy,
                fallback_reason = reason,
                "semantic bootstrap used correctness fallback"
            );
        }
        let mut focus = FocusModel::default();
        focus.reconcile(&scene, None);
        let mut runtime = crate::runtime::RuntimeSession::default();
        runtime.open_application(application_locator.clone());
        let spatial = if spatial_layout {
            build_spatial_layout(&backend, &cache, &content, runtime.generation()).await
        } else {
            None
        };
        let (mut layout_analysis, spatial_evidence) = spatial
            .map(|(analysis, evidence)| (Some(analysis), Some(evidence)))
            .unwrap_or((None, None));
        if let Some(layout) = layout_analysis.as_mut() {
            refine_layout_demands_from_scene(layout, &scene);
        }
        let active_region = layout_analysis.as_ref().and_then(default_active_region);
        let inline_materialized_extent = if spatial_layout {
            inline_materialization_budget(initial_terminal_size.1.saturating_sub(3).max(1), 0)
                .visible_blocks
        } else {
            0
        };
        let mut application = Self {
            capture_task: None,
            capture_ticket: None,
            runtime,
            modality_ticket: None,
            runtime_status_visible: false,
            help_visible: None,
            help_scroll: 0,
            modality_socket: None,
            modality_view: None,
            modality_task: None,
            modality_cancel: Default::default(),
            materialized_artifacts: Vec::new(),
            backend,
            app_selector,
            application_locator,
            inspect_options,
            bootstrap_strategy,
            settle_delay,
            cache,
            event_subscription,
            event_stream_available: true,
            presentation_mode,
            spatial_layout,
            layout_analysis,
            spatial_evidence,
            active_region,
            inline_materialized_extent,
            scene,
            scopes,
            commands,
            choices,
            recent_commands: HashMap::new(),
            scope_focus_history: HashMap::new(),
            focus,
            viewport: Viewport::default(),
            viewport_height: initial_terminal_size.1.saturating_sub(3).max(1),
            viewport_width: initial_terminal_size.0.saturating_sub(2).max(1),
            hit_map: HitMap::default(),
            status: format!(
                "Loaded {} semantic nodes via {} in {snapshot_ms} ms",
                bootstrap.metrics.node_count, bootstrap.metrics.strategy
            ),
            application_available: true,
            backend_available: true,
            external_text_handler_available,
            edit_session: None,
            external_text_requested: false,
            command_palette: None,
            choice_overlay: None,
            content,
            content_view: None,
            content_return: None,
            reader_stale_fallbacks: 0,
        };
        application.reconcile_active_region();
        if let Some(EventDelivery::ResyncRequired { dropped }) =
            application.event_subscription.take_resync()
        {
            application.resynchronize_after_overflow(dropped).await;
        } else {
            let mut buffered = Vec::new();
            while let Ok(event) = application.event_subscription.try_recv() {
                buffered.push(event);
            }
            if !buffered.is_empty() {
                application
                    .apply_event_batch(buffered, Some("Replayed bootstrap events".to_owned()))
                    .await;
            }
        }
        Ok(application)
    }

    fn region_focus_candidates(&self) -> Vec<SpatialRegionId> {
        let Some(layout) = self.layout_analysis.as_ref() else {
            return Vec::new();
        };
        region_focus_order(&layout.plan, &self.scene)
    }

    fn region_navigator(&self) -> Option<RegionNavigator> {
        self.layout_analysis
            .as_ref()
            .map(|layout| RegionNavigator::derive(&layout.plan, &self.scene))
    }

    fn reconcile_active_region(&mut self) {
        let candidates = self.region_focus_candidates();
        if !self
            .active_region
            .is_some_and(|active| candidates.contains(&active))
        {
            self.active_region = self
                .layout_analysis
                .as_ref()
                .and_then(default_active_region)
                .filter(|active| candidates.contains(active))
                .or_else(|| candidates.first().copied());
        }
    }

    fn cycle_region(&mut self, reverse: bool) {
        let Some(navigator) = self.region_navigator() else {
            return;
        };
        let Some(next) = navigator.cycle_major(self.active_region, reverse) else {
            return;
        };
        self.activate_region(next);
    }

    fn cycle_subregion(&mut self, reverse: bool) {
        let Some(navigator) = self.region_navigator() else {
            return;
        };
        let Some(next) = navigator.cycle_subregion(self.active_region, reverse) else {
            return;
        };
        self.activate_region(next);
    }

    fn activate_region(&mut self, region_id: SpatialRegionId) {
        self.active_region = Some(region_id);
        self.viewport.offset = 0;
        let _ = self.focus_within_active_region(false);
        if let Some(region) = self.layout_analysis.as_ref().and_then(|layout| {
            layout
                .plan
                .regions
                .iter()
                .find(|region| Some(region.id) == self.active_region)
        }) {
            self.status = format!("Region: {}", region.presentation.title);
        }
    }

    fn focus_within_active_region(&mut self, reverse: bool) -> bool {
        let Some(active) = self.active_region else {
            return false;
        };
        let Some(region) = self.layout_analysis.as_ref().and_then(|layout| {
            layout
                .plan
                .regions
                .iter()
                .find(|region| region.id == active)
        }) else {
            return false;
        };
        let ids = self
            .scene
            .elements
            .iter()
            .filter(|element| {
                element.is_focusable()
                    && element
                        .sources
                        .iter()
                        .any(|source| region.presentation.source_nodes.contains(source))
            })
            .map(|element| element.id)
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return false;
        }
        let current = self
            .focus
            .current()
            .and_then(|id| ids.iter().position(|candidate| *candidate == id));
        let next = if reverse {
            current.map_or(ids.len() - 1, |index| {
                if index == 0 { ids.len() - 1 } else { index - 1 }
            })
        } else {
            current.map_or(0, |index| (index + 1) % ids.len())
        };
        self.focus.set(&self.scene, ids[next])
    }

    fn inline_content(&self, layout: &LayoutAnalysis) -> Option<InlineContentRender> {
        let primary =
            layout.plan.regions.iter().find(|region| {
                region.kind == crate::transcompile::SpatialRegionKind::PrimaryContent
            })?;
        if primary.presentation.kind
            == crate::transcompile::RegionPresentationKind::GraphicalPlaceholder
        {
            return Some(InlineContentRender {
                title: primary.presentation.title.clone(),
                lines: vec![
                    "[fidelity-required content]".into(),
                    "[View / Materialize]".into(),
                ],
                total_lines: 2,
                partial: false,
            });
        }
        if primary.presentation.kind != crate::transcompile::RegionPresentationKind::InlineContent {
            return None;
        }
        let root = primary
            .presentation
            .source_nodes
            .iter()
            .find_map(|id| self.content.model(*id).map(|_| *id));
        if let Some(root) = root {
            let model = self.content.model(root)?;
            let mut lines = vec![format!(
                "Document: {}",
                model
                    .metadata
                    .title
                    .as_deref()
                    .unwrap_or("semantic content")
            )];
            let row_budget = usize::from(self.viewport_height)
                .saturating_mul(3)
                .saturating_add(usize::from(self.viewport.offset))
                .max(24);
            let mut total_lines = 1_usize;
            for id in model.reading_order().into_iter().take(row_budget) {
                let Some(block) = model.block(id) else {
                    continue;
                };
                let text = self
                    .content
                    .displayed_block_text(root, block.id)
                    .filter(|text| {
                        text != "[text unavailable through the application's accessibility interface]"
                    })
                    .or_else(|| {
                        self.cache
                            .node(block.source)
                            .filter(|node| {
                                node.text_input_kind
                                    != Some(crate::semantic::TextInputKind::Password)
                            })
                            .and_then(|node| node.value.clone())
                    })
                    .unwrap_or_else(|| {
                        "[text unavailable through the application's accessibility interface]"
                            .to_owned()
                    });
                let prefix = match block.kind {
                    crate::content::ContentBlockKind::Heading { .. } => "# ",
                    crate::content::ContentBlockKind::Link => "[Link] ",
                    crate::content::ContentBlockKind::ListItem => "• ",
                    crate::content::ContentBlockKind::OpaqueContent(_) => "[Media] ",
                    _ => "",
                };
                let mut first = true;
                for logical_line in text.lines() {
                    total_lines = total_lines.saturating_add(1);
                    if lines.len() >= row_budget {
                        continue;
                    }
                    lines.push(if first {
                        format!("{prefix}{logical_line}")
                    } else {
                        logical_line.to_owned()
                    });
                    first = false;
                }
            }
            if lines.is_empty() {
                lines.push("[semantic content exposed; no realized blocks]".into());
            }
            let lines = compress_degradation_lines(lines);
            return Some(InlineContentRender {
                title: model
                    .metadata
                    .title
                    .clone()
                    .unwrap_or_else(|| "Document".into()),
                lines,
                total_lines: total_lines.max(model.blocks.len()),
                partial: model.completeness != ContentCompleteness::Complete,
            });
        }
        None
    }

    fn replan_spatial_layout(&mut self) {
        if !self.spatial_layout {
            return;
        }
        let (Some(generation), Some(evidence)) =
            (self.runtime.generation(), self.spatial_evidence.as_ref())
        else {
            self.layout_analysis = None;
            return;
        };
        if evidence.generation != generation {
            self.layout_analysis = None;
            return;
        }
        let Ok(tree) = self.cache.materialize_tree() else {
            self.layout_analysis = None;
            return;
        };
        let graph = crate::semantic::RelationalSemanticGraph::new(&self.cache);
        let analysis = analyze_regions_with_graph(&tree, &graph);
        let presentation = RegionPresentationContext::from_content_runtime(&self.content);
        let mut layout =
            infer_layout_with_presentations(&analysis, &tree, evidence, Some(&presentation));
        refine_layout_demands_from_scene(&mut layout, &self.scene);
        self.layout_analysis = Some(layout);
        self.reconcile_active_region();
    }

    async fn recollect_spatial_layout(&mut self) {
        if !self.spatial_layout {
            return;
        }
        let result = build_spatial_layout(
            &self.backend,
            &self.cache,
            &self.content,
            self.runtime.generation(),
        )
        .await;
        if let Some((mut analysis, evidence)) = result {
            refine_layout_demands_from_scene(&mut analysis, &self.scene);
            self.layout_analysis = Some(analysis);
            self.spatial_evidence = Some(evidence);
            self.reconcile_active_region();
        } else {
            self.layout_analysis = None;
            self.spatial_evidence = None;
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        if let Some(context) = self.help_visible {
            use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
            frame.render_widget(
                Paragraph::new(format!("{}{}", context.text(), super::help::GLOBAL))
                    .wrap(Wrap { trim: true })
                    .scroll((self.help_scroll, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("GUI2TUI Help — Esc return"),
                    ),
                frame.area(),
            );
            return;
        }
        if self.runtime_status_visible {
            use ratatui::widgets::{Block, Borders, Paragraph};
            frame.render_widget(
                Paragraph::new(
                    serde_json::to_string_pretty(&self.runtime_status()).unwrap_or_default(),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Runtime status (contents-free) — F12/Esc return"),
                ),
                frame.area(),
            );
            return;
        }
        if !self.application_available || !self.backend_available {
            use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
            let message = if !self.backend_available {
                "Desktop accessibility service unavailable.\nCached controls are read-only.\n\nF5: retry\nb: applications\nd: diagnostics\nq: quit"
            } else {
                self.status.as_str()
            };
            frame.render_widget(
                Paragraph::new(message).wrap(Wrap { trim: true }).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Application unavailable — F5 retry | b Apps | d Diagnose"),
                ),
                frame.area(),
            );
            return;
        }
        if let Some(view) = &self.modality_view {
            view.render(frame, &self.status);
            return;
        }
        self.viewport_height = frame.area().height.saturating_sub(3).max(1);
        self.viewport_width = frame.area().width.saturating_sub(2).max(1);
        let palette = self.command_palette.as_ref().map(|palette| PaletteRender {
            query: palette.query(),
            entries: palette.entries(),
            selected: palette.selected(),
            all_scopes: palette.searches_all_scopes(),
        });
        let choice = self.choice_overlay.as_ref().map(|overlay| ChoiceRender {
            label: self
                .cache
                .node(overlay.choice().owner)
                .and_then(|node| node.name.as_deref())
                .unwrap_or("choice"),
            options: overlay.choice().options.options(),
            selected: overlay.selected(),
            partial: matches!(overlay.choice().options, ChoiceOptions::Partial(_)),
        });
        let content = self.content_view.as_ref().and_then(|view| {
            let model = self.content.model(view.root)?;
            let outline = model
                .navigation
                .headings
                .iter()
                .filter_map(|id| model.block(*id))
                .map(|block| {
                    let level = match block.kind {
                        crate::content::ContentBlockKind::Heading { level } => level,
                        _ => None,
                    };
                    (
                        self.content
                            .displayed_block_text(view.root, block.id)
                            .unwrap_or_else(|| "Untitled heading".to_owned()),
                        level,
                    )
                })
                .collect();
            let structure_lines = if let Some(table) = &view.table {
                let mut lines = Vec::new();
                if table.completeness != crate::semantic::CollectionCompleteness::Complete {
                    lines.push("Partial table view — only realized cells are shown.".to_owned());
                }
                for cell in &table.cells {
                    let marker = if table.position.cell == Some(cell.source) {
                        ">"
                    } else {
                        " "
                    };
                    lines.push(format!(
                        "{marker} r{} c{}  {}",
                        cell.row + 1,
                        cell.column + 1,
                        cell.label
                    ));
                }
                lines
            } else if let Some(collection) = &view.virtual_collection {
                let mut lines = Vec::new();
                if collection.completeness != crate::semantic::CollectionCompleteness::Complete {
                    lines.push(
                        "Partial collection — only currently exposed items are available."
                            .to_owned(),
                    );
                }
                lines.extend(collection.realized_items.iter().map(|id| {
                    let marker = if collection.current == Some(*id) {
                        ">"
                    } else {
                        " "
                    };
                    let selected = if collection.selected_items.contains(id) {
                        "*"
                    } else {
                        " "
                    };
                    let label = self
                        .cache
                        .node(*id)
                        .and_then(|node| node.name.as_deref())
                        .unwrap_or("[unnamed item]");
                    format!("{marker} {selected} {label}")
                }));
                lines
            } else {
                Vec::new()
            };
            Some(ContentRender {
                title: model
                    .metadata
                    .title
                    .clone()
                    .or_else(|| {
                        self.cache
                            .node(view.root)
                            .and_then(|node| node.name.clone())
                    })
                    .unwrap_or_else(|| "Document".to_owned()),
                mode: view.mode,
                blocks: view.reader_blocks.clone(),
                outline,
                outline_selected: view.outline_selected,
                query: view.query.clone(),
                results: view.results.clone(),
                result_selected: view.result_selected,
                partial: model.completeness != ContentCompleteness::Complete,
                full_search: view
                    .full_search
                    .as_ref()
                    .map(|search| (search.state.clone(), search.progress)),
                structure_lines,
            })
        });
        let inline_content = self
            .layout_analysis
            .as_ref()
            .and_then(|layout| self.inline_content(layout));
        let regions = render(
            frame,
            RenderContext {
                scene: &self.scene,
                focused: self.focus.current(),
                scroll_offset: self.viewport.offset,
                status: &self.status,
                application_available: self.application_available,
                external_text_handler_available: self.external_text_handler_available,
                edit_session: self.edit_session.as_ref(),
                palette,
                choice,
                content,
                spatial: self.layout_analysis.as_ref().map(|analysis| &analysis.plan),
                active_region: self.active_region,
                inline_content,
            },
        );
        self.hit_map.replace(regions);
    }

    pub async fn handle_intent(&mut self, intent: UiIntent) -> bool {
        if !self.application_available {
            return intent == UiIntent::Quit;
        }
        if !self.backend_available
            && !matches!(
                intent,
                UiIntent::Quit
                    | UiIntent::RegionNext
                    | UiIntent::RegionPrevious
                    | UiIntent::SubregionNext
                    | UiIntent::SubregionPrevious
                    | UiIntent::FocusNext
                    | UiIntent::FocusPrevious
                    | UiIntent::ScrollLines(_)
                    | UiIntent::ScrollPages(_)
            )
        {
            self.status = crate::runtime::RuntimeError::BackendUnavailable.to_string();
            return false;
        }
        match intent {
            UiIntent::Quit => return true,
            UiIntent::RegionNext => self.cycle_region(false),
            UiIntent::RegionPrevious => self.cycle_region(true),
            UiIntent::SubregionNext => self.cycle_subregion(false),
            UiIntent::SubregionPrevious => self.cycle_subregion(true),
            UiIntent::FocusNext => {
                if !self.focus_within_active_region(false) {
                    self.focus.next(&self.scene);
                }
                self.ensure_focus_visible();
                self.ensure_focused_relations().await;
            }
            UiIntent::FocusPrevious => {
                if !self.focus_within_active_region(true) {
                    self.focus.previous(&self.scene);
                }
                self.ensure_focus_visible();
                self.ensure_focused_relations().await;
            }
            UiIntent::Activate
            | UiIntent::Toggle
            | UiIntent::Select
            | UiIntent::OpenMenu
            | UiIntent::IncreaseValue
            | UiIntent::DecreaseValue => self.execute_focused(intent).await,
            UiIntent::BeginChoice => self.begin_choice(),
            UiIntent::ClosePopup => {
                self.status =
                    "Popup closing is resolved from its active interaction scope".to_owned()
            }
            UiIntent::BeginEdit => self.begin_edit().await,
            UiIntent::BeginExternalEdit => self.external_text_requested = true,
            UiIntent::CommitEdit => self.commit_edit().await,
            UiIntent::CancelEdit => self.cancel_edit(),
            UiIntent::OpenCommandPalette => {
                let palette = CommandPalette::new(
                    self.commands.clone(),
                    self.scopes.active(),
                    self.recent_commands.clone(),
                );
                self.status = format!("Command palette — {} commands", palette.entries().len());
                self.command_palette = Some(palette);
            }
            UiIntent::BeginRead => self.begin_read().await,
            UiIntent::OpenOutline => self.open_outline(),
            UiIntent::OpenContentSearch => self.open_content_search(),
            UiIntent::Refresh => {
                self.full_reload(Some("Forced full semantic snapshot".to_owned()))
                    .await;
            }
            UiIntent::ScrollLines(delta) => {
                let content_height = self
                    .layout_analysis
                    .as_ref()
                    .and_then(|layout| self.inline_content(layout))
                    .map(|content| content.total_lines.min(usize::from(u16::MAX)) as u16)
                    .unwrap_or_else(|| self.scene.content_height(self.viewport_width));
                self.viewport
                    .scroll_lines(delta, content_height, self.viewport_height);
            }
            UiIntent::ScrollPages(pages) => {
                let content_height = self
                    .layout_analysis
                    .as_ref()
                    .and_then(|layout| self.inline_content(layout))
                    .map(|content| content.total_lines.min(usize::from(u16::MAX)) as u16)
                    .unwrap_or_else(|| self.scene.content_height(self.viewport_width));
                self.viewport
                    .scroll_pages(pages, content_height, self.viewport_height);
            }
        }
        false
    }

    pub async fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyEventKind};
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.code == KeyCode::Char('c')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return true;
        }
        if self.help_visible.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => self.help_visible = None,
                KeyCode::Down => self.help_scroll = self.help_scroll.saturating_add(1).min(30),
                KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                _ => {}
            }
            return false;
        }
        let typing = self.edit_session.is_some()
            || self.command_palette.is_some()
            || self
                .content_view
                .as_ref()
                .is_some_and(|view| view.mode == ContentViewMode::Search);
        if key.code == KeyCode::F(1) || (key.code == KeyCode::Char('?') && !typing) {
            use super::help::HelpContext as H;
            self.help_visible = Some(if !self.application_available || !self.backend_available {
                H::Unavailable
            } else if self.edit_session.is_some() {
                H::Edit
            } else if self.modality_view.is_some() {
                H::Modality
            } else if self.command_palette.is_some() {
                H::Command
            } else if self.choice_overlay.is_some() {
                H::Choice
            } else if let Some(view) = &self.content_view {
                match view.mode {
                    ContentViewMode::Reader => H::Reader,
                    ContentViewMode::Outline => H::Outline,
                    ContentViewMode::Search => H::Search,
                    ContentViewMode::Table => H::Table,
                    ContentViewMode::VirtualCollection => H::Collection,
                }
            } else {
                H::Scene
            });
            self.help_scroll = 0;
            return false;
        }
        if key.code == KeyCode::F(12) {
            self.runtime_status_visible = !self.runtime_status_visible;
            tracing::debug!(status = %self.runtime_status(), "runtime status requested");
            return false;
        }
        if self.runtime_status_visible {
            if key.code == KeyCode::Esc {
                self.runtime_status_visible = false;
            }
            return false;
        }
        if !self.backend_available {
            if key.code == KeyCode::F(5) {
                self.reconnect_backend().await;
            }
            return matches!(key.code, KeyCode::Char('q') | KeyCode::Esc);
        }
        if !self.application_available {
            if key.code == KeyCode::F(5) {
                self.open_fresh_generation().await;
            }
            return matches!(key.code, KeyCode::Char('q') | KeyCode::Esc);
        }
        if self.modality_view.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.modality_view = None;
                    self.status =
                        "Returned from modality task; semantic position preserved".to_owned();
                }
                KeyCode::Up | KeyCode::Down => {
                    self.modality_view
                        .as_mut()
                        .unwrap()
                        .move_selection(if key.code == KeyCode::Up { -1 } else { 1 });
                    self.resolve_selected_modality().await;
                }
                KeyCode::Enter => self.handoff_selected_modality(),
                KeyCode::Char('m') => self.materialize_selected_modality().await,
                KeyCode::Char('o') => self.open_materialized_same_host(),
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::F(4)
            && self.edit_session.is_none()
            && self.choice_overlay.is_none()
            && self.command_palette.is_none()
        {
            self.begin_modality().await;
            return false;
        }
        if let Some(mut content_view) = self.content_view.take() {
            let command = content_view.handle_key(key);
            self.content_view = Some(content_view);
            self.handle_content_view_command(command).await;
            return false;
        }
        if key.code == crossterm::event::KeyCode::Esc
            && let Some(view) = self.content_return.take()
        {
            self.content_view = Some(view);
            self.status = "Returned to Reader semantic position".to_owned();
            self.refresh_content_view().await;
            return false;
        }
        if let Some(mut overlay) = self.choice_overlay.take() {
            match overlay.handle_key(key) {
                ChoiceOverlayOutcome::Continue => self.choice_overlay = Some(overlay),
                ChoiceOverlayOutcome::Cancel => {
                    self.restore_choice_owner(&overlay);
                    self.status = "Choice selection cancelled; GUI unchanged".to_owned();
                }
                ChoiceOverlayOutcome::Select(option) => {
                    self.execute_choice(&overlay, option).await;
                }
            }
            return false;
        }
        if let Some(mut palette) = self.command_palette.take() {
            match palette.handle_key(key) {
                PaletteOutcome::Continue => self.command_palette = Some(palette),
                PaletteOutcome::Close => self.status = "Command palette closed".to_owned(),
                PaletteOutcome::Execute(runtime_id, intent) => {
                    if let Some(scene_id) = self.scene.scene_id_for_runtime(runtime_id) {
                        self.focus.set(&self.scene, scene_id);
                        self.execute_focused(intent).await;
                    } else {
                        self.execute_cached_command(runtime_id, intent).await;
                    }
                }
            }
            return false;
        }
        if self.edit_session.is_some() {
            return match key_to_edit_command(key) {
                EditCommand::Insert(character) => {
                    self.edit_session.as_mut().unwrap().buffer.insert(character);
                    false
                }
                EditCommand::Left => {
                    self.edit_session.as_mut().unwrap().buffer.move_left();
                    false
                }
                EditCommand::Right => {
                    self.edit_session.as_mut().unwrap().buffer.move_right();
                    false
                }
                EditCommand::Home => {
                    self.edit_session.as_mut().unwrap().buffer.home();
                    false
                }
                EditCommand::End => {
                    self.edit_session.as_mut().unwrap().buffer.end();
                    false
                }
                EditCommand::Backspace => {
                    self.edit_session.as_mut().unwrap().buffer.backspace();
                    false
                }
                EditCommand::Delete => {
                    self.edit_session.as_mut().unwrap().buffer.delete();
                    false
                }
                EditCommand::Commit => self.handle_intent(UiIntent::CommitEdit).await,
                EditCommand::Cancel => self.handle_intent(UiIntent::CancelEdit).await,
                EditCommand::BlockedTab => {
                    self.status = "Commit or cancel editing first".to_owned();
                    false
                }
                EditCommand::Quit => true,
                EditCommand::Ignore => false,
            };
        }
        let focused_document = self
            .focus
            .current()
            .and_then(|id| self.scene.element(id))
            .is_some_and(|element| {
                matches!(element.kind, SceneElementKind::DocumentSummary { .. })
            });
        if focused_document {
            match key.code {
                crossterm::event::KeyCode::Char('e')
                    if self
                        .focus
                        .current()
                        .and_then(|id| self.scene.element(id))
                        .is_some_and(|element| {
                            matches!(
                                element.kind,
                                SceneElementKind::DocumentSummary {
                                    external_edit: true,
                                    ..
                                }
                            )
                        }) =>
                {
                    self.handle_intent(UiIntent::BeginExternalEdit).await;
                    return false;
                }
                crossterm::event::KeyCode::Char('o') => {
                    self.begin_read().await;
                    self.open_outline();
                    return false;
                }
                crossterm::event::KeyCode::Char('/') => {
                    self.begin_read().await;
                    self.open_content_search();
                    return false;
                }
                _ => {}
            }
        }
        let focused_value_intent = match key.code {
            crossterm::event::KeyCode::Up
                if self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| {
                        element.capability() == InteractionCapability::AdjustValue
                    }) =>
            {
                Some(UiIntent::IncreaseValue)
            }
            crossterm::event::KeyCode::Down
                if self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| {
                        element.capability() == InteractionCapability::AdjustValue
                    }) =>
            {
                Some(UiIntent::DecreaseValue)
            }
            _ => None,
        };
        if let Some(mut intent) = focused_value_intent.or_else(|| key_to_intent(key)) {
            if intent == UiIntent::Activate
                && self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| matches!(element.kind, SceneElementKind::Field { .. }))
            {
                intent = UiIntent::BeginEdit;
            } else if intent == UiIntent::Activate
                && self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| {
                        matches!(element.kind, SceneElementKind::Selector { .. })
                    })
            {
                intent = UiIntent::BeginChoice;
            } else if intent == UiIntent::Activate
                && self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| {
                        matches!(element.kind, SceneElementKind::DocumentSummary { .. })
                    })
            {
                intent = UiIntent::BeginRead;
            }
            self.handle_intent(intent).await
        } else {
            false
        }
    }

    pub async fn handle_mouse(&mut self, intent: MouseIntent) {
        if !self.application_available
            || !self.backend_available
            || self.runtime_status_visible
            || self.help_visible.is_some()
        {
            return;
        }
        if self.modality_view.is_some() {
            return;
        }
        match intent {
            MouseIntent::Scroll(delta) => {
                self.viewport.scroll_lines(
                    delta,
                    self.scene.content_height(self.viewport_width),
                    self.viewport_height,
                );
            }
            MouseIntent::Click { x, y } => {
                let Some(region) = self.hit_map.hit(x, y) else {
                    return;
                };
                self.focus.set(&self.scene, region.scene_id);
                self.ensure_focus_visible();
                match region.interaction {
                    HitInteraction::Activate => {
                        let intent = self
                            .scene
                            .element(region.scene_id)
                            .map(intent_for_element)
                            .unwrap_or(UiIntent::Activate);
                        self.execute_focused(intent).await;
                    }
                    HitInteraction::Unavailable => self.report_unavailable(region.scene_id),
                    HitInteraction::Focus => {}
                }
            }
        }
    }

    async fn begin_read(&mut self) {
        let Some(scene_id) = self.focus.current() else {
            self.status = "No document summary is focused".to_owned();
            return;
        };
        let Some(element) = self.scene.element(scene_id) else {
            return;
        };
        let Some(binding) = element.binding.as_ref() else {
            return;
        };
        let root = binding.runtime_id;
        let Some(model) = self.content.model(root) else {
            self.status = "Document content is no longer available".to_owned();
            return;
        };
        let Some(position) = model.reading_order().first().copied() else {
            self.status = "Document exposes no readable semantic blocks".to_owned();
            return;
        };
        self.content_view = Some(ContentViewState::new(
            root,
            position,
            Some(scene_id),
            Some(root),
        ));
        self.refresh_content_view().await;
    }

    fn open_outline(&mut self) {
        if let Some(view) = self.content_view.as_mut() {
            view.mode = ContentViewMode::Outline;
            if let Some(model) = self.content.model(view.root) {
                view.outline_selected = model
                    .navigation
                    .headings
                    .iter()
                    .position(|id| *id == view.position)
                    .unwrap_or(0);
            }
            self.status = "Document outline — Enter reads from heading; Esc returns".to_owned();
        }
    }

    fn open_content_search(&mut self) {
        if let Some(view) = self.content_view.as_mut() {
            view.mode = ContentViewMode::Search;
            view.results = self.content.search(view.root, &view.query);
            view.result_selected = 0;
            self.status =
                "Content search covers indexed labels and currently loaded text".to_owned();
        }
    }

    async fn handle_content_view_command(&mut self, command: ContentViewCommand) {
        match command {
            ContentViewCommand::Continue => {}
            ContentViewCommand::Close => {
                let Some(view) = self.content_view.take() else {
                    return;
                };
                if let Some(scene_id) = view.restore_scene
                    && self.scene.element(scene_id).is_some()
                {
                    self.focus.set(&self.scene, scene_id);
                } else {
                    self.focus
                        .reconcile_identity(&self.scene, view.restore_runtime, None);
                }
                self.status = "Reader closed; document focus restored".to_owned();
            }
            ContentViewCommand::MoveBlocks(delta) => {
                let Some(view) = self.content_view.as_mut() else {
                    return;
                };
                let Some(model) = self.content.model(view.root) else {
                    self.content_view = None;
                    self.status = "Document disappeared; Reader closed".to_owned();
                    return;
                };
                let order = model.reading_order();
                let current = order
                    .iter()
                    .position(|id| *id == view.position)
                    .unwrap_or(0);
                let next = move_index(current, delta, order.len());
                if let Some(id) = order.get(next) {
                    view.position = *id;
                }
                self.refresh_content_view().await;
            }
            ContentViewCommand::OpenOutline => self.open_outline(),
            ContentViewCommand::OpenSearch => self.open_content_search(),
            ContentViewCommand::OutlineMove(delta) => {
                if let Some(view) = self.content_view.as_mut()
                    && let Some(model) = self.content.model(view.root)
                {
                    view.outline_selected = move_index(
                        view.outline_selected,
                        delta,
                        model.navigation.headings.len(),
                    );
                }
            }
            ContentViewCommand::ChooseOutline => {
                if let Some(view) = self.content_view.as_mut()
                    && let Some(model) = self.content.model(view.root)
                    && let Some(id) = model.navigation.headings.get(view.outline_selected)
                {
                    view.position = *id;
                    view.mode = ContentViewMode::Reader;
                }
                self.refresh_content_view().await;
            }
            ContentViewCommand::SearchChanged => {
                if let Some(view) = self.content_view.as_mut() {
                    if let Some(search) = view.full_search.as_mut() {
                        search.cancel();
                    }
                    view.full_search = None;
                    view.results = self.content.search(view.root, &view.query);
                    view.result_selected = 0;
                }
            }
            ContentViewCommand::SearchMove(delta) => {
                if let Some(view) = self.content_view.as_mut() {
                    view.result_selected =
                        move_index(view.result_selected, delta, view.results.len());
                }
            }
            ContentViewCommand::ChooseSearch => {
                if let Some(view) = self.content_view.as_mut()
                    && let Some(result) = view.results.get(view.result_selected)
                {
                    view.position = result.block_id;
                    view.mode = ContentViewMode::Reader;
                }
                self.refresh_content_view().await;
            }
            ContentViewCommand::StartFullSearch => {
                if let Some(view) = self.content_view.as_mut() {
                    if view.query.trim().is_empty() {
                        self.status = "Enter a query before starting full search".to_owned();
                    } else if let Some(search) = self
                        .content
                        .begin_progressive_search(view.root, view.query.clone())
                    {
                        view.results.clear();
                        view.result_selected = 0;
                        let scope = if search.progress.total_blocks.is_some() {
                            "Full document search"
                        } else {
                            "Exposed semantic content search"
                        };
                        view.full_search = Some(search);
                        self.status = format!(
                            "{scope} started; Esc cancels immediately between bounded RPCs"
                        );
                    }
                }
            }
            ContentViewCommand::CancelFullSearch => {
                if let Some(search) = self
                    .content_view
                    .as_mut()
                    .and_then(|view| view.full_search.as_mut())
                {
                    search.cancel();
                    let cache = self.content.cache_metrics();
                    let scope = if search.progress.total_blocks.is_some() {
                        "Full search"
                    } else {
                        "Exposed semantic content search"
                    };
                    self.status = format!(
                        "{scope} cancelled after {} blocks and {} text RPCs; cache={} bytes/{} ranges",
                        search.progress.scanned_blocks,
                        search.progress.text_rpcs,
                        cache.bytes,
                        cache.ranges,
                    );
                }
            }
            ContentViewCommand::OpenStructure => {
                let Some((root, position)) = self
                    .content_view
                    .as_ref()
                    .map(|view| (view.root, view.position))
                else {
                    return;
                };
                let source = self
                    .content
                    .model(root)
                    .and_then(|model| model.block(position))
                    .map(|block| block.source);
                let Some(source) = source else { return };
                let task_source = self
                    .content
                    .model(root)
                    .and_then(|model| model.block(position))
                    .and_then(|block| block.interactive_sources.first().copied())
                    .or_else(|| {
                        self.content
                            .model(root)
                            .and_then(|model| model.block(position))
                            .filter(|block| {
                                matches!(block.kind, crate::content::ContentBlockKind::Link)
                            })
                            .map(|block| block.source)
                    });
                if let Some(task_source) = task_source
                    && let Some(scene_id) = self.scene.scene_id_for_runtime(task_source)
                {
                    self.focus.set(&self.scene, scene_id);
                    self.content_return = self.content_view.take();
                    self.status =
                        "Reader task focused; Esc returns to the saved content position".to_owned();
                    return;
                }
                if let Some(table) = self.content.table(source).cloned() {
                    if let Some(view) = self.content_view.as_mut() {
                        view.table = Some(table);
                        view.virtual_collection = None;
                        view.mode = ContentViewMode::Table;
                    }
                    self.status = "Table view uses semantic row/column positions".to_owned();
                } else if let Some(collection) = self
                    .content
                    .virtual_collections()
                    .iter()
                    .find(|collection| collection.owner == source)
                    .cloned()
                {
                    if let Some(view) = self.content_view.as_mut() {
                        view.virtual_collection = Some(collection);
                        view.table = None;
                        view.mode = ContentViewMode::VirtualCollection;
                    }
                    self.status =
                        "Collection view is limited to currently realized accessibility items"
                            .to_owned();
                } else {
                    self.status =
                        "Current content block has no structured navigation task".to_owned();
                }
            }
            ContentViewCommand::StructureMove { rows, columns } => {
                if let Some(view) = self.content_view.as_mut() {
                    if let Some(table) = view.table.as_mut() {
                        table.move_by(rows, columns);
                    } else if let Some(collection) = view.virtual_collection.as_mut() {
                        collection.move_realized(rows);
                    }
                }
            }
        }
    }

    pub async fn progress_content_operations(&mut self) {
        if !self.application_available || !self.backend_available {
            return;
        }
        self.materialized_artifacts.retain(|a| !a.expired());
        self.refresh_inline_materialization().await;
        if self
            .capture_task
            .as_ref()
            .is_some_and(|task| task.is_finished())
            && let Some(task) = self.capture_task.take()
        {
            let result = task.await;
            if let Some(ticket) = self.capture_ticket.take()
                && self.runtime.complete(&ticket).is_ok()
            {
                match result {
                    Ok(Ok(completion)) => {
                        let valid = self
                            .cache
                            .node(completion.candidate.owner)
                            .is_some_and(|n| n.backend_locator == completion.candidate.locator)
                            && self.scopes.allows_node(completion.candidate.owner);
                        if valid {
                            self.status = format!(
                                "RenderedSnapshot (may be occluded): {}",
                                completion.artifact.path().display()
                            );
                            self.materialized_artifacts.push(completion.artifact);
                            if let Some(view) = &mut self.modality_view
                                && view
                                    .candidates
                                    .get(view.selected)
                                    .is_some_and(|c| c.locator == completion.candidate.locator)
                            {
                                view.resolved = Some(completion.modality);
                            }
                        } else {
                            self.status = crate::runtime::RuntimeError::StaleIdentity.to_string();
                        }
                    }
                    Ok(Err(error)) => self.status = error.to_string(),
                    Err(_) => self.status = crate::runtime::RuntimeError::Internal.to_string(),
                }
            }
        }
        if self
            .modality_task
            .as_ref()
            .is_some_and(|task| task.is_finished())
            && let Some(task) = self.modality_task.take()
        {
            let result = task.await;
            if let Some(ticket) = self.modality_ticket.take()
                && self.runtime.complete(&ticket).is_ok()
            {
                self.status = result.unwrap_or_else(|_| "Local handoff task failed".to_owned());
                // Every completed handoff invalidates the negotiated lease.
                // Reopening F4 negotiates anew; no stale fake Open remains.
                if let Some(view) = &mut self.modality_view {
                    view.capabilities = None;
                    if let Some(modality) = &mut view.resolved {
                        modality.negotiate(None);
                    }
                }
                self.runtime
                    .set_endpoint(crate::runtime::EndpointState::Disconnected);
            }
        }
        let Some(mut search) = self
            .content_view
            .as_mut()
            .and_then(|view| view.full_search.take())
        else {
            return;
        };
        if search.state == SearchState::Running {
            self.content
                .progressive_search_step(
                    &self.backend,
                    &self.cache,
                    &mut search,
                    SearchBudget::default(),
                )
                .await;
        }
        if let Some(view) = self.content_view.as_mut() {
            view.results = search.results.clone();
            if search.state == SearchState::Complete {
                self.status = completed_search_status(
                    search.progress,
                    search.results.len(),
                    self.content.cache().metrics().bytes,
                );
            }
            view.full_search = Some(search);
        }
    }

    async fn refresh_content_view(&mut self) {
        let Some((root, position)) = self
            .content_view
            .as_ref()
            .map(|view| (view.root, view.position))
        else {
            return;
        };
        match self
            .content
            .materialize_viewport(
                &self.backend,
                &self.cache,
                root,
                position,
                MaterializationBudget::default(),
            )
            .await
        {
            Ok((blocks, report)) => {
                if let Some(view) = self.content_view.as_mut() {
                    view.reader_blocks = blocks;
                }
                self.status = format!(
                    "Reader loaded {} blocks (text_rpcs={} ranges={} cache={} bytes) in {:.3} ms",
                    report.requested_blocks,
                    report.backend_text_rpcs,
                    report.loaded_ranges,
                    report.cache.bytes,
                    report.duration_micros as f64 / 1000.0
                );
            }
            Err(error) => {
                self.status = format!("Reader content load failed: {error}");
            }
        }
    }

    async fn execute_focused(&mut self, requested_intent: UiIntent) {
        let Some(scene_id) = self.focus.current() else {
            self.status = "No focusable control".to_owned();
            return;
        };
        let Some(element) = self.scene.element(scene_id).cloned() else {
            self.status = "Focused control disappeared; press r to refresh".to_owned();
            return;
        };
        if matches!(element.kind, SceneElementKind::Field { .. }) {
            self.status = "Press Enter to edit the focused text input".to_owned();
            return;
        }
        if element.capability() == InteractionCapability::None {
            self.status = format!(
                "No compatible semantic action for \"{}\"",
                element_label(&element)
            );
            return;
        }
        if element.capability() == InteractionCapability::AdjustValue
            && !matches!(
                requested_intent,
                UiIntent::IncreaseValue | UiIntent::DecreaseValue
            )
        {
            self.status = format!("Adjust \"{}\" with Up/Down", element_label(&element));
            return;
        }
        let intent = if matches!(
            requested_intent,
            UiIntent::IncreaseValue | UiIntent::DecreaseValue
        ) {
            requested_intent
        } else {
            intent_for_element(&element)
        };
        if intent == UiIntent::BeginChoice {
            self.begin_choice();
            return;
        }
        if intent == UiIntent::BeginRead {
            self.begin_read().await;
            return;
        }
        let Some(runtime_id) = element.binding.as_ref().map(|binding| binding.runtime_id) else {
            self.status = "Scene element has no semantic binding".to_owned();
            return;
        };
        let Some(semantic_operation) = SemanticOperation::from_intent(runtime_id, intent) else {
            self.status = format!(
                "No backend operation is defined for \"{}\"",
                element_label(&element)
            );
            return;
        };
        let backend_operation = match resolve_backend_operation(&self.scene, semantic_operation) {
            Ok(operation) => operation,
            Err(error) => {
                self.status = format!("Cannot operate {}: {error}", element_label(&element));
                return;
            }
        };

        let operation_description = describe_operation(intent, &backend_operation);
        let popup_owner = (intent == UiIntent::Select)
            .then(|| self.active_popup_owner())
            .flatten();
        if let BackendOperation::AdjustValue { locator, increase } = &backend_operation {
            let Some(node) = self.cache.node(runtime_id) else {
                self.status = "Value control became stale; refresh and retry".to_owned();
                return;
            };
            if node.backend_locator != *locator
                || !node
                    .capabilities
                    .contains(&crate::semantic::SemanticCapability::Value)
            {
                self.status =
                    "Value control became stale or unavailable; refresh and retry".to_owned();
                return;
            }
            match self.backend.adjust_value(locator, *increase).await {
                Ok(mutation) => {
                    let status = if (mutation.resulting - mutation.previous).abs() <= f64::EPSILON {
                        format!(
                            "Value unchanged for \"{}\" (authoritative {})",
                            element_label(&element),
                            mutation.resulting
                        )
                    } else if mutation.normalized {
                        format!(
                            "Value {} \"{}\" → {} (normalized)",
                            if *increase { "increased" } else { "decreased" },
                            element_label(&element),
                            mutation.resulting
                        )
                    } else if (mutation.resulting - mutation.requested).abs() <= f64::EPSILON {
                        format!(
                            "Value {} \"{}\" → {}",
                            if *increase { "increased" } else { "decreased" },
                            element_label(&element),
                            mutation.resulting
                        )
                    } else {
                        format!(
                            "Value outcome unverified for \"{}\"; authoritative {}",
                            element_label(&element),
                            mutation.resulting
                        )
                    };
                    match self.backend.refresh_node(locator, false).await {
                        Ok(node) => {
                            if let Err(error) = self.cache.refresh_node(node) {
                                self.status =
                                    format!("Value changed but cache refresh failed: {error}");
                                return;
                            }
                            self.rebuild_view_preserving_focus().await;
                            self.status = status;
                        }
                        Err(error) => {
                            let (reason, _) = value_operation_error_status(&error);
                            self.full_reload(Some(format!(
                                "Value confirmed as {}, but presentation refresh failed ({reason})",
                                mutation.resulting
                            )))
                            .await;
                        }
                    }
                }
                Err(error) => {
                    let (status, refresh) = value_operation_error_status(&error);
                    self.status = status;
                    if refresh {
                        self.full_reload(Some(self.status.clone())).await;
                    }
                }
            }
            return;
        }
        if let BackendOperation::InvokeAction { locator, action } = &backend_operation
            && matches!(
                intent,
                UiIntent::Activate | UiIntent::Toggle | UiIntent::OpenMenu
            )
        {
            if self
                .invoke_action_with_transition(
                    runtime_id,
                    intent,
                    locator.clone(),
                    action.clone(),
                    element_label(&element).to_owned(),
                    operation_description,
                )
                .await
            {
                *self.recent_commands.entry(runtime_id).or_default() += 1;
            }
            return;
        }
        let result = match &backend_operation {
            BackendOperation::InvokeAction { locator, action } => self
                .backend
                .do_action(&locator.encode(), action.index)
                .await
                .map(|_| ()),
            BackendOperation::SelectChild {
                container_locator,
                child_index,
            } => {
                self.backend
                    .select_child(container_locator, *child_index)
                    .await
            }
            BackendOperation::SetTextContents { .. } => {
                unreachable!("text commits use commit_edit")
            }
            BackendOperation::SetComplexTextContents { .. } => {
                unreachable!("complex text commits use the external text session")
            }
            BackendOperation::AdjustValue { .. } => unreachable!("Value operations handled above"),
        };
        match result {
            Ok(_) => {
                *self.recent_commands.entry(runtime_id).or_default() += 1;
                let status = format!(
                    "{} \"{}\" via {}",
                    operation_verb(intent),
                    element_label(&element),
                    operation_description
                );
                self.update_from_action_events(status).await;
                if let Some(owner) = popup_owner {
                    self.close_popup_after_selection(owner).await;
                }
            }
            Err(error) => {
                let (status, refresh) = operation_error_status(&error);
                self.status = status;
                if refresh {
                    let status = self.status.clone();
                    self.full_reload(Some(status)).await;
                }
            }
        }
    }

    async fn execute_cached_command(&mut self, runtime_id: RuntimeNodeId, intent: UiIntent) {
        let Some(node) = self.cache.node(runtime_id) else {
            self.status = "Command is no longer present in the semantic runtime".to_owned();
            return;
        };
        let label = node.name.clone().unwrap_or_else(|| node.role.to_string());
        let Some(operation) = SemanticOperation::from_intent(runtime_id, intent) else {
            self.status = format!("Command \"{label}\" has no executable semantic operation");
            return;
        };
        let backend_operation = match resolve_cached_node_operation(&self.cache, operation) {
            Ok(operation) => operation,
            Err(error) => {
                self.status = format!("Cannot execute command \"{label}\": {error}");
                return;
            }
        };
        let description = describe_operation(intent, &backend_operation);
        if let BackendOperation::InvokeAction { locator, action } = &backend_operation
            && matches!(
                intent,
                UiIntent::Activate | UiIntent::Toggle | UiIntent::OpenMenu
            )
        {
            if self
                .invoke_action_with_transition(
                    runtime_id,
                    intent,
                    locator.clone(),
                    action.clone(),
                    label.clone(),
                    description,
                )
                .await
            {
                *self.recent_commands.entry(runtime_id).or_default() += 1;
            }
            return;
        }
        let result = match &backend_operation {
            BackendOperation::InvokeAction { locator, action } => self
                .backend
                .do_action(&locator.encode(), action.index)
                .await
                .map(|_| ()),
            BackendOperation::SelectChild {
                container_locator,
                child_index,
            } => {
                self.backend
                    .select_child(container_locator, *child_index)
                    .await
            }
            BackendOperation::SetTextContents { .. } => {
                unreachable!("command palette never edits text")
            }
            BackendOperation::SetComplexTextContents { .. } => {
                unreachable!("command palette never edits complex text")
            }
            BackendOperation::AdjustValue { .. } => {
                unreachable!("command palette never adjusts Value controls")
            }
        };
        match result {
            Ok(()) => {
                *self.recent_commands.entry(runtime_id).or_default() += 1;
                self.update_from_action_events(format!(
                    "{} \"{}\" via {}",
                    operation_verb(intent),
                    label,
                    description
                ))
                .await;
            }
            Err(error) => {
                let (status, refresh) = operation_error_status(&error);
                self.status = status;
                if refresh {
                    self.full_reload(Some(self.status.clone())).await;
                }
            }
        }
    }

    fn begin_choice(&mut self) {
        let Some(scene_id) = self.focus.current() else {
            self.status = "No focused choice control".to_owned();
            return;
        };
        let Some(element) = self.scene.element(scene_id) else {
            self.status = "Focused choice disappeared".to_owned();
            return;
        };
        let Some(binding) = element.binding.as_ref() else {
            self.status = "Choice control has no semantic binding".to_owned();
            return;
        };
        let Some(choice) = self.choices.get(binding.runtime_id).cloned() else {
            self.status = format!("Choices for \"{}\" are unavailable", element_label(element));
            return;
        };
        if !choice.is_interactive() {
            self.status = format!(
                "Choices for \"{}\" are unavailable through accessibility; control is read-only",
                element_label(element)
            );
            return;
        }
        tracing::debug!(
            owner_runtime_id = %binding.runtime_id,
            owner_scene_id = %scene_id,
            gui_disclosure_calls = 0,
            "opened terminal-native choice overlay"
        );
        self.status = "Choice overlay — ↑/↓ Navigate | Enter Select | Esc Cancel".to_owned();
        self.choice_overlay = Some(ChoiceOverlay::new(choice, scene_id, binding.runtime_id));
    }

    async fn execute_choice(
        &mut self,
        overlay: &ChoiceOverlay,
        option: crate::transcompile::ChoiceOption,
    ) {
        let Some(strategy) = option.selection.as_ref() else {
            self.status = format!("Choice \"{}\" has no safe semantic selection", option.label);
            self.choice_overlay = Some(overlay.clone());
            return;
        };
        let operation = match resolve_choice_backend_operation(&self.cache, strategy) {
            Ok(operation) => operation,
            Err(error) => {
                self.status = format!("Cannot select \"{}\": {error}", option.label);
                self.choice_overlay = Some(overlay.clone());
                return;
            }
        };
        let operation_name = describe_operation(UiIntent::Select, &operation);
        let result = match &operation {
            BackendOperation::InvokeAction { locator, action } => self
                .backend
                .do_action(&locator.encode(), action.index)
                .await
                .map(|_| ()),
            BackendOperation::SelectChild {
                container_locator,
                child_index,
            } => {
                self.backend
                    .select_child(container_locator, *child_index)
                    .await
            }
            BackendOperation::SetTextContents { .. } => unreachable!("choice never edits text"),
            BackendOperation::SetComplexTextContents { .. } => {
                unreachable!("choice never edits complex text")
            }
            BackendOperation::AdjustValue { .. } => unreachable!("choice never adjusts Value"),
        };
        match result {
            Ok(()) => {
                let restore_runtime = overlay.restore_runtime();
                self.update_from_action_events(format!(
                    "Selected \"{}\" via {} (GUI disclosure calls=0)",
                    option.label, operation_name
                ))
                .await;
                if let Some(scene_id) = self.scene.scene_id_for_runtime(restore_runtime) {
                    self.focus.set(&self.scene, scene_id);
                }
            }
            Err(error) => {
                let (status, refresh) = operation_error_status(&error);
                self.status = status;
                if refresh {
                    self.full_reload(Some(self.status.clone())).await;
                }
            }
        }
    }

    fn restore_choice_owner(&mut self, overlay: &ChoiceOverlay) {
        if self.focus.set(&self.scene, overlay.restore_scene()) {
            return;
        }
        if let Some(scene_id) = self.scene.scene_id_for_runtime(overlay.restore_runtime()) {
            self.focus.set(&self.scene, scene_id);
        }
    }

    fn active_popup_owner(&self) -> Option<RuntimeNodeId> {
        let scope = self.scopes.scope(self.scopes.active())?;
        if !matches!(
            scope.kind,
            crate::transcompile::InteractionScopeKind::Popup
                | crate::transcompile::InteractionScopeKind::MenuPopup
        ) {
            return None;
        }
        let graph = crate::semantic::RelationalSemanticGraph::new(&self.cache);
        graph.popup_owner(scope.root).or_else(|| {
            self.cache
                .node(scope.root)
                .and_then(|node| node.parent)
                .filter(|parent| {
                    self.cache
                        .node(*parent)
                        .is_some_and(|node| node.role == crate::semantic::SemanticRole::ComboBox)
                })
        })
    }

    async fn close_popup_after_selection(&mut self, owner: RuntimeNodeId) {
        let still_open = self
            .scopes
            .scope(self.scopes.active())
            .is_some_and(|scope| {
                matches!(
                    scope.kind,
                    crate::transcompile::InteractionScopeKind::Popup
                        | crate::transcompile::InteractionScopeKind::MenuPopup
                )
            });
        if !still_open {
            return;
        }
        let operation = SemanticOperation::ClosePopup(owner);
        let Ok(BackendOperation::InvokeAction { locator, action }) =
            resolve_cached_node_operation(&self.cache, operation)
        else {
            self.status
                .push_str("; popup remains open (no safe close operation)");
            return;
        };
        match self
            .backend
            .do_action(&locator.encode(), action.index)
            .await
        {
            Ok(_) => {
                self.update_from_action_events(format!(
                    "Selection confirmed; closed popup via {}",
                    action.name
                ))
                .await;
            }
            Err(error) => {
                self.status = format!("Selection succeeded, but popup close failed: {error}");
            }
        }
    }

    async fn begin_edit(&mut self) {
        let Some(scene_id) = self.focus.current() else {
            self.status = "No focused text input".to_owned();
            return;
        };
        let Some(element) = self.scene.element(scene_id) else {
            self.status = "Focused control disappeared".to_owned();
            return;
        };
        if matches!(
            element.kind,
            SceneElementKind::Field {
                input_kind: crate::semantic::TextInputKind::Password,
                ..
            }
        ) {
            self.status = "Password editing is disabled by GUI2TUI".to_owned();
            return;
        }
        if element.capability() != InteractionCapability::EditText {
            self.status = format!("Text input \"{}\" is read-only", element_label(element));
            return;
        }
        let Some(binding) = element.binding.as_ref() else {
            self.status = "Text field has no semantic binding".to_owned();
            return;
        };
        let runtime_id = binding.runtime_id;
        let locator = binding.backend_locator.clone();
        let label = element_label(element).to_owned();
        match self.backend.read_full_editable_text(&locator).await {
            Ok(value) => {
                self.edit_session = Some(EditSession::new(
                    runtime_id,
                    locator,
                    value,
                    self.cache.generation(),
                ));
                self.status = format!(
                    "Editing \"{label}\" — Enter Commit | Esc Cancel | ←/→ Move | Backspace/Delete Edit"
                );
            }
            Err(error) => self.status = format!("Cannot edit \"{label}\": {error}"),
        }
    }

    fn cancel_edit(&mut self) {
        if self.edit_session.take().is_some() {
            self.status = "Edit cancelled; GUI value unchanged".to_owned();
        }
    }

    pub async fn begin_external_text_interaction(&mut self) -> Result<ExternalTextSession, String> {
        let scene_id = self
            .focus
            .current()
            .ok_or_else(|| "No focused document text target".to_owned())?;
        let element = self
            .scene
            .element(scene_id)
            .ok_or_else(|| "Focused text target disappeared".to_owned())?;
        if !matches!(
            element.kind,
            SceneElementKind::DocumentSummary {
                external_edit: true,
                ..
            }
        ) {
            return Err("Focused document is not qualified for external text editing".into());
        }
        let binding = element
            .binding
            .as_ref()
            .ok_or_else(|| "Focused text target has no semantic binding".to_owned())?;
        let target = binding.runtime_id;
        let locator = binding.backend_locator.clone();
        let label = element_label(element).to_owned();
        let node = self
            .cache
            .node(target)
            .ok_or_else(|| "Focused text target disappeared".to_owned())?;
        if node.backend_locator != locator
            || !node
                .capabilities
                .contains(&SemanticCapability::EditComplexText)
            || !self.scopes.allows_node(target)
        {
            return Err("Focused text target is no longer safely editable".into());
        }
        let scope = self
            .scopes
            .scope_for_node(target)
            .ok_or_else(|| "Focused text target has no active interaction scope".to_owned())?;
        let generation = self
            .runtime
            .generation()
            .ok_or_else(|| "Application generation is unavailable".to_owned())?;
        let original = self
            .backend
            .read_complete_plain_multiline_text(&locator)
            .await
            .map_err(|error| format!("Cannot acquire complete plain text: {error}"))?;
        let ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::TextInteraction,
                crate::modality::CancellationToken::default(),
            )
            .map_err(|error| error.to_string())?;
        match ExternalTextSession::new(
            target,
            locator,
            generation,
            scope,
            original,
            ticket.clone(),
            label,
        ) {
            Ok(session) => Ok(session),
            Err(error) => {
                let _ = self.runtime.complete(&ticket);
                Err(error)
            }
        }
    }

    pub async fn finish_external_text_interaction(
        &mut self,
        mut session: ExternalTextSession,
        handler: HandlerOutcome,
    ) {
        self.synchronize_after_external_handler().await;
        match handler {
            HandlerOutcome::Unchanged => {
                let _ = self.runtime.complete(&session.ticket);
                self.status = "External text unchanged; GUI was not mutated".to_owned();
            }
            HandlerOutcome::Failed { reason, modified } => {
                let _ = self.runtime.complete(&session.ticket);
                self.status = if modified {
                    preserved_status(&mut session, &format!("{reason}; GUI was not mutated"))
                } else {
                    format!("{reason}; GUI was not mutated")
                };
            }
            HandlerOutcome::Modified(candidate) => {
                if self.runtime.generation() != Some(session.generation)
                    || self.cache.node(session.target).is_none_or(|node| {
                        node.backend_locator != session.locator
                            || !node
                                .capabilities
                                .contains(&SemanticCapability::EditComplexText)
                    })
                    || self.scopes.scope_for_node(session.target) != Some(session.scope)
                    || !self.scopes.allows_node(session.target)
                {
                    let _ = self.runtime.complete(&session.ticket);
                    self.status = preserved_status(
                        &mut session,
                        "External text target became stale; GUI was not mutated",
                    );
                    return;
                }

                let current = match self
                    .backend
                    .read_complete_plain_multiline_text(&session.locator)
                    .await
                {
                    Ok(current) => current,
                    Err(_) => {
                        let _ = self.runtime.complete(&session.ticket);
                        self.status = preserved_status(
                            &mut session,
                            "External text target is unavailable or unverified; GUI was not mutated",
                        );
                        return;
                    }
                };
                if current != session.original {
                    let _ = self.runtime.complete(&session.ticket);
                    self.status = preserved_status(
                        &mut session,
                        "External text conflict detected; GUI was not overwritten",
                    );
                    return;
                }

                let operation = SemanticOperation::ReplaceComplexText {
                    target: session.target,
                    expected: session.original.clone(),
                    text: candidate.clone(),
                };
                let operation = match resolve_backend_operation(&self.scene, operation) {
                    Ok(operation) => operation,
                    Err(error) => {
                        let _ = self.runtime.complete(&session.ticket);
                        self.status = preserved_status(
                            &mut session,
                            &format!("External text write became unavailable: {error}"),
                        );
                        return;
                    }
                };
                let BackendOperation::SetComplexTextContents {
                    locator,
                    expected,
                    text,
                } = operation
                else {
                    unreachable!("complex text operation must resolve to complete text write")
                };
                let result = self
                    .backend
                    .replace_complete_plain_multiline_text(&locator, &expected, &text)
                    .await;
                let _ = self.runtime.complete(&session.ticket);
                match result {
                    Ok(mutation) if mutation.resulting == mutation.requested => {
                        self.full_reload(Some(format!(
                            "External text update confirmed — chars={}",
                            mutation.resulting.chars().count()
                        )))
                        .await;
                    }
                    Ok(_) => {
                        self.status = preserved_status(
                            &mut session,
                            "External text write was not authoritatively confirmed",
                        );
                        self.full_reload(Some(self.status.clone())).await;
                    }
                    Err(BackendError::ComplexTextConflict(_)) => {
                        self.status = preserved_status(
                            &mut session,
                            "External text conflict detected immediately before write; GUI was not overwritten",
                        );
                        self.full_reload(Some(self.status.clone())).await;
                    }
                    Err(BackendError::TextUpdateRejected(_))
                    | Err(BackendError::PermissionDenied { .. }) => {
                        self.status = preserved_status(
                            &mut session,
                            "Application rejected external text write",
                        );
                        self.full_reload(Some(self.status.clone())).await;
                    }
                    Err(_) => {
                        self.status = preserved_status(
                            &mut session,
                            "External text write could not be authoritatively verified",
                        );
                        self.full_reload(Some(self.status.clone())).await;
                    }
                }
            }
        }
    }

    async fn synchronize_after_external_handler(&mut self) {
        self.check_application_available().await;
        if !self.application_available {
            return;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
        let mut events = Vec::new();
        while let Ok(event) = self.event_subscription.try_recv() {
            events.push(event);
        }
        if let Some(EventDelivery::ResyncRequired { dropped }) =
            self.event_subscription.take_resync()
        {
            self.resynchronize_after_overflow(dropped).await;
            return;
        }
        if !events.is_empty() {
            self.apply_event_batch(events, None).await;
        }
    }

    async fn commit_edit(&mut self) {
        let Some(session) = self.edit_session.as_ref() else {
            return;
        };
        let target = session.target;
        if !session.can_commit() {
            self.status =
                "Input changed externally; cancel or reload before editing again".to_owned();
            return;
        }
        let Some(current) = self.cache.node(session.target) else {
            self.status = "Edited control disappeared; edit cancelled".to_owned();
            self.edit_session = None;
            return;
        };
        if current.backend_locator != session.backend_locator
            || current.role != crate::semantic::SemanticRole::TextInput
            || current.text_input_kind != Some(crate::semantic::TextInputKind::Plain)
            || !current
                .capabilities
                .contains(&crate::semantic::SemanticCapability::EditText)
        {
            self.status = "Edited control was replaced; edit cancelled".to_owned();
            self.edit_session = None;
            return;
        }
        let operation = SemanticOperation::ReplaceText {
            target: session.target,
            text: session.buffer.text().to_owned(),
        };
        let operation = match resolve_backend_operation(&self.scene, operation) {
            Ok(operation) => operation,
            Err(error) => {
                self.status = format!("Cannot commit text edit: {error}");
                return;
            }
        };
        let BackendOperation::SetTextContents { locator, text } = operation else {
            unreachable!("ReplaceText must resolve to SetTextContents")
        };
        if let Some(session) = self.edit_session.as_mut() {
            session.commit_pending = true;
        }
        tracing::debug!(target = %locator, chars = text.chars().count(), "replacing editable text");
        if let Err(error) = self.backend.set_text_contents(&locator, &text).await {
            if let Some(session) = self.edit_session.as_mut() {
                session.commit_pending = false;
            }
            if matches!(error, BackendError::ObjectUnavailable(_, _)) {
                self.edit_session = None;
                self.status = "Edited control was replaced; edit cancelled".to_owned();
                return;
            }
            self.status = match error {
                BackendError::TextUpdateRejected(_) => {
                    "Application rejected text update; edit buffer retained".to_owned()
                }
                _ => format!("Text update failed: {error}"),
            };
            return;
        }

        let started = Instant::now();
        let mut events = Vec::new();
        let relevant_event = loop {
            let remaining = self.settle_delay.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break false;
            }
            match tokio::time::timeout(remaining, self.event_subscription.recv()).await {
                Ok(Some(EventDelivery::Event(event))) => {
                    let relevant =
                        event_targets(&event, &locator) && event_is_text_value_change(&event);
                    events.push(event);
                    if relevant {
                        break true;
                    }
                }
                Ok(Some(EventDelivery::ResyncRequired { dropped })) => {
                    self.edit_session = None;
                    self.resynchronize_after_overflow(dropped).await;
                    return;
                }
                _ => break false,
            }
        };
        tokio::time::sleep(Duration::from_millis(40)).await;
        while let Ok(event) = self.event_subscription.try_recv() {
            events.push(event);
        }
        // The target is always refreshed explicitly below from the GUI's
        // authoritative Text/EditableText interfaces.  Do not feed echoed
        // target events through the generic dirty-scope path: older Qt emits
        // a legacy PropertyChange body alongside TextChanged, and treating an
        // otherwise-unparsed echo as Unknown would unnecessarily promote this
        // local commit to a full-application refresh.
        discard_target_echoes(&mut events, &locator);
        if !events.is_empty() {
            self.apply_event_batch(events, None).await;
        }

        let current_locator_matches = self
            .cache
            .node(target)
            .is_some_and(|node| node.backend_locator == locator);
        if !current_locator_matches {
            self.status = "Edited control was replaced; edit cancelled".to_owned();
            self.edit_session = None;
            return;
        }
        let confirmed = match self.backend.read_full_editable_text(&locator).await {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Text was submitted but confirmation failed: {error}");
                self.edit_session = None;
                return;
            }
        };
        match self.backend.refresh_node(&locator, false).await {
            Ok(mut node) => {
                node.value = Some(confirmed.clone());
                if let Err(error) = self.cache.refresh_node(node) {
                    self.status = format!("Text confirmation cache update failed: {error}");
                    self.edit_session = None;
                    return;
                }
                self.rebuild_view_preserving_focus().await;
            }
            Err(error) => {
                self.status = format!("Text was submitted but node refresh failed: {error}");
                self.edit_session = None;
                return;
            }
        }
        self.edit_session = None;
        self.status = if confirmed == text {
            format!(
                "Text update confirmed — chars={} event={} node_refresh=1 full_snapshots={}",
                confirmed.chars().count(),
                relevant_event,
                self.cache.full_snapshot_count()
            )
        } else {
            format!(
                "Application normalized or rejected submitted text; showing GUI value — event={} node_refresh=1 full_snapshots={}",
                relevant_event,
                self.cache.full_snapshot_count()
            )
        };
    }

    async fn full_reload(&mut self, success_status: Option<String>) {
        if !self.application_available {
            return;
        }
        let previous_scope = self.scopes.active();
        let previous_scene = self.focus.current();
        let previous_binding = previous_scene
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref());
        let previous_runtime = previous_binding.map(|binding| binding.runtime_id);
        let previous_locator = previous_binding.map(|binding| binding.backend_locator.clone());
        if let (Some(scene_id), Some(runtime_id), Some(locator)) =
            (previous_scene, previous_runtime, previous_locator.clone())
        {
            self.scope_focus_history.insert(
                previous_scope,
                FocusAnchor {
                    scene_id,
                    runtime_id,
                    locator,
                },
            );
        }
        let started = Instant::now();
        match load_snapshot(
            &self.backend,
            &self.app_selector,
            self.inspect_options,
            self.bootstrap_strategy,
        )
        .await
        {
            Ok(bootstrap) => {
                if !self
                    .runtime
                    .validates_application(&bootstrap.root.backend_locator)
                {
                    self.application_gone();
                    return;
                }
                let snapshot_ms = started.elapsed().as_millis();
                if let Err(error) = self.cache.full_refresh(bootstrap.root) {
                    self.status = format!("Full refresh fallback failed: {error}");
                    return;
                }
                if let Err(error) =
                    enrich_relational_cache(&self.backend, &mut self.cache, self.presentation_mode)
                        .await
                {
                    self.status = format!("Relation enrichment failed: {error}");
                    return;
                }
                self.content.rebuild_semantics(&self.cache);
                qualify_complex_text_capabilities(&self.backend, &mut self.cache, &self.content)
                    .await;
                let Ok((scene, scopes, commands, choices)) = build_contextual_view(
                    &self.cache,
                    self.presentation_mode,
                    self.content.catalog(),
                ) else {
                    self.status = "Contextual scene rebuild failed".to_owned();
                    return;
                };
                self.scene = scene;
                self.scopes = scopes;
                self.commands = commands;
                self.choices = choices;
                self.recollect_spatial_layout().await;
                let restore_anchor = (self.scopes.active() != previous_scope)
                    .then(|| self.scope_focus_history.get(&self.scopes.active()))
                    .flatten();
                let restore_runtime = restore_anchor
                    .map(|anchor| anchor.runtime_id)
                    .or(previous_runtime);
                let restore_locator = restore_anchor
                    .map(|anchor| &anchor.locator)
                    .or(previous_locator.as_ref());
                self.focus
                    .reconcile_identity(&self.scene, restore_runtime, restore_locator);
                let restored_scene = self.focus.current();
                let restored_runtime = restored_scene
                    .and_then(|id| self.scene.element(id))
                    .and_then(|element| element.binding.as_ref())
                    .map(|binding| binding.runtime_id);
                tracing::debug!(
                    refresh = "full",
                    previous_scope = %previous_scope,
                    active_scope = %self.scopes.active(),
                    previous_scene_id = ?previous_scene.map(SceneElementId::get),
                    previous_runtime_id = ?previous_runtime.map(RuntimeNodeId::get),
                    history_scene_id = ?restore_anchor.map(|anchor| anchor.scene_id.get()),
                    restored_scene_id = ?restored_scene.map(SceneElementId::get),
                    restored_runtime_id = ?restored_runtime.map(RuntimeNodeId::get),
                    "restored exact semantic focus after contextual scene rebuild"
                );
                self.application_available = true;
                self.status = format!(
                    "{} — {} nodes via {} in {snapshot_ms} ms full_snapshots={}",
                    success_status.unwrap_or_else(|| "Refreshed".to_owned()),
                    bootstrap.metrics.node_count,
                    bootstrap.metrics.strategy,
                    self.cache.full_snapshot_count()
                );
                self.ensure_focus_visible();
            }
            Err(error) if application_is_gone(&error) => {
                self.application_gone();
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error}");
            }
        }
    }

    async fn invoke_action_with_transition(
        &mut self,
        runtime_id: RuntimeNodeId,
        intent: UiIntent,
        locator: BackendLocator,
        action: SemanticAction,
        label: String,
        description: String,
    ) -> bool {
        let authority = match OperationAuthority::capture(
            &self.runtime,
            &self.application_locator,
            runtime_id,
            &locator,
            &self.cache,
            &self.scopes,
        ) {
            Ok(authority) => authority,
            Err(outcome) => {
                self.status = transition_status(outcome, &label, intent, &description);
                return false;
            }
        };
        if let Err(outcome) =
            authority.validate_before_invocation(&self.runtime, &self.cache, &self.scopes)
        {
            self.status = transition_status(outcome, &label, intent, &description);
            return false;
        }

        let condition =
            TransitionCondition::for_action(intent, runtime_id, &self.cache, &self.scopes);
        let cancellation = crate::modality::CancellationToken::default();
        let ticket = match self.runtime.begin(
            crate::runtime::OperationKind::TransitionObservation,
            cancellation.clone(),
        ) {
            Ok(ticket) => ticket,
            Err(error) => {
                self.status = error.to_string();
                return false;
            }
        };

        let Some(condition) = condition else {
            let result = self
                .backend
                .do_action_by_name(&locator.encode(), &action.name)
                .await;
            let ticket_result = self.runtime.complete(&ticket);
            return match result {
                Ok(_) if ticket_result.is_ok() => {
                    self.full_reload(None).await;
                    self.status = transition_status(
                        TransitionOutcome::Unverifiable,
                        &label,
                        intent,
                        &description,
                    );
                    tracing::debug!(
                        target: "gui2tui::product",
                        outcome = ?TransitionOutcome::Unverifiable,
                        condition = "unavailable",
                        authoritative_checks = 1_u32,
                        event_wakeups = 0_u32,
                        invocation_wakeups = 1_u32,
                        "semantic transition observation completed"
                    );
                    true
                }
                Ok(_) => {
                    self.status = crate::runtime::RuntimeError::StaleIdentity.to_string();
                    false
                }
                Err(error) => {
                    let (status, refresh) = operation_error_status(&error);
                    self.status = status;
                    if refresh {
                        self.full_reload(Some(self.status.clone())).await;
                    }
                    false
                }
            };
        };

        let condition_kind = condition.kind();
        let observation =
            TransitionObservation::new(authority, condition, self.settle_delay, cancellation);
        let mut invocation = {
            let backend = self.backend.clone();
            let encoded = locator.encode();
            tokio::spawn(async move { backend.do_action_by_name(&encoded, &action.name).await })
        };
        let mut result = self
            .observe_transition_action(&observation, &mut invocation)
            .await;
        if !invocation.is_finished() {
            // The public call may remain blocked until a newly opened modal is
            // closed. Once fresh semantics confirm the transition, its return
            // value is no longer authoritative and the bounded client wait can
            // be retired without affecting the GUI-owned surface.
            invocation.abort();
        }
        if let Err(error) = self.runtime.complete(&ticket)
            && !matches!(
                result.report.outcome,
                TransitionOutcome::ApplicationGone | TransitionOutcome::Cancelled
            )
        {
            result.report.outcome = match error {
                crate::runtime::RuntimeError::StaleIdentity => TransitionOutcome::Stale,
                _ => TransitionOutcome::Cancelled,
            };
        }

        tracing::debug!(
            target: "gui2tui::product",
            outcome = ?result.report.outcome,
            condition = condition_kind,
            authoritative_checks = result.report.authoritative_checks,
            event_wakeups = result.report.event_wakeups,
            invocation_wakeups = result.report.invocation_wakeups,
            "semantic transition observation completed"
        );
        if let Some(error) = result.rejection {
            let (status, refresh) = operation_error_status(&error);
            self.status = status;
            if refresh {
                self.full_reload(Some(self.status.clone())).await;
            }
            return false;
        }
        if result.report.outcome != TransitionOutcome::ApplicationGone {
            self.status = transition_status(result.report.outcome, &label, intent, &description);
        }
        result.invocation_accepted || result.report.outcome == TransitionOutcome::Confirmed
    }

    async fn observe_transition_action(
        &mut self,
        observation: &TransitionObservation,
        invocation: &mut tokio::task::JoinHandle<Result<SemanticAction, BackendError>>,
    ) -> ActionObservationResult {
        let mut authoritative_checks = 1_u32;
        let mut event_wakeups = 0_u32;
        let mut invocation_wakeups = 0_u32;
        let mut invocation_accepted = false;
        let mut invocation_pending = true;

        let initial = self.evaluate_transition_authoritatively(observation).await;
        if let Some(outcome) = terminal_transition_outcome(initial) {
            return ActionObservationResult {
                report: TransitionObservation::report(
                    outcome,
                    authoritative_checks,
                    event_wakeups,
                    invocation_wakeups,
                ),
                invocation_accepted,
                rejection: None,
            };
        }

        loop {
            let remaining = observation.remaining();
            if remaining.is_zero() {
                authoritative_checks = authoritative_checks.saturating_add(1);
                let final_state = self.evaluate_transition_authoritatively(observation).await;
                let outcome =
                    terminal_transition_outcome(final_state).unwrap_or(TransitionOutcome::Timeout);
                return ActionObservationResult {
                    report: TransitionObservation::report(
                        outcome,
                        authoritative_checks,
                        event_wakeups,
                        invocation_wakeups,
                    ),
                    invocation_accepted,
                    rejection: None,
                };
            }

            tokio::select! {
                biased;
                joined = &mut *invocation, if invocation_pending => {
                    invocation_pending = false;
                    invocation_wakeups = invocation_wakeups.saturating_add(1);
                    match joined {
                        Ok(Ok(_)) => invocation_accepted = true,
                        Ok(Err(error)) => {
                            return ActionObservationResult {
                                report: TransitionObservation::report(
                                    TransitionOutcome::Unverifiable,
                                    authoritative_checks,
                                    event_wakeups,
                                    invocation_wakeups,
                                ),
                                invocation_accepted,
                                rejection: Some(error),
                            };
                        }
                        Err(_) => {
                            return ActionObservationResult {
                                report: TransitionObservation::report(
                                    TransitionOutcome::Unverifiable,
                                    authoritative_checks,
                                    event_wakeups,
                                    invocation_wakeups,
                                ),
                                invocation_accepted,
                                rejection: None,
                            };
                        }
                    }
                    authoritative_checks = authoritative_checks.saturating_add(1);
                    let evaluation = self.evaluate_transition_authoritatively(observation).await;
                    if let Some(outcome) = terminal_transition_outcome(evaluation) {
                        return ActionObservationResult {
                            report: TransitionObservation::report(
                                outcome,
                                authoritative_checks,
                                event_wakeups,
                                invocation_wakeups,
                            ),
                            invocation_accepted,
                            rejection: None,
                        };
                    }
                }
                delivery = self.event_subscription.recv() => {
                    let Some(delivery) = delivery else {
                        self.event_stream_available = false;
                        self.handle_event_stream_closed().await;
                        authoritative_checks = authoritative_checks.saturating_add(1);
                        let final_state = observation.evaluate(
                            &self.runtime,
                            &self.cache,
                            &self.scopes,
                        );
                        let outcome = terminal_transition_outcome(final_state)
                            .unwrap_or(TransitionOutcome::ApplicationGone);
                        return ActionObservationResult {
                            report: TransitionObservation::report(
                                outcome,
                                authoritative_checks,
                                event_wakeups,
                                invocation_wakeups,
                            ),
                            invocation_accepted,
                            rejection: None,
                        };
                    };
                    match delivery {
                        EventDelivery::Event(first) => {
                            event_wakeups = event_wakeups.saturating_add(1);
                            let batch_window = Duration::from_millis(40).min(observation.remaining());
                            if !batch_window.is_zero() {
                                tokio::time::sleep(batch_window).await;
                            }
                            let mut events = vec![first];
                            while let Ok(event) = self.event_subscription.try_recv() {
                                events.push(event);
                            }
                            if let Some(EventDelivery::ResyncRequired { dropped }) =
                                self.event_subscription.take_resync()
                            {
                                self.resynchronize_after_overflow(dropped).await;
                            } else {
                                self.apply_event_batch(events, None).await;
                            }
                        }
                        EventDelivery::ResyncRequired { dropped } => {
                            event_wakeups = event_wakeups.saturating_add(1);
                            self.resynchronize_after_overflow(dropped).await;
                        }
                    }
                    authoritative_checks = authoritative_checks.saturating_add(1);
                    let evaluation = self.evaluate_transition_authoritatively(observation).await;
                    if let Some(outcome) = terminal_transition_outcome(evaluation) {
                        return ActionObservationResult {
                            report: TransitionObservation::report(
                                outcome,
                                authoritative_checks,
                                event_wakeups,
                                invocation_wakeups,
                            ),
                            invocation_accepted,
                            rejection: None,
                        };
                    }
                }
                _ = tokio::time::sleep(remaining) => {
                    authoritative_checks = authoritative_checks.saturating_add(1);
                    let final_state = self.evaluate_transition_authoritatively(observation).await;
                    let outcome = terminal_transition_outcome(final_state)
                        .unwrap_or(TransitionOutcome::Timeout);
                    return ActionObservationResult {
                        report: TransitionObservation::report(
                            outcome,
                            authoritative_checks,
                            event_wakeups,
                            invocation_wakeups,
                        ),
                        invocation_accepted,
                        rejection: None,
                    };
                }
            }
        }
    }

    async fn evaluate_transition_authoritatively(
        &mut self,
        observation: &TransitionObservation,
    ) -> TransitionEvaluation {
        if let Some(evaluation) = observation.authority_evaluation(&self.runtime) {
            return evaluation;
        }
        // Event-applied cache state is never sufficient for confirmation.
        // Every decision below follows a fresh backend read at the condition's
        // narrowest safe refresh boundary.
        match observation.condition().refresh_kind() {
            ConditionRefresh::ExactNode => {
                let locator = observation
                    .condition()
                    .exact_locator()
                    .expect("exact-node transition condition has locator")
                    .clone();
                match self.backend.refresh_node(&locator, false).await {
                    Ok(node) => {
                        if self.cache.refresh_node(node).is_ok() {
                            self.rebuild_view_preserving_focus().await;
                        } else {
                            self.full_reload(None).await;
                        }
                    }
                    Err(_) => self.full_reload(None).await,
                }
            }
            ConditionRefresh::FullApplication => self.full_reload(None).await,
        }
        observation.evaluate(&self.runtime, &self.cache, &self.scopes)
    }

    async fn update_from_action_events(&mut self, success_status: String) {
        let first =
            match tokio::time::timeout(self.settle_delay, self.event_subscription.recv()).await {
                Ok(Some(EventDelivery::Event(event))) => event,
                Ok(Some(EventDelivery::ResyncRequired { dropped })) => {
                    self.resynchronize_after_overflow(dropped).await;
                    return;
                }
                _ => {
                    self.full_reload(Some(format!(
                        "Full refresh fallback: no related AT-SPI event after {success_status}"
                    )))
                    .await;
                    return;
                }
            };
        tokio::time::sleep(Duration::from_millis(40)).await;
        let mut events = vec![first];
        while let Ok(event) = self.event_subscription.try_recv() {
            events.push(event);
        }
        if let Some(EventDelivery::ResyncRequired { dropped }) =
            self.event_subscription.take_resync()
        {
            self.resynchronize_after_overflow(dropped).await;
            return;
        }
        self.apply_event_batch(events, Some(success_status)).await;
    }

    pub async fn next_event(&mut self) -> Option<EventDelivery> {
        if !self.event_stream_available {
            return std::future::pending().await;
        }
        let event = self.event_subscription.recv().await;
        if event.is_none() {
            self.event_stream_available = false;
        }
        event
    }

    pub async fn handle_event_stream_closed(&mut self) {
        if !self.application_available {
            return;
        }
        self.runtime.state = crate::runtime::SessionState::Degraded;
        self.backend_available = false;
        self.runtime.record_backend_loss();
        self.runtime.invalidate_application();
        self.status = crate::runtime::RuntimeError::BackendUnavailable.to_string();
        self.reconnect_backend().await;
    }

    async fn reconnect_backend(&mut self) {
        let timeout = self.backend.operation_timeout();
        for delay in [100_u64, 200, 400, 800] {
            self.runtime.record_backend_reconnect_attempt();
            tokio::time::sleep(Duration::from_millis(delay)).await;
            let Ok(backend) = AtspiBackend::connect(timeout).await else {
                continue;
            };
            let Ok(mut fresh) = Self::new(
                backend,
                self.app_selector.clone(),
                self.inspect_options,
                self.settle_delay,
                self.bootstrap_strategy,
                self.event_subscription.capacity(),
                self.presentation_mode,
                self.spatial_layout,
                (
                    self.viewport_width.saturating_add(2),
                    self.viewport_height.saturating_add(3),
                ),
                self.external_text_handler_available,
            )
            .await
            else {
                continue;
            };
            fresh.configure_modality_client(self.modality_socket.clone());
            let mut runtime = std::mem::take(&mut self.runtime);
            runtime.invalidate_application();
            runtime.open_application(fresh.application_locator.clone());
            runtime.record_backend_reconnect();
            fresh.runtime = runtime;
            fresh.backend_available = true;
            fresh.status =
                "Accessibility backend reconnected; opened a fresh application generation".into();
            *self = fresh;
            return;
        }
        self.event_stream_available = false;
        self.status =
            "Desktop accessibility service unavailable. Existing view is read-only. F5: retry; b: applications; d: diagnostics; q: quit."
                .into();
    }

    /// Cheap lifecycle check used by the terminal loop. It never walks an
    /// application tree; it only verifies that the selected AT-SPI root still
    /// exists in the desktop registry.
    pub async fn check_application_available(&mut self) {
        if !self.application_available {
            return;
        }
        let alive = match self.backend.applications().await {
            Ok(applications) => applications
                .iter()
                .any(|application| application.backend_locator == self.application_locator),
            // A transient registry failure is not evidence that the GUI has
            // exited.  The event stream and subsequent polls can still recover.
            Err(error) => {
                tracing::debug!(%error, "application liveness probe failed");
                return;
            }
        };
        if !alive {
            self.application_gone();
        }
    }

    pub async fn apply_external_delivery(&mut self, delivery: EventDelivery) {
        if !self.application_available {
            return;
        }
        let first = match delivery {
            EventDelivery::Event(event) => event,
            EventDelivery::ResyncRequired { dropped } => {
                self.resynchronize_after_overflow(dropped).await;
                return;
            }
        };
        tokio::time::sleep(Duration::from_millis(40)).await;
        let mut events = vec![first];
        while let Ok(event) = self.event_subscription.try_recv() {
            events.push(event);
        }
        if let Some(EventDelivery::ResyncRequired { dropped }) =
            self.event_subscription.take_resync()
        {
            self.resynchronize_after_overflow(dropped).await;
            return;
        }
        self.apply_event_batch(events, None).await;
    }

    async fn resynchronize_after_overflow(&mut self, dropped: u64) {
        // Discard the incomplete pre-overflow prefix. The full bootstrap below
        // becomes the new baseline; events arriving during it remain buffered.
        while self.event_subscription.try_recv().is_ok() {}
        self.status =
            format!("Event overflow detected ({dropped} dropped); resynchronizing semantic tree");
        self.full_reload(Some(format!(
            "Semantic tree resynchronized after event overflow ({dropped} dropped)"
        )))
        .await;
        if let Some(EventDelivery::ResyncRequired { dropped }) =
            self.event_subscription.take_resync()
        {
            // A distinct flood overlapped the resync. Coalesce it into one more
            // correctness baseline rather than replaying an incomplete suffix.
            while self.event_subscription.try_recv().is_ok() {}
            self.full_reload(Some(format!(
                "Semantic tree resynchronized after overlapping overflow ({dropped} dropped)"
            )))
            .await;
        } else {
            let mut events = Vec::new();
            while let Ok(event) = self.event_subscription.try_recv() {
                events.push(event);
            }
            if !events.is_empty() {
                self.apply_event_batch(
                    events,
                    Some("Replayed events received during resync".to_owned()),
                )
                .await;
            }
        }
    }

    async fn apply_event_batch(
        &mut self,
        mut events: Vec<NormalizedEvent>,
        success_status: Option<String>,
    ) {
        let started = Instant::now();
        if let Some(session) = self.edit_session.as_mut()
            && !session.commit_pending
            && events.iter().any(|event| {
                event_targets(event, &session.backend_locator) && event_is_text_value_change(event)
            })
        {
            session.mark_external_change();
        }
        if let Some(search) = self
            .content_view
            .as_mut()
            .and_then(|view| view.full_search.as_mut())
        {
            for event in &events {
                if let Some(source) = self.cache.runtime_id(event.source()) {
                    search.invalidate_source(source);
                    if source == search.root
                        && matches!(
                            event,
                            NormalizedEvent::ChildrenChanged { .. }
                                | NormalizedEvent::WindowDestroyed { .. }
                        )
                    {
                        search.cancel();
                    }
                }
            }
        }
        for event in &events {
            self.content.invalidate_event(&self.cache, event);
        }
        let raw_count = events.len();
        // Cache Add/Remove reports cache residency. A bootstrap (especially a
        // recursive walk) can itself populate the toolkit cache, so replaying
        // an Add for an object already present in our semantic baseline is a
        // no-op. Likewise, removing an object we never owned cannot stale us.
        events.retain(|event| event_requires_refresh(&self.cache, event));
        if events.is_empty() {
            if let Some(status) = success_status {
                self.status = format!(
                    "{status} — events={raw_count} cache-residency events already reflected; full_snapshots={}",
                    self.cache.full_snapshot_count()
                );
            }
            return;
        }
        let mut scopes = coalesce_dirty_scopes(&events);
        let has_structural_refresh = scopes
            .iter()
            .any(|scope| matches!(scope, DirtyScope::Subtree(_)));
        // A toolkit can emit state/property echoes for transient descendants
        // immediately before or after the parent's ChildrenChanged signal.
        // Refresh the structural baseline first so those node echoes are
        // interpreted against the current tree, not the tree from the start
        // of the burst.
        scopes.sort_by_key(|scope| match scope {
            DirtyScope::Subtree(_) => 0,
            DirtyScope::Node(_) => 1,
            DirtyScope::Application => 2,
        });
        let dirty_count = scopes.len();
        let mut refreshed_nodes = 0_usize;
        let mut reconciled = 0_usize;
        let mut reconciled_ids = Vec::new();
        let mut new_ids = 0_usize;
        let mut removed_ids = 0_usize;
        for scope in scopes {
            let result = match scope {
                DirtyScope::Node(locator) => match self.backend.refresh_node(&locator, false).await
                {
                    Ok(node) => {
                        refreshed_nodes += 1;
                        self.cache.refresh_node(node).map(|_| ())
                    }
                    Err(_error)
                        if has_structural_refresh && self.cache.runtime_id(&locator).is_none() =>
                    {
                        // The preceding subtree refresh proved that this
                        // transient source no longer belongs to the semantic
                        // tree. Its trailing state/property event is stale,
                        // not evidence that the whole application is corrupt.
                        Ok(())
                    }
                    Err(error) => {
                        self.full_reload(Some(format!(
                            "Full refresh fallback: node refresh failed: {error}"
                        )))
                        .await;
                        return;
                    }
                },
                DirtyScope::Subtree(locator) => {
                    match self
                        .backend
                        .refresh_subtree(&locator, self.inspect_options)
                        .await
                    {
                        Ok(node) => {
                            refreshed_nodes += semantic_node_count(&node);
                            self.cache.replace_subtree(&locator, node).map(|report| {
                                reconciled += report.locator_reconciled;
                                reconciled_ids.extend(report.reconciled_runtime_ids);
                                new_ids += report.new_runtime_ids;
                                removed_ids += report.removed_runtime_ids;
                            })
                        }
                        Err(error) => {
                            self.full_reload(Some(format!(
                                "Full refresh fallback: subtree refresh failed: {error}"
                            )))
                            .await;
                            return;
                        }
                    }
                }
                DirtyScope::Application => {
                    self.full_reload(Some("Full refresh fallback: application dirty".to_owned()))
                        .await;
                    return;
                }
            };
            if let Err(error) = result {
                self.full_reload(Some(format!(
                    "Full refresh fallback: cache invariant/update failed: {error}"
                )))
                .await;
                return;
            }
        }
        self.rebuild_view_preserving_focus().await;
        let elapsed = started.elapsed().as_millis();
        let cache_nodes = self.cache.node_count();
        let reconciled_detail = reconciled_ids
            .first()
            .map(|id| format!(" first_reconciled={id}"))
            .unwrap_or_default();
        self.status = format!(
            "{} — events={raw_count} dirty={dirty_count} refreshed={refreshed_nodes} nodes cache_nodes={cache_nodes} reconciled={reconciled}{reconciled_detail} new_ids={new_ids} removed_ids={removed_ids} update={elapsed} ms full_snapshots={}",
            success_status.unwrap_or_else(|| "Live update".to_owned()),
            self.cache.full_snapshot_count()
        );
    }

    async fn rebuild_view_preserving_focus(&mut self) {
        let materialization_started = Instant::now();
        let previous_scope = self.scopes.active();
        let previous_id = self.focus.current();
        let previous_runtime = previous_id
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref())
            .map(|binding| binding.runtime_id);
        let previous_locator = previous_id
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref())
            .map(|binding| binding.backend_locator.clone());
        if let (Some(scene_id), Some(runtime_id), Some(locator)) =
            (previous_id, previous_runtime, previous_locator.clone())
        {
            self.scope_focus_history.insert(
                previous_scope,
                FocusAnchor {
                    scene_id,
                    runtime_id,
                    locator,
                },
            );
        }
        if let Err(error) =
            enrich_relational_cache(&self.backend, &mut self.cache, self.presentation_mode).await
        {
            self.status = format!("Relation enrichment failed: {error}");
            return;
        }
        self.content.rebuild_semantics(&self.cache);
        qualify_complex_text_capabilities(&self.backend, &mut self.cache, &self.content).await;
        let (scene, scopes, commands, choices) = match build_contextual_view(
            &self.cache,
            self.presentation_mode,
            self.content.catalog(),
        ) {
            Ok(view) => view,
            Err(error) => {
                self.status = format!("Contextual scene rebuild failed: {error}");
                return;
            }
        };
        self.scene = scene;
        self.scopes = scopes;
        self.commands = commands;
        self.choices = choices;
        self.replan_spatial_layout();
        if self.edit_session.as_ref().is_some_and(|session| {
            self.cache
                .node(session.target)
                .is_none_or(|node| node.backend_locator != session.backend_locator)
        }) {
            self.edit_session = None;
            self.status = "Edited control was replaced; edit cancelled".to_owned();
        }
        tracing::debug!(
            cache_nodes = self.cache.node_count(),
            materialization_ms = materialization_started.elapsed().as_secs_f64() * 1000.0,
            "materialized semantic arena for TUI view"
        );
        let restore_anchor = if self.scopes.active() != previous_scope {
            self.scope_focus_history.get(&self.scopes.active())
        } else {
            None
        };
        let restore_runtime = restore_anchor
            .map(|anchor| anchor.runtime_id)
            .or(previous_runtime);
        let restore_locator = restore_anchor
            .map(|anchor| &anchor.locator)
            .or(previous_locator.as_ref());
        self.focus
            .reconcile_identity(&self.scene, restore_runtime, restore_locator);
        let restored_scene = self.focus.current();
        let restored_runtime = restored_scene
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref())
            .map(|binding| binding.runtime_id);
        tracing::debug!(
            previous_scope = %previous_scope,
            active_scope = %self.scopes.active(),
            previous_scene_id = ?previous_id.map(SceneElementId::get),
            previous_runtime_id = ?previous_runtime.map(RuntimeNodeId::get),
            history_scene_id = ?restore_anchor.map(|anchor| anchor.scene_id.get()),
            restored_scene_id = ?restored_scene.map(SceneElementId::get),
            restored_runtime_id = ?restored_runtime.map(RuntimeNodeId::get),
            "restored exact semantic focus after contextual scene rebuild"
        );
        self.ensure_focus_visible();
        if self.content_view.is_some() {
            let mut reader_fallback = false;
            let root_is_live = self
                .content_view
                .as_ref()
                .is_some_and(|view| self.content.model(view.root).is_some());
            if root_is_live {
                if let Some(view) = self.content_view.as_mut()
                    && let Some(model) = self.content.model(view.root)
                    && model.block(view.position).is_none()
                    && let Some(fallback) = model.reading_order().first().copied()
                {
                    view.position = fallback;
                    self.reader_stale_fallbacks += 1;
                    reader_fallback = true;
                }
                self.refresh_content_view().await;
                if reader_fallback {
                    self.status =
                        "Reader target disappeared; moved to first valid semantic block".into();
                }
            } else {
                self.content_view = None;
                self.status = "Document content disappeared; Reader closed".to_owned();
            }
        }
    }

    fn ensure_focus_visible(&mut self) {
        let Some(id) = self.focus.current() else {
            return;
        };
        if let Some((top, height)) = self.scene.row_span(id, self.viewport_width) {
            self.viewport
                .ensure_visible(top, height, self.viewport_height);
        }
    }

    async fn ensure_focused_relations(&mut self) {
        let Some(scene_id) = self.focus.current() else {
            return;
        };
        let Some(element) = self.scene.element(scene_id) else {
            return;
        };
        let schedule = schedule_on_demand_relations(&self.cache, element.sources.clone(), 8);
        if schedule.candidates.is_empty() {
            return;
        }
        let previous_runtime = element.binding.as_ref().map(|binding| binding.runtime_id);
        let previous_locator = element
            .binding
            .as_ref()
            .map(|binding| binding.backend_locator.clone());
        let candidates: Vec<_> = schedule
            .candidates
            .iter()
            .map(|candidate| candidate.runtime_id)
            .collect();
        let metrics = self
            .backend
            .enrich_relations(&mut self.cache, &candidates)
            .await;
        tracing::debug!(
            requested = candidates.len(),
            relation_rpcs = metrics.rpc_count,
            relations = metrics.relations_found,
            relation_ms = metrics.duration.as_secs_f64() * 1000.0,
            "on-demand relation enrichment for focused scene element"
        );
        if metrics.relations_found == 0 {
            return;
        }
        self.content.rebuild_semantics(&self.cache);
        let Ok((scene, scopes, commands, choices)) =
            build_contextual_view(&self.cache, self.presentation_mode, self.content.catalog())
        else {
            return;
        };
        self.scene = scene;
        self.scopes = scopes;
        self.commands = commands;
        self.choices = choices;
        self.replan_spatial_layout();
        self.focus
            .reconcile_identity(&self.scene, previous_runtime, previous_locator.as_ref());
        self.ensure_focus_visible();
    }

    fn report_unavailable(&mut self, scene_id: SceneElementId) {
        if let Some(element) = self.scene.element(scene_id) {
            self.status = format!(
                "No compatible semantic action for \"{}\"",
                element_label(element)
            );
        }
    }
}

fn compress_degradation_lines(lines: Vec<String>) -> Vec<String> {
    const UNAVAILABLE: &str =
        "[text unavailable through the application's accessibility interface]";
    let mut compressed = Vec::with_capacity(lines.len());
    let mut unavailable = 0_usize;
    let flush = |output: &mut Vec<String>, count: &mut usize| {
        if *count == 1 {
            output.push("[… unavailable …]".to_owned());
        } else if *count > 1 {
            output.push(format!("[… {} inaccessible semantic blocks …]", *count));
        }
        *count = 0;
    };
    for line in lines {
        if line.contains(UNAVAILABLE) {
            unavailable = unavailable.saturating_add(1);
        } else {
            flush(&mut compressed, &mut unavailable);
            compressed.push(line);
        }
    }
    flush(&mut compressed, &mut unavailable);
    compressed
}

fn inline_materialization_budget(
    viewport_height: u16,
    viewport_offset: u16,
) -> MaterializationBudget {
    let visible_rows = usize::from(viewport_height.max(1));
    let offset = usize::from(viewport_offset);
    let visible_blocks = visible_rows.saturating_add(offset).clamp(1, 128);
    let lookahead_blocks = visible_rows.div_ceil(3).clamp(1, 16);
    MaterializationBudget {
        visible_blocks,
        lookahead_blocks,
        paragraph_ranges_per_source: visible_blocks.saturating_add(lookahead_blocks).min(128),
    }
}

fn completed_search_status(
    progress: crate::content::SearchProgress,
    result_count: usize,
    cache_bytes: usize,
) -> String {
    let scope = if progress.total_blocks.is_some() {
        "Full search complete"
    } else {
        "Exposed semantic content exhausted (document coverage partial or unknown)"
    };
    format!(
        "{} — {} blocks, {} matches, {} text RPCs, cache={} bytes",
        scope, progress.scanned_blocks, result_count, progress.text_rpcs, cache_bytes
    )
}

impl Drop for TuiApplication {
    fn drop(&mut self) {
        self.modality_cancel.cancel();
        if let Some(task) = self.capture_task.take() {
            task.abort();
        }
        if let Some(task) = self.modality_task.take() {
            task.abort();
        }
    }
}

async fn load_snapshot(
    backend: &AtspiBackend,
    selector: &str,
    options: InspectOptions,
    strategy: BootstrapStrategy,
) -> Result<crate::backend::BootstrapResult, BackendError> {
    let applications = backend.applications().await?;
    let application = AtspiBackend::select_application(&applications, Some(selector), None)?;
    backend
        .bootstrap_application(application, options, strategy)
        .await
}

fn semantic_node_count(node: &crate::semantic::SemanticNode) -> usize {
    1 + node.children.iter().map(semantic_node_count).sum::<usize>()
}

fn event_requires_refresh(cache: &SemanticCache, event: &NormalizedEvent) -> bool {
    match event {
        NormalizedEvent::CacheAdded { locator } => cache.runtime_id(locator).is_none(),
        NormalizedEvent::CacheRemoved { locator } => cache.runtime_id(locator).is_some(),
        _ => true,
    }
}

fn event_targets(event: &NormalizedEvent, target: &crate::semantic::BackendLocator) -> bool {
    match event {
        NormalizedEvent::NodeStateChanged { locator, .. }
        | NormalizedEvent::NodePropertyChanged { locator, .. }
        | NormalizedEvent::TextChanged { locator, .. }
        | NormalizedEvent::WindowCreated { locator }
        | NormalizedEvent::WindowDestroyed { locator }
        | NormalizedEvent::CacheAdded { locator }
        | NormalizedEvent::CacheRemoved { locator }
        | NormalizedEvent::Unknown { locator, .. } => locator == target,
        NormalizedEvent::ChildrenChanged { parent, .. } => parent == target,
        NormalizedEvent::SelectionChanged { container }
        | NormalizedEvent::ActiveDescendantChanged {
            container,
            descendant: _,
        } => container == target,
    }
}

fn event_is_text_value_change(event: &NormalizedEvent) -> bool {
    match event {
        NormalizedEvent::TextChanged { .. } => true,
        NormalizedEvent::NodePropertyChanged { property, .. } => {
            let property = property.to_ascii_lowercase();
            property.contains("value") || property.contains("text")
        }
        _ => false,
    }
}

fn discard_target_echoes(
    events: &mut Vec<NormalizedEvent>,
    target: &crate::semantic::BackendLocator,
) {
    events.retain(|event| !event_targets(event, target));
}

fn application_is_gone(error: &BackendError) -> bool {
    matches!(
        error,
        BackendError::NoApplications
            | BackendError::ApplicationNotFound(_)
            | BackendError::ObjectUnavailable(_, _)
    )
}

fn intent_for_element(element: &SceneElement) -> UiIntent {
    if let Some(binding) = &element.binding {
        return binding.default_intent;
    }
    match element.capability() {
        InteractionCapability::Toggle => UiIntent::Toggle,
        InteractionCapability::Select => UiIntent::Select,
        InteractionCapability::Choose => UiIntent::BeginChoice,
        InteractionCapability::OpenMenu => UiIntent::OpenMenu,
        InteractionCapability::EditText => UiIntent::BeginEdit,
        InteractionCapability::AdjustValue => UiIntent::IncreaseValue,
        InteractionCapability::BrowseContent => UiIntent::BeginRead,
        InteractionCapability::Activate | InteractionCapability::None => UiIntent::Activate,
    }
}

fn operation_verb(intent: UiIntent) -> &'static str {
    match intent {
        UiIntent::Select => "Selected",
        UiIntent::BeginChoice => "Opened choice overlay",
        UiIntent::OpenMenu => "Opened menu",
        UiIntent::ClosePopup => "Closed popup",
        UiIntent::Toggle => "Toggled",
        UiIntent::BeginEdit
        | UiIntent::BeginExternalEdit
        | UiIntent::CommitEdit
        | UiIntent::CancelEdit => "Edited",
        UiIntent::IncreaseValue => "Increased",
        UiIntent::DecreaseValue => "Decreased",
        _ => "Activated",
    }
}

fn terminal_transition_outcome(evaluation: TransitionEvaluation) -> Option<TransitionOutcome> {
    match evaluation {
        TransitionEvaluation::Confirmed => Some(TransitionOutcome::Confirmed),
        TransitionEvaluation::Pending => None,
        TransitionEvaluation::Stale => Some(TransitionOutcome::Stale),
        TransitionEvaluation::ApplicationGone => Some(TransitionOutcome::ApplicationGone),
        TransitionEvaluation::Cancelled => Some(TransitionOutcome::Cancelled),
        TransitionEvaluation::Ambiguous => Some(TransitionOutcome::Ambiguous),
    }
}

fn transition_status(
    outcome: TransitionOutcome,
    label: &str,
    intent: UiIntent,
    description: &str,
) -> String {
    match outcome {
        TransitionOutcome::Confirmed => format!(
            "{} \"{label}\" via {description}; authoritative transition confirmed",
            operation_verb(intent)
        ),
        TransitionOutcome::Timeout => {
            format!("Action not confirmed for \"{label}\" before observation deadline")
        }
        TransitionOutcome::Stale => {
            format!("Target changed before \"{label}\" could be confirmed")
        }
        TransitionOutcome::ApplicationGone => {
            "Application changed; action outcome not confirmed".to_owned()
        }
        TransitionOutcome::Cancelled => "Action observation cancelled".to_owned(),
        TransitionOutcome::Ambiguous => {
            format!("Action outcome for \"{label}\" is semantically ambiguous")
        }
        TransitionOutcome::Unverifiable => format!(
            "Action invoked for \"{label}\"; no authoritative transition condition was available"
        ),
    }
}

fn describe_operation(intent: UiIntent, operation: &BackendOperation) -> String {
    match operation {
        BackendOperation::InvokeAction { action, .. } => action.name.clone(),
        BackendOperation::SelectChild { child_index, .. } => {
            debug_assert_eq!(intent, UiIntent::Select);
            format!("parent Selection child {child_index}")
        }
        BackendOperation::SetTextContents { .. } => "EditableText.SetTextContents".to_owned(),
        BackendOperation::SetComplexTextContents { .. } => {
            "EditableText.SetTextContents + complete Text read-back".to_owned()
        }
        BackendOperation::AdjustValue { increase, .. } => if *increase {
            "Value.increase"
        } else {
            "Value.decrease"
        }
        .to_owned(),
    }
}

fn operation_error_status(error: &BackendError) -> (String, bool) {
    match error {
        BackendError::SelectionRejected { .. } => {
            ("Selection was rejected by application".to_owned(), false)
        }
        BackendError::ObjectUnavailable(_, _) => (
            "Action failed: object became stale. Refreshing...".to_owned(),
            true,
        ),
        _ => (format!("Action failed: {error}"), false),
    }
}

fn value_operation_error_status(error: &BackendError) -> (String, bool) {
    match error {
        BackendError::ObjectUnavailable(_, _) => (
            "Value operation failed: control became stale. Refreshing...".to_owned(),
            true,
        ),
        BackendError::OperationTimeout { .. } | BackendError::DbusCall { .. } => (
            "Value outcome could not be verified; refreshing authoritative GUI state".to_owned(),
            true,
        ),
        BackendError::PermissionDenied { .. } => {
            ("Application rejected the Value operation".to_owned(), false)
        }
        BackendError::ValueUnsupported(_) | BackendError::ValueUnavailable(_) => (
            "Value control is no longer safely adjustable; showing it read-only".to_owned(),
            true,
        ),
        _ => (
            "Value operation was rejected by the application".to_owned(),
            false,
        ),
    }
}

fn element_label(element: &SceneElement) -> &str {
    element.label()
}

fn preserved_status(session: &mut ExternalTextSession, reason: &str) -> String {
    session.preserve().map_or_else(
        || format!("{reason}; candidate recovery artifact could not be retained"),
        |path| {
            format!(
                "{reason}; candidate preserved privately at {}",
                path.display()
            )
        },
    )
}

fn build_scene(root: &crate::semantic::SemanticNode, mode: PresentationMode) -> TuiScene {
    match mode {
        PresentationMode::Legacy => compile_legacy_scene(root),
        PresentationMode::Transcompiled => {
            let analysis = analyze_regions(root);
            compile_scene(root, &analysis)
        }
    }
}

async fn qualify_complex_text_capabilities(
    backend: &AtspiBackend,
    cache: &mut SemanticCache,
    content: &ContentRuntime,
) {
    let mut candidates = content
        .catalog()
        .visible_models()
        .filter(|model| model.completeness == ContentCompleteness::Complete)
        .map(|model| model.root)
        .filter(|root| content.text_capability(*root) == TextCapabilityStatus::Verified)
        .filter(|root| {
            cache.node(*root).is_some_and(|node| {
                node.role == SemanticRole::TextInput
                    && node.text_input_kind == Some(crate::semantic::TextInputKind::Plain)
                    && node.states.contains(&SemanticState::Editable)
                    && node.states.iter().any(|state| {
                        matches!(state, SemanticState::Other(value) if value == "multi-line")
                    })
                    && node.debug.interfaces.iter().any(|item| item == "Text")
                    && node.debug.interfaces.iter().any(|item| item == "EditableText")
                    && node.children.is_empty()
            })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.truncate(8);
    for runtime_id in candidates {
        let Some(locator) = cache
            .node(runtime_id)
            .map(|node| node.backend_locator.clone())
        else {
            continue;
        };
        if backend
            .read_complete_plain_multiline_text(&locator)
            .await
            .is_err()
        {
            continue;
        }
        let Ok(mut node) = backend.refresh_node(&locator, false).await else {
            continue;
        };
        if !node
            .capabilities
            .contains(&SemanticCapability::EditComplexText)
        {
            node.capabilities.push(SemanticCapability::EditComplexText);
        }
        let _ = cache.refresh_node(node);
    }
}

fn default_active_region(layout: &LayoutAnalysis) -> Option<SpatialRegionId> {
    layout
        .plan
        .regions
        .iter()
        .find(|region| region.kind == crate::transcompile::SpatialRegionKind::PrimaryContent)
        .or_else(|| {
            layout
                .plan
                .regions
                .iter()
                .filter(|region| {
                    region.obligation == crate::transcompile::PresentationObligation::Persistent
                        && region.demand != crate::transcompile::LayoutDemand::Hidden
                })
                .max_by_key(|region| default_surface_rank(region))
        })
        .or_else(|| {
            layout
                .plan
                .regions
                .iter()
                .filter(|region| {
                    region.demand != crate::transcompile::LayoutDemand::Hidden
                        && region.presentation.meaningful_items > 0
                })
                .max_by_key(|region| default_surface_rank(region))
        })
        .map(|region| region.id)
}

fn default_surface_rank(region: &SpatialRegion) -> (u8, u8, bool, usize) {
    let task = default_surface_task_rank(region.presentation.kind);
    let demand = match region.demand {
        crate::transcompile::LayoutDemand::Expand => 5,
        crate::transcompile::LayoutDemand::Supporting => 4,
        crate::transcompile::LayoutDemand::Compact => 3,
        crate::transcompile::LayoutDemand::Minimal => 2,
        crate::transcompile::LayoutDemand::Hidden => 0,
    };
    (
        task,
        demand,
        region.bounds.is_some(),
        region.presentation.meaningful_items.min(8),
    )
}

fn default_surface_task_rank(kind: RegionPresentationKind) -> u8 {
    match kind {
        RegionPresentationKind::InlineContent => 6,
        RegionPresentationKind::GraphicalPlaceholder => 5,
        RegionPresentationKind::Form
        | RegionPresentationKind::Table
        | RegionPresentationKind::WorkspacePane => 4,
        RegionPresentationKind::InputSurface
        | RegionPresentationKind::Navigation
        | RegionPresentationKind::ChoiceList => 3,
        RegionPresentationKind::ControlGroup => 2,
        RegionPresentationKind::CommandBar
        | RegionPresentationKind::Status
        | RegionPresentationKind::CollapsedSummary => 1,
        RegionPresentationKind::Structural
        | RegionPresentationKind::DiagnosticOnly
        | RegionPresentationKind::Empty => 0,
    }
}

async fn build_spatial_layout(
    backend: &AtspiBackend,
    cache: &crate::semantic::SemanticCache,
    content: &ContentRuntime,
    generation: Option<crate::runtime::ApplicationGenerationId>,
) -> Option<(LayoutAnalysis, SpatialEvidenceIndex)> {
    let generation = generation?;
    let tree = cache.materialize_tree().ok()?;
    let graph = crate::semantic::RelationalSemanticGraph::new(cache);
    let analysis = analyze_regions_with_graph(&tree, &graph);
    let evidence = SpatialEvidenceIndex::from_backend(
        &tree,
        generation,
        SpatialProbeBudget::default(),
        backend,
    )
    .await;
    let presentation = RegionPresentationContext::from_content_runtime(content);
    let layout = infer_layout_with_presentations(&analysis, &tree, &evidence, Some(&presentation));
    Some((layout, evidence))
}

async fn enrich_relational_cache(
    backend: &AtspiBackend,
    cache: &mut SemanticCache,
    mode: PresentationMode,
) -> Result<(), BackendError> {
    let tree = cache
        .materialize_tree()
        .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
    let preliminary = build_scene(&tree, mode);
    let provisional_graph = crate::semantic::RelationalSemanticGraph::new(cache);
    let provisional_scopes = InteractionScopes::analyze(cache, &provisional_graph);
    let focused = cache
        .nodes()
        .find(|node| node.states.contains(&SemanticState::Focused))
        .map(|node| node.runtime_id);
    let active_scope: std::collections::HashSet<_> = cache
        .nodes()
        .filter(|node| {
            provisional_scopes.scope_for_node(node.runtime_id) == Some(provisional_scopes.active())
        })
        .map(|node| node.runtime_id)
        .collect();
    let visible_scene = preliminary
        .elements
        .iter()
        .flat_map(|element| element.sources.iter().copied())
        .collect();
    let window_root = provisional_scopes
        .scope(provisional_scopes.active())
        .map(|scope| scope.root)
        .and_then(|mut id| {
            loop {
                let node = cache.node(id)?;
                if matches!(node.role, SemanticRole::Window | SemanticRole::Dialog) {
                    break Some(id);
                }
                id = node.parent?;
            }
        });
    let mut current_window = std::collections::HashSet::new();
    if let Some(root) = window_root {
        collect_cached_subtree_ids(cache, root, &mut current_window);
    }
    let budget = if cache.node_count() <= 512 {
        cache.node_count()
    } else {
        LARGE_TREE_RELATION_CANDIDATE_LIMIT
    };
    let schedule = schedule_relation_candidates(
        cache,
        &RelationPriorityContext {
            focused,
            active_scope,
            visible_scene,
            current_window,
        },
        budget,
    );
    let candidates: Vec<_> = schedule
        .candidates
        .iter()
        .map(|candidate| candidate.runtime_id)
        .collect();
    let metrics = backend.enrich_relations(cache, &candidates).await;
    tracing::debug!(
        budget = schedule.budget,
        deferred = schedule.deferred,
        focused =
            schedule
                .candidates
                .iter()
                .filter(|candidate| candidate.reason
                    == crate::semantic::RelationPriorityReason::Focused)
                .count(),
        active_scope = schedule
            .candidates
            .iter()
            .filter(|candidate| candidate.reason
                == crate::semantic::RelationPriorityReason::ActiveScope)
            .count(),
        visible_scene = schedule
            .candidates
            .iter()
            .filter(|candidate| candidate.reason
                == crate::semantic::RelationPriorityReason::VisibleScene)
            .count(),
        relation_sensitive = schedule
            .candidates
            .iter()
            .filter(|candidate| candidate.reason
                == crate::semantic::RelationPriorityReason::RelationSensitiveRole)
            .count(),
        current_window = schedule
            .candidates
            .iter()
            .filter(|candidate| candidate.reason
                == crate::semantic::RelationPriorityReason::CurrentWindow)
            .count(),
        background = schedule
            .candidates
            .iter()
            .filter(
                |candidate| candidate.reason == crate::semantic::RelationPriorityReason::Background
            )
            .count(),
        candidates = metrics.candidate_nodes,
        relation_rpcs = metrics.rpc_count,
        relations = metrics.relations_found,
        unresolved = metrics.unresolved_targets,
        unavailable = metrics.unavailable_nodes,
        relation_ms = metrics.duration.as_secs_f64() * 1000.0,
        "priority-driven semantic relation enrichment"
    );
    Ok(())
}

fn collect_cached_subtree_ids(
    cache: &SemanticCache,
    root: RuntimeNodeId,
    output: &mut std::collections::HashSet<RuntimeNodeId>,
) {
    if !output.insert(root) {
        return;
    }
    if let Some(node) = cache.node(root) {
        for child in &node.children {
            collect_cached_subtree_ids(cache, *child, output);
        }
    }
}

fn build_contextual_view(
    cache: &SemanticCache,
    mode: PresentationMode,
    content: &crate::content::ContentCatalog,
) -> Result<(TuiScene, InteractionScopes, CommandHierarchy, ChoiceCatalog), BackendError> {
    let tree = cache
        .materialize_tree()
        .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
    let graph = crate::semantic::RelationalSemanticGraph::new(cache);
    let scopes = InteractionScopes::analyze(cache, &graph);
    let choices = ChoiceCatalog::discover(cache);
    let mut scene = match mode {
        PresentationMode::Legacy => compile_legacy_scene(&tree),
        PresentationMode::Transcompiled => {
            let analysis = analyze_regions_with_graph(&tree, &graph);
            compile_scene(&tree, &analysis)
        }
    };
    let mut promoted_choice_options = std::collections::HashSet::new();
    for choice in choices.choices() {
        let option_ids: std::collections::HashSet<_> = choice
            .options
            .options()
            .iter()
            .map(|option| option.runtime_id)
            .collect();
        let candidate = scene.elements.iter().position(|element| {
            element
                .binding
                .as_ref()
                .is_some_and(|binding| binding.runtime_id == choice.owner)
                || (matches!(element.kind, SceneElementKind::Group { .. })
                    && (element.sources.contains(&choice.owner)
                        || (!option_ids.is_empty()
                            && option_ids.iter().all(|id| element.sources.contains(id)))))
        });
        let Some(index) = candidate else { continue };
        let Some(owner) = cache.node(choice.owner) else {
            continue;
        };
        let existing_label = match &scene.elements[index].kind {
            SceneElementKind::Group { label } | SceneElementKind::Selector { label } => {
                Some(label.clone())
            }
            _ => None,
        };
        let owner_label = existing_label
            .or_else(|| owner.name.clone())
            .unwrap_or_else(|| "Choice".to_owned());
        let label = choice_scene_label(choice, &owner_label);
        scene.elements[index].kind = SceneElementKind::Selector { label };
        scene.elements[index].binding = Some(SceneBinding {
            runtime_id: choice.owner,
            backend_locator: owner.backend_locator.clone(),
            semantic_role: owner.role.clone(),
            actions: owner.actions.clone(),
            capability: if choice.is_interactive() {
                InteractionCapability::Choose
            } else {
                InteractionCapability::None
            },
            default_intent: UiIntent::BeginChoice,
        });
        promoted_choice_options.extend(option_ids);
    }
    scene.elements.retain(|element| {
        element.binding.as_ref().is_none_or(|binding| {
            binding.capability == InteractionCapability::Choose
                || !promoted_choice_options.contains(&binding.runtime_id)
        })
    });
    for element in &mut scene.elements {
        if let Some(binding) = &element.binding
            && !scopes.allows_node(binding.runtime_id)
        {
            element.binding = None;
        }
    }
    compress_content_scene(&mut scene, cache, content);
    let commands = CommandHierarchy::build(cache, &scopes);
    Ok((scene, scopes, commands, choices))
}

fn choice_scene_label(choice: &crate::transcompile::SemanticChoice, owner_label: &str) -> String {
    let current = choice.current.and_then(|id| {
        choice
            .options
            .options()
            .iter()
            .find(|option| option.runtime_id == id)
            .map(|option| option.label.clone())
    });
    match (&choice.options, current) {
        (ChoiceOptions::Unavailable, _) => format!("{owner_label} (options unavailable)"),
        (_, Some(value))
            if choice
                .options
                .options()
                .iter()
                .any(|option| option.label == owner_label)
                || owner_label.ends_with(&format!(": {value}")) =>
        {
            format!("Choice: {value}")
        }
        (_, Some(value)) => format!("{owner_label}: {value}"),
        _ => format!("{owner_label} (current value unavailable)"),
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, CollectionCompleteness, RuntimeNodeId, SemanticAction, SemanticNode,
        SemanticRole,
    };
    use crate::transcompile::{
        ChoiceOption, ChoiceOptions, DisclosureRequirement, DismissBehavior, PresentationStrategy,
        SceneBinding, SemanticChoice,
    };

    use super::*;

    fn element(
        semantic_role: SemanticRole,
        kind: SceneElementKind,
        capability: InteractionCapability,
    ) -> SceneElement {
        let default_intent = match capability {
            InteractionCapability::Toggle => UiIntent::Toggle,
            InteractionCapability::Select => UiIntent::Select,
            InteractionCapability::Choose => UiIntent::BeginChoice,
            InteractionCapability::OpenMenu => UiIntent::OpenMenu,
            InteractionCapability::EditText => UiIntent::BeginEdit,
            _ => UiIntent::Activate,
        };
        SceneElement {
            id: SceneElementId::new(1),
            kind,
            sources: vec![RuntimeNodeId::new(1)],
            binding: Some(SceneBinding {
                runtime_id: RuntimeNodeId::new(1),
                backend_locator: BackendLocator::new(":1.2", "/node"),
                semantic_role,
                actions: vec![SemanticAction {
                    index: 0,
                    name: "Click".to_owned(),
                    description: None,
                    keybinding: None,
                }],
                capability,
                default_intent,
            }),
            strategy: PresentationStrategy::DirectWidget,
        }
    }

    #[test]
    fn default_region_selection_prioritizes_task_surface_over_control_density() {
        assert!(
            default_surface_task_rank(RegionPresentationKind::GraphicalPlaceholder)
                > default_surface_task_rank(RegionPresentationKind::ControlGroup)
        );
        assert!(
            default_surface_task_rank(RegionPresentationKind::Form)
                > default_surface_task_rank(RegionPresentationKind::CommandBar)
        );
    }

    #[test]
    fn mouse_activation_uses_toggle_intent_for_toggle_controls() {
        assert_eq!(
            intent_for_element(&element(
                SemanticRole::CheckBox,
                SceneElementKind::Checkbox {
                    label: "Enabled".to_owned(),
                    checked: false,
                },
                InteractionCapability::Toggle,
            )),
            UiIntent::Toggle
        );
        assert_eq!(
            intent_for_element(&element(
                SemanticRole::Button,
                SceneElementKind::Button {
                    label: "Apply".to_owned(),
                },
                InteractionCapability::Activate,
            )),
            UiIntent::Activate
        );
        assert_eq!(
            intent_for_element(&element(
                SemanticRole::TextInput,
                SceneElementKind::Field {
                    label: "Username".to_owned(),
                    display: "alice".to_owned(),
                    input_kind: crate::semantic::TextInputKind::Plain,
                },
                InteractionCapability::EditText,
            )),
            UiIntent::BeginEdit
        );
    }

    #[test]
    fn rejected_parent_selection_is_reported_without_local_refresh_or_state_change() {
        let error = BackendError::SelectionRejected {
            node_id: "container".to_owned(),
            index: 3,
        };
        assert_eq!(
            operation_error_status(&error),
            ("Selection was rejected by application".to_owned(), false)
        );
    }

    #[test]
    fn text_commit_discards_target_echoes_including_unknown_legacy_signals() {
        let target = BackendLocator::new(":1.2", "/input");
        let other = BackendLocator::new(":1.2", "/status");
        let mut events = vec![
            NormalizedEvent::TextChanged {
                locator: target.clone(),
                change: "insert".to_owned(),
                start: 0,
                length: 4,
            },
            NormalizedEvent::Unknown {
                locator: target.clone(),
                interface: "org.a11y.atspi.Event.Object".to_owned(),
                member: "PropertyChange".to_owned(),
            },
            NormalizedEvent::NodePropertyChanged {
                locator: other.clone(),
                property: "accessible-name".to_owned(),
            },
        ];

        discard_target_echoes(&mut events, &target);

        assert_eq!(events.len(), 1);
        assert!(event_targets(&events[0], &other));
    }

    #[test]
    fn unrelated_property_change_is_not_an_edit_conflict() {
        let locator = BackendLocator::new(":1.2", "/input");
        assert!(!event_is_text_value_change(
            &NormalizedEvent::NodePropertyChanged {
                locator: locator.clone(),
                property: "accessible-name".to_owned(),
            }
        ));
        assert!(event_is_text_value_change(
            &NormalizedEvent::NodePropertyChanged {
                locator,
                property: "accessible-value".to_owned(),
            }
        ));
    }

    #[test]
    fn bootstrap_cache_residency_events_do_not_force_a_second_snapshot() {
        let root = SemanticNode {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/root"),
            index_in_parent: None,
            role: SemanticRole::Application,
            name: Some("root".to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: crate::semantic::DebugInfo::default(),
        };
        let cache = SemanticCache::from_snapshot(root).unwrap();
        assert!(!event_requires_refresh(
            &cache,
            &NormalizedEvent::CacheAdded {
                locator: BackendLocator::new(":1.2", "/root")
            }
        ));
        assert!(!event_requires_refresh(
            &cache,
            &NormalizedEvent::CacheRemoved {
                locator: BackendLocator::new(":1.2", "/not-cached")
            }
        ));
        assert!(event_requires_refresh(
            &cache,
            &NormalizedEvent::CacheAdded {
                locator: BackendLocator::new(":1.2", "/new")
            }
        ));
    }

    #[test]
    fn choice_scene_label_tracks_gui_confirmation_without_using_popup_lifecycle() {
        let choice = SemanticChoice {
            owner: RuntimeNodeId::new(1),
            current: Some(RuntimeNodeId::new(3)),
            options: ChoiceOptions::Available(vec![
                ChoiceOption {
                    runtime_id: RuntimeNodeId::new(2),
                    label: "Alpha".to_owned(),
                    selected: false,
                    enabled: true,
                    selection: None,
                },
                ChoiceOption {
                    runtime_id: RuntimeNodeId::new(3),
                    label: "Beta".to_owned(),
                    selected: true,
                    enabled: true,
                    selection: None,
                },
            ]),
            disclosure: DisclosureRequirement::NotRequired,
            dismiss: DismissBehavior::NotApplicable,
            completeness: CollectionCompleteness::Complete,
        };
        assert_eq!(choice_scene_label(&choice, "Alpha"), "Choice: Beta");
        assert_eq!(choice_scene_label(&choice, "Theme"), "Theme: Beta");
    }

    #[test]
    fn completed_search_status_distinguishes_source_from_exposed_content() {
        let complete = completed_search_status(
            crate::content::SearchProgress {
                scanned_blocks: 17,
                total_blocks: Some(17),
                text_rpcs: 2,
            },
            4,
            100,
        );
        assert!(complete.starts_with("Full search complete"));

        let partial = completed_search_status(
            crate::content::SearchProgress {
                scanned_blocks: 17,
                total_blocks: None,
                text_rpcs: 0,
            },
            4,
            100,
        );
        assert!(partial.starts_with("Exposed semantic content exhausted"));
        assert!(partial.contains("coverage partial or unknown"));
        assert!(!partial.contains("Full search complete"));
    }

    #[test]
    fn adjacent_content_gaps_are_compacted_without_claiming_complete_coverage() {
        let unavailable = "[text unavailable through the application's accessibility interface]";
        let compressed = compress_degradation_lines(vec![
            "Heading".into(),
            unavailable.into(),
            format!("• {unavailable}"),
            "Available paragraph".into(),
            unavailable.into(),
        ]);
        assert_eq!(
            compressed,
            vec![
                "Heading",
                "[… 2 inaccessible semantic blocks …]",
                "Available paragraph",
                "[… unavailable …]",
            ]
        );
    }

    #[test]
    fn inline_materialization_tracks_viewport_and_remains_bounded() {
        let small = inline_materialization_budget(8, 0);
        let large = inline_materialization_budget(30, 12);
        let capped = inline_materialization_budget(u16::MAX, u16::MAX);
        assert_eq!(small.visible_blocks, 8);
        assert!(large.visible_blocks > small.visible_blocks);
        assert_eq!(capped.visible_blocks, 128);
        assert!(capped.paragraph_ranges_per_source <= 128);
    }
}
