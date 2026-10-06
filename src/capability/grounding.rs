//! Description-to-command bindings. Evidence is not operation authority.
use super::*;
use crate::tui::action::{UiIntent, resolve_action};
use self_description::{DescriptionClaim, DescriptionOrigin, is_publicly_visible};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingDrop {
    UnresolvedExecutionTarget,
    UnresolvedExecutionContext,
    Disabled,
    OutOfScope,
    RiskBlocked,
    VerifierUnavailable,
    StaleBinding,
    NoRelevantEffect,
    UnsupportedDelivery,
}

/// These scopes describe routing, not permission. Only ExactTarget is currently
/// instantiated. Global/window shortcut routing remains deliberately absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionScope {
    ExactTarget,
    Window,
    Document,
    Application,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionContext {
    pub validity: ValidityScope,
    pub application: BackendLocator,
    pub window: BackendLocator,
    pub scope_locator: BackendLocator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactStateVerifier {
    pub state: SemanticState,
    pub before: bool,
    pub after: bool,
    pub effect: Effect,
    pub page_context: Option<BackendLocator>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptionBinding {
    pub claim: DescriptionClaim,
    pub execution_scope: ExecutionScope,
    pub context: Option<ExecutionContext>,
    pub target: Option<BackendLocator>,
    pub route: Option<CapabilityOperation>,
    /// Association only: never an authorized native route. A menu accelerator
    /// does not imply that its source can receive native focus.
    pub advertised_alternative: Option<KeyboardPrimitive>,
    pub evidence: &'static str,
    pub verifier: Option<ExactStateVerifier>,
    pub risk: RiskClass,
    pub drop_reason: Option<GroundingDrop>,
}

impl DescriptionBinding {
    pub fn executable(&self) -> bool {
        self.target.is_some()
            && self.route.is_some()
            && self.context.is_some()
            && !matches!(
                self.drop_reason,
                Some(
                    GroundingDrop::Disabled
                        | GroundingDrop::OutOfScope
                        | GroundingDrop::StaleBinding
                )
            )
    }

    /// Automatic research policy is stricter than route availability.
    pub fn probe_allowed(&self, disposable: bool) -> bool {
        disposable
            && self.executable()
            && self.drop_reason.is_none()
            && self.verifier.is_some()
            && matches!(
                self.risk,
                RiskClass::Observational | RiskClass::ReversibleNavigation
            )
    }
}

fn ancestry(
    objects: &RuntimeObjectModel,
    start: RuntimeNodeId,
    role: SemanticRole,
) -> Option<BackendLocator> {
    let mut current = Some(start);
    for _ in 0..64 {
        let object = objects.object(current?)?;
        if object.role == role
            || (role == SemanticRole::Window && object.role == SemanticRole::Dialog)
        {
            return Some(object.backend_locator.clone());
        }
        current = object.parent;
    }
    None
}

/// Existing public current-page contract: Selected, or a unique direct child's
/// name matching the list's current-page name AND that exact child Focused.
/// Names never choose or rebind a target; Focused alone is insufficient.
pub fn page_is_current(objects: &RuntimeObjectModel, id: RuntimeNodeId) -> bool {
    let Some(object) = objects.object(id) else {
        return false;
    };
    let Some(parent) = object.parent.and_then(|id| objects.object(id)) else {
        return false;
    };
    if object.role != SemanticRole::Tab || parent.role != SemanticRole::TabList {
        return false;
    }
    object.states.contains(&SemanticState::Selected)
        || (object.states.contains(&SemanticState::Focused)
            && object.name.as_ref().is_some_and(|n| !n.trim().is_empty())
            && object.name == parent.name
            && parent
                .children
                .iter()
                .filter_map(|id| objects.object(*id))
                .filter(|s| s.name == object.name)
                .count()
                == 1)
}

impl CapabilityGraph {
    /// One bounded binding per current claim, owned by the same graph. No name
    /// matching across objects and no shortcut-string search for a target.
    pub fn ground_descriptions(&self, scopes: &InteractionScopes) -> Vec<DescriptionBinding> {
        self.self_description.claims.iter().map(|claim| {
            let mut binding = DescriptionBinding {
                claim: claim.clone(), execution_scope: ExecutionScope::ExactTarget,
                context: None, target: None, route: None, advertised_alternative: None,
                evidence: "no public command ownership evidence", verifier: None,
                risk: claim.risk, drop_reason: Some(GroundingDrop::UnresolvedExecutionTarget),
            };
            let Some(object) = self.objects.object_by_locator(&claim.source) else { return binding; };
            let explicit = object.advertised_actions.iter().filter(|a| {
                claim.shortcut.primitive.is_some_and(|p| a.keybinding.as_deref().is_some_and(|s|
                    s.split(';').any(|part| parse_advertised_keybinding(part) == Some(p))))
            }).collect::<Vec<_>>();
            // A name on a command object may name that command. Descriptions on
            // arbitrary actionable objects can instead explain another command.
            let own_command_name = object.name.as_deref() == Some(claim.description.as_str())
                && matches!(object.role, SemanticRole::MenuItem | SemanticRole::Tab | SemanticRole::ListItem);
            let intent = if object.role == SemanticRole::Tab { UiIntent::SwitchPage }
                else if object.role == SemanticRole::ListItem { UiIntent::Select }
                else { UiIntent::Activate };
            let action = if explicit.len() == 1 && claim.origin == DescriptionOrigin::RuntimeDeclared {
                binding.advertised_alternative = claim.shortcut.primitive;
                binding.evidence = "exact action owns advertised binding; native routing still unqualified";
                Some(explicit[0])
            } else if own_command_name {
                binding.evidence = "command object owns name and compatible public semantic action; shortcut equivalence unverified";
                resolve_action(&object.role, &object.advertised_actions, intent).ok()
            } else { None };
            let Some(action) = action else { return binding; };
            binding.target = Some(object.backend_locator.clone());
            // A binding on an unnamed action proves ownership, not action-zero delivery.
            binding.route = (!action.name.trim().is_empty()).then(|| CapabilityOperation::AccessibilityInvoke(action.name.clone()));
            let context = (|| Some(ExecutionContext {
                validity: ValidityScope { session: self.session.clone(), generation: self.generation, interaction_scope: object.interaction_scope },
                application: ancestry(&self.objects, object.runtime_id, SemanticRole::Application)?,
                window: ancestry(&self.objects, object.runtime_id, SemanticRole::Window)?,
                scope_locator: scopes.scope(object.interaction_scope)?.backend_locator.clone(),
            }))();
            binding.context = context;
            // Narrow navigation verifier from role+public action+current state,
            // never from words such as "preview" or arbitrary window churn.
            if object.role == SemanticRole::Tab && resolve_action(&object.role, &object.advertised_actions, UiIntent::SwitchPage).is_ok_and(|a| a.name == action.name)
                && !page_is_current(&self.objects, object.runtime_id)
                && object.parent.and_then(|id| self.objects.object(id)).is_some_and(|p| p.role == SemanticRole::TabList
                    && !has_state(&p.states, "multiselectable")
                    && p.children.iter().filter(|id| page_is_current(&self.objects, **id)).count() == 1) {
                binding.verifier = Some(ExactStateVerifier { state: SemanticState::Selected, before: false, after: true, effect: Effect::SelectionChanged,
                    page_context: object.parent.and_then(|id|self.objects.object(id)).map(|p|p.backend_locator.clone()) });
                if binding.risk == RiskClass::Unknown { binding.risk = RiskClass::ReversibleNavigation; }
            }
            binding.drop_reason = if binding.context.is_none() { Some(GroundingDrop::UnresolvedExecutionContext) }
                else if !claim.enabled || !object.states.contains(&SemanticState::Enabled) || !is_publicly_visible(object) { Some(GroundingDrop::Disabled) }
                else if !scopes.allows_node(object.runtime_id) { Some(GroundingDrop::OutOfScope) }
                else if binding.route.is_none() { Some(GroundingDrop::UnsupportedDelivery) }
                else if self.rejected.iter().any(|r| r.target_locator == object.backend_locator
                    && Some(&r.operation) == binding.route.as_ref()
                    && self.context_signature(object.runtime_id).as_ref() == Some(&r.context)) { Some(GroundingDrop::NoRelevantEffect) }
                else if binding.risk == RiskClass::ExternalEffect || binding.risk == RiskClass::PotentiallyDestructive { Some(GroundingDrop::RiskBlocked) }
                else if binding.verifier.is_none() { Some(GroundingDrop::VerifierUnavailable) }
                else if !matches!(binding.risk, RiskClass::Observational | RiskClass::ReversibleNavigation) { Some(GroundingDrop::RiskBlocked) }
                else { None };
            binding
        }).collect()
    }

    /// Re-derive from current facts. Reusing a locator or text cannot preserve
    /// a binding across session/generation/scope/precondition changes.
    pub fn validate_description_binding(
        &self,
        binding: &DescriptionBinding,
        scopes: &InteractionScopes,
    ) -> bool {
        self.ground_descriptions(scopes)
            .iter()
            .any(|current| current == binding)
    }

    /// Readback accepts only the exact state transition on the bound object in
    /// the same application/window/scope. Dialogs elsewhere cannot satisfy it.
    pub fn verify_description_effect(
        &self,
        binding: &DescriptionBinding,
        scopes: &InteractionScopes,
    ) -> Effect {
        let (Some(context), Some(target), Some(verifier)) =
            (&binding.context, &binding.target, &binding.verifier)
        else {
            return Effect::UnknownEffect;
        };
        if context.validity.session != self.session
            || context.validity.generation != self.generation
            || !self.self_description.claims.contains(&binding.claim)
        {
            return Effect::NoRelevantEffect;
        }
        let Some(object) = self.objects.object_by_locator(target) else {
            return Effect::NoRelevantEffect;
        };
        if object.application_generation != context.validity.generation
            || !matches!(&binding.route, Some(CapabilityOperation::AccessibilityInvoke(name)) if object.advertised_actions.iter().any(|a| &a.name == name))
            || !scopes.allows_node(object.runtime_id)
            || scopes
                .scope(object.interaction_scope)
                .is_none_or(|s| s.backend_locator != context.scope_locator)
            || ancestry(&self.objects, object.runtime_id, SemanticRole::Application).as_ref()
                != Some(&context.application)
            || ancestry(&self.objects, object.runtime_id, SemanticRole::Window).as_ref()
                != Some(&context.window)
            || !is_publicly_visible(object)
            || verifier.before == verifier.after
        {
            return Effect::NoRelevantEffect;
        }
        let matches = if let Some(parent) = &verifier.page_context {
            object
                .parent
                .and_then(|id| self.objects.object(id))
                .is_some_and(|p| &p.backend_locator == parent)
                && page_is_current(&self.objects, object.runtime_id) == verifier.after
        } else {
            object.states.contains(&verifier.state) == verifier.after
        };
        if matches {
            verifier.effect
        } else {
            Effect::NoRelevantEffect
        }
    }
}

pub fn format_description_bindings(graph: &CapabilityGraph, scopes: &InteractionScopes) -> String {
    graph.ground_descriptions(scopes).iter().map(|b| format!(
        "Grounding source={} description={:?} origin={:?} status={:?} scope={:?} target={:?} route={:?} advertised_alternative={:?} risk={:?} verifier={:?} evidence={:?} executable={} drop={:?} historically_verified={}\n",
        b.claim.source, b.claim.description, b.claim.origin, b.claim.status, b.execution_scope,
        b.target.as_ref().map(BackendLocator::encode), b.route, b.advertised_alternative,
        b.risk, b.verifier, b.evidence, b.executable(), b.drop_reason,
        graph.observed_affordances().iter().any(|a| Some(&a.target_locator)==b.target.as_ref() && Some(&a.operation)==b.route.as_ref()),
    )).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{DebugInfo, RelationalSemanticGraph, SemanticNode};

    fn node(path: &str, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.8", path),
            index_in_parent: None,
            role,
            name: Some(name.into()),
            description: None,
            value: None,
            text_input_kind: None,
            states: vec![
                SemanticState::Enabled,
                SemanticState::Other("showing".into()),
            ],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        }
    }
    fn snapshot(selected: bool, churn: &str) -> (SemanticCache, InteractionScopes) {
        let mut app = node("/app", SemanticRole::Application, "Application");
        let mut window = node("/window", SemanticRole::Window, "Workspace");
        let mut tabs = node("/tabs", SemanticRole::TabList, "Views");
        for (path, name, is_selected, enabled) in [
            ("/old", "Overview", !selected, true),
            ("/new", "Outline Ctrl+K", selected, true),
            ("/disabled", "Other context Ctrl+K", false, false),
        ] {
            let mut tab = node(path, SemanticRole::Tab, name);
            tab.actions.push(SemanticAction {
                index: 7,
                name: "Press".into(),
                description: None,
                keybinding: None,
            });
            if is_selected {
                tab.states.push(SemanticState::Selected);
            }
            if !enabled {
                tab.states.retain(|s| *s != SemanticState::Enabled);
            }
            tabs.children.push(tab);
        }
        let mut command = node("/command", SemanticRole::MenuItem, "Export Ctrl+E");
        command.actions.push(SemanticAction {
            index: 9,
            name: "Press".into(),
            description: None,
            keybinding: None,
        });
        window.children.extend([
            tabs,
            command,
            node("/help", SemanticRole::Label, "Example Ctrl+K"),
            node("/churn", SemanticRole::Label, churn),
        ]);
        app.children.push(window);
        let cache = SemanticCache::from_snapshot(app).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        (cache, scopes)
    }
    #[test]
    fn description_ownership_disabled_duplicate_and_risk_gates() {
        let (cache, scopes) = snapshot(false, "0");
        let mut graph = CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &cache,
            &scopes,
        );
        graph.acquire_self_descriptions();
        let bindings = graph.ground_descriptions(&scopes);
        let get = |path| {
            bindings
                .iter()
                .find(|b| b.claim.source.object_path() == path)
                .unwrap()
        };
        assert!(get("/new").probe_allowed(true));
        assert!(!get("/new").probe_allowed(false));
        assert_eq!(get("/help").target, None); // same chord never joins objects
        assert_eq!(get("/disabled").drop_reason, Some(GroundingDrop::Disabled));
        assert!(!get("/disabled").executable());
        assert_eq!(
            get("/command").drop_reason,
            Some(GroundingDrop::RiskBlocked)
        );
        assert!(get("/command").executable()); // route != automatic permission
        assert!(!get("/command").probe_allowed(true));
        let id = graph
            .objects
            .object_by_locator(&BackendLocator::new(":1.8", "/command"))
            .unwrap()
            .runtime_id;
        let action = &mut graph
            .objects
            .objects
            .get_mut(&id)
            .unwrap()
            .advertised_actions[0];
        action.name.clear();
        action.keybinding = Some("<Alt>D;".into());
        graph.acquire_self_descriptions();
        let anonymous = graph
            .ground_descriptions(&scopes)
            .into_iter()
            .find(|b| b.claim.origin == DescriptionOrigin::RuntimeDeclared)
            .unwrap();
        assert!(anonymous.target.is_some());
        assert_eq!(anonymous.route, None);
        assert!(!anonymous.executable());
    }
    #[test]
    fn exact_verifier_rejects_churn_no_effect_and_stale_generation() {
        let (cache, scopes) = snapshot(false, "0");
        let mut graph = CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &cache,
            &scopes,
        );
        graph.acquire_self_descriptions();
        let b = graph
            .ground_descriptions(&scopes)
            .into_iter()
            .find(|b| b.probe_allowed(true))
            .unwrap();
        assert!(graph.validate_description_binding(&b, &scopes));
        assert_eq!(
            graph.verify_description_effect(&b, &scopes),
            Effect::NoRelevantEffect
        );
        let (churn, churn_scopes) = snapshot(false, "unrelated dialog and clock tick");
        graph.refresh(ApplicationGenerationId(1), &churn, &churn_scopes);
        assert_eq!(
            graph.verify_description_effect(&b, &churn_scopes),
            Effect::NoRelevantEffect
        );
        // Accepted delivery/no effect cannot create an observed capability.
        let target = graph
            .objects
            .object_by_locator(b.target.as_ref().unwrap())
            .unwrap()
            .runtime_id;
        assert!(
            graph
                .record_observation(target, b.route.clone().unwrap(), Effect::NoRelevantEffect)
                .is_none()
        );
        let (after, after_scopes) = snapshot(true, "tick");
        graph.refresh(ApplicationGenerationId(1), &after, &after_scopes);
        assert_eq!(
            graph.verify_description_effect(&b, &after_scopes),
            Effect::SelectionChanged
        );
        graph.refresh(ApplicationGenerationId(2), &after, &after_scopes);
        assert!(!graph.validate_description_binding(&b, &after_scopes));
        assert_eq!(
            graph.verify_description_effect(&b, &after_scopes),
            Effect::NoRelevantEffect
        );
        assert!(graph.observed_affordances().is_empty());
    }

    #[test]
    fn current_page_composite_requires_identity_and_unrelated_dialog_is_not_effect() {
        let (cache, scopes) = snapshot(false, "0");
        let mut graph = CapabilityGraph::new(
            RuntimeSessionId::default(),
            ApplicationGenerationId(1),
            &cache,
            &scopes,
        );
        graph.acquire_self_descriptions();
        let binding = graph
            .ground_descriptions(&scopes)
            .into_iter()
            .find(|b| b.probe_allowed(true))
            .unwrap();
        let mut root = cache.materialize_tree().unwrap();
        let mut foreign = node("/foreign-dialog", SemanticRole::Dialog, "Unrelated");
        foreign.backend_locator = BackendLocator::new(":1.99", "/dialog");
        foreign.states.push(SemanticState::Selected);
        root.children.push(foreign);
        let cache = SemanticCache::from_snapshot(root).unwrap();
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        graph.refresh(ApplicationGenerationId(1), &cache, &scopes);
        assert_eq!(
            graph.verify_description_effect(&binding, &scopes),
            Effect::NoRelevantEffect
        );
        let id = graph
            .objects
            .object_by_locator(binding.target.as_ref().unwrap())
            .unwrap()
            .runtime_id;
        graph
            .objects
            .objects
            .get_mut(&id)
            .unwrap()
            .states
            .push(SemanticState::Focused);
        assert!(!page_is_current(&graph.objects, id));
        let parent = graph.objects.object(id).unwrap().parent.unwrap();
        graph.objects.objects.get_mut(&parent).unwrap().name = Some("Outline Ctrl+K".into());
        assert!(page_is_current(&graph.objects, id));
        let disabled = graph
            .objects
            .object_by_locator(&BackendLocator::new(":1.8", "/disabled"))
            .unwrap()
            .runtime_id;
        graph.objects.objects.get_mut(&disabled).unwrap().name = Some("Outline Ctrl+K".into());
        assert!(!page_is_current(&graph.objects, id));
    }
}
