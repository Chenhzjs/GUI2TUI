use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::{
    backend::{
        AtspiBackend, BackendError, BootstrapStrategy, EventDelivery, EventSubscription,
        InspectOptions,
    },
    events::{DirtyScope, NormalizedEvent, coalesce_dirty_scopes},
    semantic::SemanticCache,
    transcompile::{
        PresentationMode, SceneElement, SceneElementId, SceneElementKind, TuiScene,
        analyze_regions, compile_legacy_scene, compile_scene,
    },
};

use super::{
    action::{InteractionCapability, UiIntent},
    edit::{EditCommand, EditSession, key_to_edit_command},
    focus::{FocusModel, Viewport},
    hit_test::{HitInteraction, HitMap},
    input::{MouseIntent, key_to_intent},
    operation::{BackendOperation, SemanticOperation, resolve_backend_operation},
    palette::{CommandPalette, PaletteOutcome},
    renderer::{PaletteRender, RenderContext, render},
};

pub struct TuiApplication {
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
    scene: TuiScene,
    focus: FocusModel,
    viewport: Viewport,
    viewport_height: u16,
    viewport_width: u16,
    hit_map: HitMap,
    status: String,
    application_available: bool,
    edit_session: Option<EditSession>,
    command_palette: Option<CommandPalette>,
}

impl TuiApplication {
    pub async fn new(
        backend: AtspiBackend,
        app_selector: String,
        inspect_options: InspectOptions,
        settle_delay: Duration,
        bootstrap_strategy: BootstrapStrategy,
        event_buffer_capacity: usize,
        presentation_mode: PresentationMode,
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
        tokio::time::sleep(Duration::from_millis(50)).await;
        while event_subscription.try_recv().is_ok() {}
        let _ = event_subscription.take_resync();
        let bootstrap = backend
            .bootstrap_application(&application, inspect_options, bootstrap_strategy)
            .await?;
        let cache = SemanticCache::from_snapshot(bootstrap.root)
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let projected = cache
            .materialize_tree()
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let scene = build_scene(&projected, presentation_mode);
        let snapshot_ms = started.elapsed().as_millis();
        let mut focus = FocusModel::default();
        focus.reconcile(&scene, None);
        let mut application = Self {
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
            scene,
            focus,
            viewport: Viewport::default(),
            viewport_height: 1,
            viewport_width: 80,
            hit_map: HitMap::default(),
            status: format!(
                "Loaded {} semantic nodes via {} in {snapshot_ms} ms",
                bootstrap.metrics.node_count, bootstrap.metrics.strategy
            ),
            application_available: true,
            edit_session: None,
            command_palette: None,
        };
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

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.viewport_height = frame.area().height.saturating_sub(3).max(1);
        self.viewport_width = frame.area().width.saturating_sub(2).max(1);
        let palette = self.command_palette.as_ref().map(|palette| PaletteRender {
            query: palette.query(),
            entries: palette.entries(),
            selected: palette.selected(),
        });
        let regions = render(
            frame,
            RenderContext {
                scene: &self.scene,
                focused: self.focus.current(),
                scroll_offset: self.viewport.offset,
                status: &self.status,
                application_available: self.application_available,
                edit_session: self.edit_session.as_ref(),
                palette,
            },
        );
        self.hit_map.replace(regions);
    }

    pub async fn handle_intent(&mut self, intent: UiIntent) -> bool {
        match intent {
            UiIntent::Quit => return true,
            UiIntent::FocusNext => {
                self.focus.next(&self.scene);
                self.ensure_focus_visible();
            }
            UiIntent::FocusPrevious => {
                self.focus.previous(&self.scene);
                self.ensure_focus_visible();
            }
            UiIntent::Activate | UiIntent::Toggle | UiIntent::Select | UiIntent::OpenMenu => {
                self.execute_focused(intent).await
            }
            UiIntent::BeginEdit => self.begin_edit().await,
            UiIntent::CommitEdit => self.commit_edit().await,
            UiIntent::CancelEdit => self.cancel_edit(),
            UiIntent::OpenCommandPalette => {
                let palette = CommandPalette::from_scene(&self.scene);
                self.status = format!("Command palette — {} commands", palette.entries().len());
                self.command_palette = Some(palette);
            }
            UiIntent::Refresh => {
                self.full_reload(Some("Forced full semantic snapshot".to_owned()))
                    .await;
            }
            UiIntent::ScrollLines(delta) => self.viewport.scroll_lines(
                delta,
                self.scene.content_height(self.viewport_width),
                self.viewport_height,
            ),
            UiIntent::ScrollPages(pages) => self.viewport.scroll_pages(
                pages,
                self.scene.content_height(self.viewport_width),
                self.viewport_height,
            ),
        }
        false
    }

