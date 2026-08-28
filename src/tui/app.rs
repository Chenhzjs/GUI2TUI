use std::time::{Duration, Instant};

use ratatui::Frame;

use crate::backend::{AtspiBackend, BackendError, InspectOptions};

use super::{
    action::{InteractionCapability, UiIntent},
    focus::{FocusModel, Viewport},
    hit_test::{HitInteraction, HitMap},
    input::MouseIntent,
    operation::{BackendOperation, SemanticOperation, resolve_backend_operation},
    renderer::{RenderContext, render},
    view_model::{TuiElement, TuiElementKind, TuiViewModel},
};

pub struct TuiApplication {
    backend: AtspiBackend,
    app_selector: String,
    inspect_options: InspectOptions,
    settle_delay: Duration,
    view: TuiViewModel,
    focus: FocusModel,
    viewport: Viewport,
    viewport_height: u16,
    hit_map: HitMap,
    status: String,
    application_available: bool,
}

impl TuiApplication {
    pub async fn new(
        backend: AtspiBackend,
        app_selector: String,
        inspect_options: InspectOptions,
        settle_delay: Duration,
    ) -> Result<Self, BackendError> {
        let started = Instant::now();
        let view = load_view(&backend, &app_selector, inspect_options).await?;
        let snapshot_ms = started.elapsed().as_millis();
        let mut focus = FocusModel::default();
        focus.reconcile(&view, None);
        Ok(Self {
            backend,
            app_selector,
            inspect_options,
            settle_delay,
            view,
            focus,
            viewport: Viewport::default(),
            viewport_height: 1,
            hit_map: HitMap::default(),
            status: format!("Ready — snapshot {snapshot_ms} ms; text input read-only"),
            application_available: true,
        })
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.viewport_height = frame.area().height.saturating_sub(3).max(1);
        let regions = render(
            frame,
            RenderContext {
                view: &self.view,
                focused: self.focus.current(),
                scroll_offset: self.viewport.offset,
                status: &self.status,
                application_available: self.application_available,
            },
        );
        self.hit_map.replace(regions);
    }

    pub async fn handle_intent(&mut self, intent: UiIntent) -> bool {
        match intent {
            UiIntent::Quit => return true,
            UiIntent::FocusNext => {
                self.focus.next(&self.view);
                self.ensure_focus_visible();
            }
            UiIntent::FocusPrevious => {
                self.focus.previous(&self.view);
                self.ensure_focus_visible();
            }
            UiIntent::Activate | UiIntent::Toggle | UiIntent::Select | UiIntent::OpenMenu => {
                self.execute_focused(intent).await
            }
            UiIntent::Refresh => {
                self.reload(Some("Refreshed semantic snapshot".to_owned()))
                    .await;
            }
            UiIntent::ScrollLines(delta) => {
                self.viewport
                    .scroll_lines(delta, self.view.content_height(), self.viewport_height)
            }
            UiIntent::ScrollPages(pages) => {
                self.viewport
                    .scroll_pages(pages, self.view.content_height(), self.viewport_height)
            }
        }
        false
    }

    pub async fn handle_mouse(&mut self, intent: MouseIntent) {
        match intent {
            MouseIntent::Scroll(delta) => {
                self.viewport
                    .scroll_lines(delta, self.view.content_height(), self.viewport_height);
            }
            MouseIntent::Click { x, y } => {
                let Some(region) = self.hit_map.hit(x, y) else {
                    return;
                };
                self.focus.set(&self.view, region.runtime_id);
                self.ensure_focus_visible();
                match region.interaction {
                    HitInteraction::Activate => {
                        let intent = self
                            .view
                            .element(region.runtime_id)
                            .map(intent_for_element)
                            .unwrap_or(UiIntent::Activate);
                        self.execute_focused(intent).await;
                    }
                    HitInteraction::Unavailable => self.report_unavailable(region.runtime_id),
                    HitInteraction::Focus => {}
                }
            }
        }
    }

    async fn execute_focused(&mut self, _requested_intent: UiIntent) {
        let Some(runtime_id) = self.focus.current() else {
            self.status = "No focusable control".to_owned();
            return;
        };
        let Some(element) = self.view.element(runtime_id).cloned() else {
            self.status = "Focused control disappeared; press r to refresh".to_owned();
            return;
        };
        if element.capability == InteractionCapability::None {
            self.status = format!(
                "No compatible semantic action for \"{}\"",
                element_label(&element)
            );
            return;
        }
        let intent = intent_for_element(&element);
        let semantic_operation = SemanticOperation::from_intent(runtime_id, intent)
            .expect("interaction capabilities only produce operation intents");
        let backend_operation = match resolve_backend_operation(&self.view, semantic_operation) {
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
        };
        match result {
            Ok(_) => {
                let status = format!(
                    "{} \"{}\" via {}",
                    operation_verb(intent),
                    element_label(&element),
                    operation_description
                );
                tokio::time::sleep(self.settle_delay).await;
                self.reload(Some(status)).await;
            }
            Err(error) => {
                let (status, refresh) = operation_error_status(&error);
                self.status = status;
                if refresh {
                    let status = self.status.clone();
                    self.reload(Some(status)).await;
                }
            }
        }
    }

