use std::time::{Duration, Instant};

use crate::{
    modality::CancellationToken,
    runtime::{ApplicationGenerationId, RuntimeSession, RuntimeSessionId},
    semantic::{BackendLocator, RuntimeNodeId, SemanticCache, SemanticRole, SemanticState},
    transcompile::{InteractionScopeId, InteractionScopeKind, InteractionScopes},
};

use super::action::UiIntent;

/// Exact authority captured immediately before one public semantic operation.
/// RuntimeNodeId may provide presentation continuity, but the locator checks
/// here prevent that continuity from authorizing a replacement object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OperationAuthority {
    session: RuntimeSessionId,
    generation: ApplicationGenerationId,
    application_locator: BackendLocator,
    target: RuntimeNodeId,
    target_locator: BackendLocator,
    scope: InteractionScopeId,
    scope_locator: BackendLocator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TransitionCondition {
    ExactNodeState {
        locator: BackendLocator,
        state: SemanticState,
        present: bool,
        refresh: ConditionRefresh,
    },
    ExactSurfaceUnavailable {
        locator: BackendLocator,
    },
    NewActiveModal {
        previous_scope_locator: BackendLocator,
    },
    ScopeInactive {
        scope_locator: BackendLocator,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConditionRefresh {
    ExactNode,
    FullApplication,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransitionEvaluation {
    Confirmed,
    Pending,
    Stale,
    ApplicationGone,
    Cancelled,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransitionOutcome {
    Confirmed,
    Timeout,
    Stale,
    ApplicationGone,
    Cancelled,
    Ambiguous,
    Unverifiable,
}

#[derive(Clone)]
pub(crate) struct TransitionObservation {
    authority: OperationAuthority,
    condition: TransitionCondition,
    deadline: Instant,
    cancellation: CancellationToken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TransitionReport {
    pub outcome: TransitionOutcome,
    pub authoritative_checks: u32,
    pub event_wakeups: u32,
    pub invocation_wakeups: u32,
}

impl OperationAuthority {
    pub fn capture(
        runtime: &RuntimeSession,
        application_locator: &BackendLocator,
        target: RuntimeNodeId,
        target_locator: &BackendLocator,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) -> Result<Self, TransitionOutcome> {
        let generation = runtime
            .generation()
            .ok_or(TransitionOutcome::ApplicationGone)?;
        if !runtime.validates_application(application_locator) {
            return Err(TransitionOutcome::Stale);
        }
        let node = cache.node(target).ok_or(TransitionOutcome::Stale)?;
        if node.backend_locator != *target_locator || !scopes.allows_node(target) {
            return Err(TransitionOutcome::Stale);
        }
        let scope = scopes
            .scope_for_node(target)
            .ok_or(TransitionOutcome::Ambiguous)?;
        let scope_locator = scopes
            .scope(scope)
            .map(|scope| scope.backend_locator.clone())
            .ok_or(TransitionOutcome::Ambiguous)?;
        Ok(Self {
            session: runtime.id.clone(),
            generation,
            application_locator: application_locator.clone(),
            target,
            target_locator: target_locator.clone(),
            scope,
            scope_locator,
        })
    }

    pub fn validate_before_invocation(
        &self,
        runtime: &RuntimeSession,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) -> Result<(), TransitionOutcome> {
        self.validate_runtime(runtime)?;
        let node = cache.node(self.target).ok_or(TransitionOutcome::Stale)?;
        if node.backend_locator != self.target_locator
            || scopes.scope_for_node(self.target) != Some(self.scope)
            || scopes
                .scope(self.scope)
                .is_none_or(|scope| scope.backend_locator != self.scope_locator)
            || !scopes.allows_node(self.target)
        {
            return Err(TransitionOutcome::Stale);
        }
        Ok(())
    }

    fn validate_runtime(&self, runtime: &RuntimeSession) -> Result<(), TransitionOutcome> {
        if runtime.id != self.session {
            return Err(TransitionOutcome::Stale);
        }
        match runtime.generation() {
            None => Err(TransitionOutcome::ApplicationGone),
            Some(generation) if generation != self.generation => Err(TransitionOutcome::Stale),
            Some(_) if !runtime.validates_application(&self.application_locator) => {
                Err(TransitionOutcome::Stale)
            }
            Some(_) => Ok(()),
        }
    }
}

impl TransitionCondition {
    pub fn for_action(
        intent: UiIntent,
        target: RuntimeNodeId,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) -> Option<Self> {
        let node = cache.node(target)?;
        match intent {
            UiIntent::Toggle
                if matches!(
                    node.role,
                    SemanticRole::CheckBox | SemanticRole::RadioButton | SemanticRole::ToggleButton
                ) =>
            {
                Some(Self::ExactNodeState {
                    locator: node.backend_locator.clone(),
                    state: SemanticState::Checked,
                    present: !node.states.contains(&SemanticState::Checked),
                    refresh: ConditionRefresh::ExactNode,
                })
            }
            UiIntent::OpenMenu => unique_menu_descendant(cache, target).and_then(|menu| {
                let menu = cache.node(menu)?;
                let showing = SemanticState::Other("showing".to_owned());
                (!menu.states.contains(&showing)).then(|| Self::ExactNodeState {
                    locator: menu.backend_locator.clone(),
                    state: showing,
                    present: true,
                    refresh: ConditionRefresh::FullApplication,
                })
            }),
            UiIntent::Activate => {
                if let Some(menu_locator) = showing_menu_ancestor(cache, target) {
                    return Some(Self::ExactSurfaceUnavailable {
                        locator: menu_locator,
                    });
                }
                let active = scopes.scope(scopes.active())?;
                if matches!(
                    active.kind,
                    InteractionScopeKind::ModalDialog
                        | InteractionScopeKind::Popup
                        | InteractionScopeKind::MenuPopup
                ) {
                    Some(Self::ScopeInactive {
                        scope_locator: active.backend_locator.clone(),
                    })
                } else {
                    Some(Self::NewActiveModal {
                        previous_scope_locator: active.backend_locator.clone(),
                    })
                }
            }
            _ => None,
        }
    }

    pub fn refresh_kind(&self) -> ConditionRefresh {
        match self {
            Self::ExactNodeState { refresh, .. } => *refresh,
            Self::ExactSurfaceUnavailable { .. }
            | Self::NewActiveModal { .. }
            | Self::ScopeInactive { .. } => ConditionRefresh::FullApplication,
        }
    }

    pub fn exact_locator(&self) -> Option<&BackendLocator> {
        match self {
            Self::ExactNodeState { locator, .. } => Some(locator),
            Self::ExactSurfaceUnavailable { .. }
            | Self::NewActiveModal { .. }
            | Self::ScopeInactive { .. } => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::ExactNodeState { .. } => "exact-node-state",
            Self::ExactSurfaceUnavailable { .. } => "exact-surface-unavailable",
            Self::NewActiveModal { .. } => "new-active-modal",
            Self::ScopeInactive { .. } => "scope-inactive",
        }
    }
}

impl TransitionObservation {
    pub fn new(
        authority: OperationAuthority,
        condition: TransitionCondition,
        bound: Duration,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            authority,
            condition,
            deadline: Instant::now() + bound,
            cancellation,
        }
    }

    pub fn condition(&self) -> &TransitionCondition {
        &self.condition
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn authority_evaluation(&self, runtime: &RuntimeSession) -> Option<TransitionEvaluation> {
        if let Err(outcome) = self.authority.validate_runtime(runtime) {
            return Some(match outcome {
                TransitionOutcome::ApplicationGone => TransitionEvaluation::ApplicationGone,
                _ => TransitionEvaluation::Stale,
            });
        }
        self.cancellation
            .is_cancelled()
            .then_some(TransitionEvaluation::Cancelled)
    }

    pub fn evaluate(
        &self,
        runtime: &RuntimeSession,
        cache: &SemanticCache,
        scopes: &InteractionScopes,
    ) -> TransitionEvaluation {
        if let Some(evaluation) = self.authority_evaluation(runtime) {
            return evaluation;
        }
        match &self.condition {
            TransitionCondition::ExactNodeState {
                locator,
                state,
                present,
                ..
            } => {
                let Some(id) = cache.runtime_id(locator) else {
                    return TransitionEvaluation::Stale;
                };
                let Some(node) = cache.node(id) else {
                    return TransitionEvaluation::Stale;
                };
                if node.states.contains(state) == *present {
                    TransitionEvaluation::Confirmed
                } else {
                    TransitionEvaluation::Pending
                }
            }
            TransitionCondition::ExactSurfaceUnavailable { locator } => {
                let Some(id) = cache.runtime_id(locator) else {
                    return TransitionEvaluation::Confirmed;
                };
                cache
                    .node(id)
                    .map_or(TransitionEvaluation::Confirmed, |node| {
                        if node.states.iter().any(
                            |state| matches!(state, SemanticState::Other(value) if value == "showing"),
                        ) {
                            TransitionEvaluation::Pending
                        } else {
                            TransitionEvaluation::Confirmed
                        }
                    })
            }
            TransitionCondition::NewActiveModal {
                previous_scope_locator,
            } => scopes
                .scope(scopes.active())
                .map_or(TransitionEvaluation::Ambiguous, |active| {
                    if active.kind == InteractionScopeKind::ModalDialog
                        && active.backend_locator != *previous_scope_locator
                    {
                        TransitionEvaluation::Confirmed
                    } else {
                        TransitionEvaluation::Pending
                    }
                }),
            TransitionCondition::ScopeInactive { scope_locator } => {
                let active_matches = scopes
                    .scope(scopes.active())
                    .is_some_and(|active| active.backend_locator == *scope_locator);
                let scope_still_present = scopes
                    .scopes()
                    .any(|scope| scope.backend_locator == *scope_locator);
                if !active_matches && !scope_still_present {
                    TransitionEvaluation::Confirmed
                } else {
                    TransitionEvaluation::Pending
                }
            }
        }
    }

    pub fn report(
        outcome: TransitionOutcome,
        authoritative_checks: u32,
        event_wakeups: u32,
        invocation_wakeups: u32,
    ) -> TransitionReport {
        TransitionReport {
            outcome,
            authoritative_checks,
            event_wakeups,
            invocation_wakeups,
        }
    }
}

fn unique_menu_descendant(cache: &SemanticCache, target: RuntimeNodeId) -> Option<RuntimeNodeId> {
    let mut pending = cache.node(target)?.children.clone();
    let mut menus = Vec::new();
    while let Some(id) = pending.pop() {
        let node = cache.node(id)?;
        if node.role == SemanticRole::Menu {
            menus.push(id);
        }
        pending.extend(node.children.iter().copied());
    }
    (menus.len() == 1).then_some(menus[0])
}

fn showing_menu_ancestor(cache: &SemanticCache, target: RuntimeNodeId) -> Option<BackendLocator> {
    let mut parent = cache.node(target)?.parent;
    while let Some(id) = parent {
        let node = cache.node(id)?;
        if node.role == SemanticRole::Menu
            && node
                .states
                .iter()
                .any(|state| matches!(state, SemanticState::Other(value) if value == "showing"))
        {
            return Some(node.backend_locator.clone());
        }
        parent = node.parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::semantic::{DebugInfo, SemanticNode, TextInputKind, TreeTruncation};

    use super::*;

    fn node(path: &str, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.2", path),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None::<TextInputKind>,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::<TreeTruncation>::new(),
            debug: DebugInfo::default(),
        }
    }

    fn tree(target_path: &str) -> SemanticNode {
        let mut app = node("/app", SemanticRole::Application, "App");
        let mut window = node("/window", SemanticRole::Window, "Main");
        window
            .states
            .push(SemanticState::Other("showing".to_owned()));
        window
            .children
            .push(node(target_path, SemanticRole::CheckBox, "Stable"));
        app.children.push(window);
        app
    }

    fn setup(target_path: &str) -> (RuntimeSession, SemanticCache, InteractionScopes) {
        let cache = SemanticCache::from_snapshot(tree(target_path)).unwrap();
        let scopes = InteractionScopes::analyze(
            &cache,
            &crate::semantic::RelationalSemanticGraph::new(&cache),
        );
        let mut runtime = RuntimeSession::default();
        runtime.open_application(BackendLocator::new(":1.2", "/app"));
        (runtime, cache, scopes)
    }

    #[test]
    fn preserved_runtime_id_never_transfers_old_locator_authority() {
        let (runtime, mut cache, scopes) = setup("/old");
        let old_locator = BackendLocator::new(":1.2", "/old");
        let id = cache.runtime_id(&old_locator).unwrap();
        let authority = OperationAuthority::capture(
            &runtime,
            &BackendLocator::new(":1.2", "/app"),
            id,
            &old_locator,
            &cache,
            &scopes,
        )
        .unwrap();

        cache
            .replace_subtree(&BackendLocator::new(":1.2", "/app"), tree("/new"))
            .unwrap();
        let rebuilt_scopes = InteractionScopes::analyze(
            &cache,
            &crate::semantic::RelationalSemanticGraph::new(&cache),
        );
        let new_locator = BackendLocator::new(":1.2", "/new");
        assert_eq!(cache.runtime_id(&new_locator), Some(id));
        assert_eq!(
            authority.validate_before_invocation(&runtime, &cache, &rebuilt_scopes),
            Err(TransitionOutcome::Stale)
        );
        assert!(
            OperationAuthority::capture(
                &runtime,
                &BackendLocator::new(":1.2", "/app"),
                id,
                &new_locator,
                &cache,
                &rebuilt_scopes,
            )
            .is_ok()
        );
    }

    #[test]
    fn unrelated_change_cannot_satisfy_an_exact_state_condition() {
        let (runtime, mut cache, scopes) = setup("/target");
        let target_locator = BackendLocator::new(":1.2", "/target");
        let target = cache.runtime_id(&target_locator).unwrap();
        let authority = OperationAuthority::capture(
            &runtime,
            &BackendLocator::new(":1.2", "/app"),
            target,
            &target_locator,
            &cache,
            &scopes,
        )
        .unwrap();
        let observation = TransitionObservation::new(
            authority,
            TransitionCondition::ExactNodeState {
                locator: target_locator.clone(),
                state: SemanticState::Checked,
                present: true,
                refresh: ConditionRefresh::ExactNode,
            },
            Duration::from_millis(10),
            CancellationToken::default(),
        );

        let mut unrelated = node("/target", SemanticRole::CheckBox, "Stable");
        unrelated.states.push(SemanticState::Focused);
        cache.refresh_node(unrelated).unwrap();
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Pending
        );

        let mut authoritative = node("/target", SemanticRole::CheckBox, "Stable");
        authoritative.states.push(SemanticState::Checked);
        cache.refresh_node(authoritative).unwrap();
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Confirmed
        );
    }

    #[test]
    fn no_event_fresh_state_confirms_while_deadline_never_creates_success() {
        let (runtime, mut cache, scopes) = setup("/target");
        let locator = BackendLocator::new(":1.2", "/target");
        let target = cache.runtime_id(&locator).unwrap();
        let authority = OperationAuthority::capture(
            &runtime,
            &BackendLocator::new(":1.2", "/app"),
            target,
            &locator,
            &cache,
            &scopes,
        )
        .unwrap();
        let observation = TransitionObservation::new(
            authority,
            TransitionCondition::ExactNodeState {
                locator: locator.clone(),
                state: SemanticState::Checked,
                present: true,
                refresh: ConditionRefresh::ExactNode,
            },
            Duration::from_millis(1),
            CancellationToken::default(),
        );

        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Pending
        );
        let timed_out = TransitionObservation::report(TransitionOutcome::Timeout, 1, 0, 0);
        assert_eq!(timed_out.outcome, TransitionOutcome::Timeout);

        let mut fresh = node("/target", SemanticRole::CheckBox, "Stable");
        fresh.states.push(SemanticState::Checked);
        cache.refresh_node(fresh).unwrap();
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Confirmed
        );
        let confirmed = TransitionObservation::report(TransitionOutcome::Confirmed, 2, 0, 0);
        assert_eq!(confirmed.event_wakeups, 0);
    }

    #[test]
    fn exact_temporary_surface_can_confirm_hidden_or_removed() {
        let mut app = node("/app", SemanticRole::Application, "App");
        let mut window = node("/window", SemanticRole::Window, "Main");
        window
            .states
            .push(SemanticState::Other("showing".to_owned()));
        let mut menu = node("/menu", SemanticRole::Menu, "Tools");
        menu.states.push(SemanticState::Other("showing".to_owned()));
        menu.children
            .push(node("/item", SemanticRole::MenuItem, "Activate"));
        window.children.push(menu);
        app.children.push(window);
        let mut cache = SemanticCache::from_snapshot(app).unwrap();
        let scopes = InteractionScopes::analyze(
            &cache,
            &crate::semantic::RelationalSemanticGraph::new(&cache),
        );
        let target = cache
            .runtime_id(&BackendLocator::new(":1.2", "/item"))
            .unwrap();
        let authority = OperationAuthority::capture(
            &RuntimeSession::default(),
            &BackendLocator::new(":1.2", "/app"),
            target,
            &BackendLocator::new(":1.2", "/item"),
            &cache,
            &scopes,
        );
        assert_eq!(authority, Err(TransitionOutcome::ApplicationGone));

        let mut runtime = RuntimeSession::default();
        runtime.open_application(BackendLocator::new(":1.2", "/app"));
        let authority = OperationAuthority::capture(
            &runtime,
            &BackendLocator::new(":1.2", "/app"),
            target,
            &BackendLocator::new(":1.2", "/item"),
            &cache,
            &scopes,
        )
        .unwrap();
        let observation = TransitionObservation::new(
            authority,
            TransitionCondition::ExactSurfaceUnavailable {
                locator: BackendLocator::new(":1.2", "/menu"),
            },
            Duration::from_millis(10),
            CancellationToken::default(),
        );
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Pending
        );

        let mut hidden = node("/menu", SemanticRole::Menu, "Tools");
        hidden.states.clear();
        cache.refresh_node(hidden).unwrap();
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Confirmed
        );

        cache
            .replace_subtree(
                &BackendLocator::new(":1.2", "/window"),
                node("/window", SemanticRole::Window, "Main"),
            )
            .unwrap();
        assert_eq!(
            observation.evaluate(&runtime, &cache, &scopes),
            TransitionEvaluation::Confirmed
        );
    }
}
