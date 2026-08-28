use std::time::{Duration, Instant};

use ratatui::Frame;

use crate::backend::{AtspiBackend, BackendError, InspectOptions};

use super::{
    action::{UiIntent, resolve_action},
    focus::{FocusModel, Viewport},
    hit_test::{HitInteraction, HitMap},
    input::MouseIntent,
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
            UiIntent::Activate | UiIntent::Toggle => self.activate_focused(intent).await,
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
                if region.interaction == HitInteraction::Activate {
                    let intent = self
                        .view
                        .element(region.runtime_id)
                        .map(intent_for_element)
                        .unwrap_or(UiIntent::Activate);
                    self.activate_focused(intent).await;
                }
            }
        }
    }

    async fn activate_focused(&mut self, intent: UiIntent) {
        let Some(runtime_id) = self.focus.current() else {
            self.status = "No focusable control".to_owned();
            return;
        };
        let Some(element) = self.view.element(runtime_id).cloned() else {
            self.status = "Focused control disappeared; press r to refresh".to_owned();
            return;
        };
        let action = match resolve_action(&element.actions, intent) {
            Ok(action) => action.clone(),
            Err(error) => {
                self.status = format!("Cannot activate {}: {error}", element_label(&element));
                return;
            }
        };

        let locator = element.backend_locator.encode();
        match self.backend.do_action(&locator, action.index).await {
            Ok(_) => {
                let status = format!(
                    "Activated \"{}\" via {}",
                    element_label(&element),
                    action.name
                );
                tokio::time::sleep(self.settle_delay).await;
                self.reload(Some(status)).await;
            }
            Err(error) => {
                let stale = matches!(error, BackendError::ObjectUnavailable(_, _));
                self.status = if stale {
                    "Action failed: object became stale. Refreshing...".to_owned()
                } else {
                    format!("Action failed: {error}")
                };
                if stale {
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
    if matches!(
        element.kind,
        TuiElementKind::CheckBox { .. } | TuiElementKind::ToggleButton { .. }
    ) {
        UiIntent::Toggle
    } else {
        UiIntent::Activate
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
        | TuiElementKind::ListItem { label } => label,
        TuiElementKind::Unsupported { role, .. } => role,
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{BackendLocator, RuntimeNodeId, SemanticAction};

    use super::*;

    fn element(kind: TuiElementKind) -> TuiElement {
        TuiElement {
            runtime_id: RuntimeNodeId::new(1),
            backend_locator: BackendLocator::new(":1.2", "/node"),
            kind,
            actions: vec![SemanticAction {
                index: 0,
                name: "Click".to_owned(),
                description: None,
                keybinding: None,
            }],
        }
    }

    #[test]
    fn mouse_activation_uses_toggle_intent_for_toggle_controls() {
        assert_eq!(
            intent_for_element(&element(TuiElementKind::CheckBox {
                label: "Enabled".to_owned(),
                checked: false,
            })),
            UiIntent::Toggle
        );
        assert_eq!(
            intent_for_element(&element(TuiElementKind::Button {
                label: "Apply".to_owned(),
            })),
            UiIntent::Activate
        );
    }
}