    async fn reload(&mut self, success_status: Option<String>) {
        let previous_locator = self
            .focus
            .current()
            .and_then(|id| self.view.element(id))
            .map(|element| element.backend_locator.clone());
        let started = Instant::now();
        match load_view(&self.backend, &self.app_selector, self.inspect_options).await {
            Ok(view) => {
                let snapshot_ms = started.elapsed().as_millis();
                self.view = view;
                self.focus.reconcile(&self.view, previous_locator.as_ref());
                self.application_available = true;
                self.status = format!(
                    "{} — snapshot {snapshot_ms} ms",
                    success_status.unwrap_or_else(|| "Refreshed".to_owned())
                );
                self.ensure_focus_visible();
            }
            Err(error) if application_is_gone(&error) => {
                self.application_available = false;
                self.status = "Application is no longer available. Press q to quit.".to_owned();
            }
            Err(error) => {
                self.status = format!("Refresh failed: {error}");
            }
        }
    }

    fn ensure_focus_visible(&mut self) {
        let Some(id) = self.focus.current() else {
            return;
        };
        if let Some((top, height)) = self.view.row_span(id) {
            self.viewport
                .ensure_visible(top, height, self.viewport_height);
        }
    }

    fn report_unavailable(&mut self, runtime_id: crate::semantic::RuntimeNodeId) {
        if let Some(element) = self.view.element(runtime_id) {
            self.status = format!(
                "No compatible semantic action for \"{}\"",
                element_label(element)
            );
        }
    }
}

async fn load_view(
    backend: &AtspiBackend,
    selector: &str,
    options: InspectOptions,
) -> Result<TuiViewModel, BackendError> {
    let applications = backend.applications().await?;
    let application = AtspiBackend::select_application(&applications, Some(selector), None)?;
    let snapshot = backend.inspect_application(application, options).await?;
    Ok(TuiViewModel::from_snapshot(&snapshot))
}

fn application_is_gone(error: &BackendError) -> bool {
    matches!(
        error,
        BackendError::NoApplications
            | BackendError::ApplicationNotFound(_)
            | BackendError::ObjectUnavailable(_, _)
    )
}

fn intent_for_element(element: &TuiElement) -> UiIntent {
    match element.capability {
        InteractionCapability::Toggle => UiIntent::Toggle,
        InteractionCapability::Select => UiIntent::Select,
        InteractionCapability::OpenMenu => UiIntent::OpenMenu,
        InteractionCapability::Activate | InteractionCapability::None => UiIntent::Activate,
    }
}

fn operation_verb(intent: UiIntent) -> &'static str {
    match intent {
        UiIntent::Select => "Selected",
        UiIntent::OpenMenu => "Opened menu",
        UiIntent::Toggle => "Toggled",
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

fn element_label(element: &TuiElement) -> &str {
    match &element.kind {
        TuiElementKind::Label { text } => text,
        TuiElementKind::Group { label }
        | TuiElementKind::Button { label }
        | TuiElementKind::ToggleButton { label, .. }
        | TuiElementKind::CheckBox { label, .. }
        | TuiElementKind::TextInput { label, .. }
        | TuiElementKind::List { label }
        | TuiElementKind::ListItem { label, .. }
        | TuiElementKind::Menu { label }
        | TuiElementKind::MenuItem { label, .. } => label,
        TuiElementKind::MenuBar => "Menu",
        TuiElementKind::Unsupported { role, .. } => role,
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, RuntimeNodeId, SemanticAction, SemanticRole};

    use super::*;

    fn element(
        semantic_role: SemanticRole,
        kind: TuiElementKind,
        capability: InteractionCapability,
    ) -> TuiElement {
        TuiElement {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/node"),
            semantic_role,
            kind,
            actions: vec![SemanticAction {
                index: 0,
                name: "Click".to_owned(),
                description: None,
                keybinding: None,
            }],
            capability,
        }
    }

    #[test]
    fn mouse_activation_uses_toggle_intent_for_toggle_controls() {
        assert_eq!(
            intent_for_element(&element(
                SemanticRole::CheckBox,
                TuiElementKind::CheckBox {
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
                TuiElementKind::Button {
                    label: "Apply".to_owned(),
                },
                InteractionCapability::Activate,
            )),
            UiIntent::Activate
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
}