    pub async fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        if let Some(mut palette) = self.command_palette.take() {
            match palette.handle_key(key, &self.scene) {
                PaletteOutcome::Continue => self.command_palette = Some(palette),
                PaletteOutcome::Close => self.status = "Command palette closed".to_owned(),
                PaletteOutcome::Execute(scene_id) => {
                    self.focus.set(&self.scene, scene_id);
                    self.execute_focused(UiIntent::Activate).await;
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
        if let Some(mut intent) = key_to_intent(key) {
            if intent == UiIntent::Activate
                && self
                    .focus
                    .current()
                    .and_then(|id| self.scene.element(id))
                    .is_some_and(|element| matches!(element.kind, SceneElementKind::Field { .. }))
            {
                intent = UiIntent::BeginEdit;
            }
            self.handle_intent(intent).await
        } else {
            false
        }
    }

    pub async fn handle_mouse(&mut self, intent: MouseIntent) {
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

    async fn execute_focused(&mut self, _requested_intent: UiIntent) {
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
        let intent = intent_for_element(&element);
        let Some(runtime_id) = element.binding.as_ref().map(|binding| binding.runtime_id) else {
            self.status = "Scene element has no semantic binding".to_owned();
            return;
        };
        let semantic_operation = SemanticOperation::from_intent(runtime_id, intent)
            .expect("interaction capabilities only produce operation intents");
        let backend_operation = match resolve_backend_operation(&self.scene, semantic_operation) {
            Ok(operation) => operation,
            Err(error) => {
                self.status = format!("Cannot operate {}: {error}", element_label(&element));
                return;
            }
        };

        let operation_description = describe_operation(intent, &backend_operation);
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
        };
        match result {
            Ok(_) => {
                let status = format!(
                    "{} \"{}\" via {}",
                    operation_verb(intent),
                    element_label(&element),
                    operation_description
                );
                self.update_from_action_events(status).await;
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
                self.rebuild_view_preserving_focus();
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
        let previous_locator = self
            .focus
            .current()
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref())
            .map(|binding| binding.backend_locator.clone());
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
                let snapshot_ms = started.elapsed().as_millis();
                if let Err(error) = self.cache.full_refresh(bootstrap.root) {
                    self.status = format!("Full refresh fallback failed: {error}");
                    return;
                }
                let projected = match self.cache.materialize_tree() {
                    Ok(projected) => projected,
                    Err(error) => {
                        self.status = format!("Tree projection failed: {error}");
                        return;
                    }
                };
                self.scene = build_scene(&projected, self.presentation_mode);
                self.focus.reconcile(&self.scene, previous_locator.as_ref());
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
                self.application_available = false;
                let editing = self.edit_session.take().is_some();
                self.status = if editing {
                    "Application is no longer available. Edit discarded. Press q to quit."
                        .to_owned()
                } else {
                    "Application is no longer available. Press q to quit.".to_owned()
                };
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error}");
            }
        }
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
        self.full_reload(Some(
            "Full refresh fallback: AT-SPI event stream closed".to_owned(),
        ))
        .await;
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
            self.application_available = false;
            let editing = self.edit_session.take().is_some();
            self.status = if editing {
                "Application is no longer available. Edit discarded. Press q to quit.".to_owned()
            } else {
                "Application is no longer available. Press q to quit.".to_owned()
            };
        }
    }

    pub async fn apply_external_delivery(&mut self, delivery: EventDelivery) {
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
        let scopes = coalesce_dirty_scopes(&events);
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
        self.rebuild_view_preserving_focus();
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

    fn rebuild_view_preserving_focus(&mut self) {
        let materialization_started = Instant::now();
        let previous_id = self.focus.current();
        let previous_locator = previous_id
            .and_then(|id| self.scene.element(id))
            .and_then(|element| element.binding.as_ref())
            .map(|binding| binding.backend_locator.clone());
        let projected = match self.cache.materialize_tree() {
            Ok(projected) => projected,
            Err(error) => {
                self.status = format!("Tree projection failed: {error}");
                return;
            }
        };
        self.scene = build_scene(&projected, self.presentation_mode);
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
        if let Some(locator) = previous_locator.as_ref()
            && let Some(id) = self.scene.scene_id_for_locator(locator)
            && self
                .scene
                .element(id)
                .is_some_and(SceneElement::is_focusable)
        {
            self.focus.set(&self.scene, id);
        } else {
            self.focus.reconcile(&self.scene, previous_locator.as_ref());
        }
        self.ensure_focus_visible();
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

    fn report_unavailable(&mut self, scene_id: SceneElementId) {
        if let Some(element) = self.scene.element(scene_id) {
            self.status = format!(
                "No compatible semantic action for \"{}\"",
                element_label(element)
            );
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
        InteractionCapability::OpenMenu => UiIntent::OpenMenu,
        InteractionCapability::EditText => UiIntent::BeginEdit,
        InteractionCapability::Activate | InteractionCapability::None => UiIntent::Activate,
    }
}

fn operation_verb(intent: UiIntent) -> &'static str {
    match intent {
        UiIntent::Select => "Selected",
        UiIntent::OpenMenu => "Opened menu",
        UiIntent::Toggle => "Toggled",
        UiIntent::BeginEdit | UiIntent::CommitEdit | UiIntent::CancelEdit => "Edited",
        _ => "Activated",
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

fn element_label(element: &SceneElement) -> &str {
    element.label()
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

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, RuntimeNodeId, SemanticAction, SemanticNode, SemanticRole,
    };
    use crate::transcompile::{PresentationStrategy, SceneBinding};

    use super::*;

    fn element(
        semantic_role: SemanticRole,
        kind: SceneElementKind,
        capability: InteractionCapability,
    ) -> SceneElement {
        let default_intent = match capability {
            InteractionCapability::Toggle => UiIntent::Toggle,
            InteractionCapability::Select => UiIntent::Select,
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
}
