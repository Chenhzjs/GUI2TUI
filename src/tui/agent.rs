//! Native observation/dispatch boundary for external decision clients.
//! Model output is an observation-local operation ID, never an AT-SPI command.
use super::*;
use crate::semantic::CachedSemanticNode;
#[cfg(test)]
use crate::semantic::TextInputKind;
use crate::tui::palette::PublicOperation;
use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum AgentExecutionError {
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    Unknown(String),
}
impl From<String> for AgentExecutionError {
    fn from(reason: String) -> Self {
        Self::Rejected(reason)
    }
}
impl From<&str> for AgentExecutionError {
    fn from(reason: &str) -> Self {
        Self::Rejected(reason.to_owned())
    }
}
impl AgentExecutionError {
    pub fn delivery(&self) -> &'static str {
        match self {
            Self::Rejected(_) => "not_attempted",
            Self::Unknown(_) => "unknown",
        }
    }
}

pub struct AgentObservation {
    pub data: Value,
    candidates: Vec<AgentCandidate>,
    created: Instant,
}

#[derive(Clone)]
struct AgentCandidate {
    id: String,
    target: RuntimeNodeId,
    locator: BackendLocator,
    operation: PublicOperation,
    authority: OperationAuthority,
    precondition: Value,
}

fn validate_batch_requests(
    published: &[(String, PublicOperation)],
    requests: &[(String, Option<Value>)],
) -> Result<(), String> {
    if !(1..=4).contains(&requests.len()) {
        return Err("Batch requires 1..4 already-published operations".into());
    }
    let mut seen = std::collections::HashSet::new();
    for (id, parameter) in requests {
        if !seen.insert(id) {
            return Err("Duplicate batch operation ID".into());
        }
        let operation = published
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, operation)| operation)
            .ok_or("Batch operation was not published in this observation")?;
        validate_parameter(operation, parameter.as_ref()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn secret_node(cache: &SemanticCache, node: &CachedSemanticNode) -> bool {
    crate::transcompile::document::secret_source(cache, node.runtime_id)
}

fn node_data(cache: &SemanticCache, node: &CachedSemanticNode) -> Value {
    let secret = secret_node(cache, node);
    json!({
        "id": node.runtime_id.get(),
        "parent": node.parent.map(RuntimeNodeId::get),
        "role": node.role.to_string(),
        "label": if secret { None } else { node.name.as_deref() },
        "description": if secret { None } else { node.description.as_deref() },
        "value": if secret { None } else { node.value.as_deref() },
        "redacted": secret,
        "states": node.states.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>(),
        "partial": !node.truncations.is_empty(),
    })
}

/// Category-only diagnostics: never include source labels, text or secrets.
fn changed_precondition_fields(before: &Value, after: &Value) -> Vec<String> {
    let mut changed = Vec::new();
    if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
        for key in left
            .keys()
            .chain(right.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            if left.get(key) == right.get(key) {
                continue;
            }
            if key == "node" && left[key].is_object() && right[key].is_object() {
                let a = left[key].as_object().unwrap();
                let b = right[key].as_object().unwrap();
                for field in a
                    .keys()
                    .chain(b.keys())
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    if a.get(field) != b.get(field) {
                        changed.push(format!("node.{field}"));
                    }
                }
            } else {
                changed.push(key.clone());
            }
        }
    } else {
        changed.push("target_presence".into());
    }
    changed
}

fn public_text_data(mut data: Value, text: Result<String, String>) -> Value {
    if data["redacted"] == true {
        return data;
    }
    data["value_source"] = json!("public_text");
    match text {
        Ok(text) => {
            // Cache values are presentation previews, not the decision client's text.
            data["value"] = json!(text);
            data["value_complete"] = json!(true);
        }
        Err(error) => {
            data["value"] = Value::Null;
            data["value_complete"] = json!(false);
            data["value_error"] = json!(error);
            data["partial"] = json!(true);
        }
    }
    data
}

fn receipt_timings(
    mut receipt: Value,
    profile: bool,
    fresh_ms: f64,
    delivery_and_readback_ms: f64,
) -> Value {
    if profile {
        receipt["timings"] = json!({"fresh_validation_ms":fresh_ms,"delivery_and_readback_ms":delivery_and_readback_ms});
    }
    receipt
}

fn data_id_for_candidate(index: usize) -> String {
    format!("op-{index}")
}

fn operation_data(operation: &PublicOperation) -> Option<Value> {
    Some(match operation {
        PublicOperation::Action(name) => json!({"kind": "action", "name": name}),
        PublicOperation::EditText { multiline } => {
            json!({"kind": "replace_text", "multiline": multiline, "parameter": "text"})
        }
        PublicOperation::Selection { selected, .. } => {
            json!({"kind": "selection", "selected": selected})
        }
        PublicOperation::Value { increase } => {
            json!({"kind": "adjust_value", "increase": increase})
        }
        PublicOperation::SetValue => json!({"kind": "set_value", "parameter": "value"}),
        // Range editing and text selection are not implemented by this v1 boundary.
        PublicOperation::EditRange | PublicOperation::ReadSelection => return None,
    })
}

fn validate_parameter(
    operation: &PublicOperation,
    parameter: Option<&Value>,
) -> Result<(), AgentExecutionError> {
    match operation {
        PublicOperation::EditText { .. } => {
            let text = parameter
                .and_then(Value::as_str)
                .ok_or("Text parameter required")?;
            if text.len() > crate::backend::external_text_limit() {
                return Err("Text exceeds configured edit budget".into());
            }
        }
        PublicOperation::SetValue => {
            parameter
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .ok_or("Finite numeric parameter required")?;
        }
        _ if parameter.is_some() => return Err("Operation takes no parameter".into()),
        _ => {}
    }
    Ok(())
}

fn readback_confirms_request(readback: &Value) -> bool {
    match (
        readback.get("requested").and_then(Value::as_f64),
        readback.get("resulting").and_then(Value::as_f64),
    ) {
        (Some(requested), Some(resulting)) => requested == resulting,
        _ => true, // Text and Selection paths already require exact readback.
    }
}

fn operation_label(label: &str, target: RuntimeNodeId) -> (String, bool) {
    if label.chars().count() <= 192 {
        return (label.to_owned(), false);
    }
    // Only this display preview is bounded. Canonical node labels, the exact
    // advertised action and operation identity remain unchanged.
    (
        format!(
            "{}… [node #{}]",
            label.chars().take(170).collect::<String>(),
            target
        ),
        true,
    )
}

impl TuiApplication {
    fn agent_target_precondition(&self, target: RuntimeNodeId) -> Value {
        let Some(node) = self.cache.node(target) else {
            return Value::Null;
        };
        let mut ancestors = Vec::new();
        let mut parent = node.parent;
        while let Some(owner) = parent.and_then(|id| self.cache.node(id)) {
            ancestors.push(
                json!({"id": owner.runtime_id.get(), "locator": owner.backend_locator.encode(),
                "role": owner.role.to_string(), "label": owner.name}),
            );
            parent = owner.parent;
        }
        json!({"node": node_data(&self.cache, node), "locator": node.backend_locator.encode(),
            "ancestors": ancestors,
            "actions": node.actions.iter().map(|a| &a.name).collect::<Vec<_>>(),
            "capabilities": node.capabilities.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>()})
    }

    pub async fn agent_observe(&mut self, observation_id: u64) -> Result<AgentObservation, String> {
        self.refresh_surface_snapshot().await?;
        if !self.is_available() {
            return Err("Application unavailable".into());
        }
        let mut entries = self.public_action_entries();
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        let mut candidates = Vec::new();
        let mut operations = Vec::new();
        for (target, locator, label, operation) in entries {
            let Some(node) = self.cache.node(target) else {
                continue;
            };
            if secret_node(&self.cache, node) {
                continue;
            }
            let Some(mut data) = operation_data(&operation) else {
                continue;
            };
            let Ok(authority) = OperationAuthority::capture(
                &self.runtime,
                &self.application_locator,
                target,
                &locator,
                &self.cache,
                &self.scopes,
            ) else {
                continue;
            };
            data["id"] = json!(format!("op-{}", candidates.len()));
            data["target"] = json!(target.get());
            let (label, partial) = operation_label(&label, target);
            data["label"] = json!(label);
            data["label_partial"] = json!(partial);
            operations.push(data);
            candidates.push(AgentCandidate {
                id: data_id_for_candidate(candidates.len()),
                target,
                locator,
                operation,
                authority,
                precondition: self.agent_target_precondition(target),
            });
        }
        let mut nodes: Vec<_> = self
            .cache
            .nodes()
            .filter(|n| self.scopes.allows_node(n.runtime_id))
            .collect();
        nodes.sort_by_key(|n| n.runtime_id);
        let mut exported: Vec<_> = nodes.iter().map(|n| node_data(&self.cache, n)).collect();
        let (reads, _) = crate::content::materialize::read_public_texts(
            &self.backend,
            &self.cache,
            &self.scopes,
            &self.refresh_cancel,
        )
        .await?;
        for (index, node) in nodes.iter().enumerate() {
            if let Some(read) = reads.get(&node.runtime_id) {
                let result = match read {
                    crate::transcompile::document::TextRead::Complete(text) => Ok(text.clone()),
                    _ => Err("Public text unavailable".to_owned()),
                };
                exported[index] = public_text_data(exported[index].take(), result);
            }
        }
        let data = json!({
            "version": "gui2tui.agent.v1", "observation_id": observation_id,
            "session": self.runtime.id, "generation": self.runtime.generation(),
            "interaction_scope": self.scopes.active().to_string(),
            "partial": self.cache.nodes().any(|n| !n.truncations.is_empty())
                || exported.iter().any(|n| n["partial"] == true),
            "nodes": exported,
            "operations": operations,
        });
        Ok(AgentObservation {
            data,
            candidates,
            created: Instant::now(),
        })
    }

    pub(super) fn document_agent_observation(
        &self,
        data: Value,
    ) -> Result<AgentObservation, String> {
        let mut candidates = Vec::new();
        for op in &self.document.operations {
            if !crate::transcompile::document::wire::agent_supports(&op.operation) {
                continue;
            }
            let authority = OperationAuthority::capture(
                &self.runtime,
                &self.application_locator,
                op.target,
                &op.locator,
                &self.cache,
                &self.scopes,
            )
            .map_err(|_| "Current operation authority changed; observe again".to_owned())?;
            candidates.push(AgentCandidate {
                id: op.id.clone(),
                target: op.target,
                locator: op.locator.clone(),
                operation: op.operation.clone(),
                authority,
                precondition: self.agent_target_precondition(op.target),
            });
        }
        Ok(AgentObservation {
            data,
            candidates,
            created: Instant::now(),
        })
    }

    /// Consume an exact candidate after refreshing the authoritative tree. The
    /// caller consumes the observation even on failure; uncertain writes are never replayed.
    pub async fn agent_execute(
        &mut self,
        observation: AgentObservation,
        operation_id: &str,
        parameter: Option<Value>,
    ) -> Result<Value, AgentExecutionError> {
        let profile =
            observation.data["version"] == crate::transcompile::document::wire::DOCUMENT_VERSION;
        let started = Instant::now();
        let result = self
            .agent_execute_inner(observation, operation_id, parameter)
            .await;
        result.map(|mut receipt| {
            if profile {
                receipt["timings"]["execute_wall_ms"] =
                    json!(started.elapsed().as_secs_f64() * 1000.0);
            }
            receipt
        })
    }

    /// One consumed publication owns this bounded list. Each original binding
    /// independently traverses the ordinary fresh authority/readback chain.
    /// No new target, locator rebinding, write retry or atomicity is introduced.
    pub async fn agent_execute_batch_v1(
        &mut self,
        observation: AgentObservation,
        requests: Vec<(String, Option<Value>)>,
        serial: &mut u64,
        width: u16,
        height: u16,
    ) -> Result<(Value, Option<AgentObservation>), String> {
        self.agent_render_tui_text(&observation, width, height)?;
        let published = observation
            .candidates
            .iter()
            .map(|candidate| (candidate.id.clone(), candidate.operation.clone()))
            .collect::<Vec<_>>();
        // Validate the whole request before the first possible GUI write.
        validate_batch_requests(&published, &requests)?;
        let mut items = Vec::new();
        let mut status = "completed";
        let mut reason: Option<String> = None;
        let mut latest = None;
        let mut view = Value::Null;
        for (operation_id, parameter) in &requests {
            self.agent_tui_prepare_execution();
            let candidate = observation
                .candidates
                .iter()
                .find(|candidate| candidate.id == *operation_id)
                .expect("whole batch preflight checked this original candidate")
                .clone();
            let step_observation = AgentObservation {
                data: observation.data.clone(),
                candidates: vec![candidate],
                created: observation.created,
            };
            let result = self
                .agent_execute(step_observation, operation_id, parameter.clone())
                .await;
            let mut item = json!({"operation_id":operation_id});
            match result {
                Ok(receipt) => {
                    let already = receipt["effect"] == "already_satisfied";
                    if !already && receipt["accepted"] != true {
                        status = "unknown";
                        reason = Some("Backend acceptance not established; remaining operations not attempted".into());
                    } else if receipt["effect"] != "confirmed" && !already {
                        status = "interrupted";
                        reason = Some(
                            "Primitive effect unknown; remaining operations not attempted".into(),
                        );
                    }
                    item["receipt"] = receipt;
                }
                Err(error) => {
                    status = if error.delivery() == "not_attempted" {
                        "interrupted"
                    } else {
                        "unknown"
                    };
                    reason = Some(error.to_string());
                    item["error"] = json!({"delivery":error.delivery(),"error":error.to_string()});
                }
            }
            *serial += 1;
            match self.agent_observe_document(*serial).await {
                Ok(fresh) => match self.agent_render_tui_text(&fresh, width, height) {
                    Ok(fresh_view) => {
                        item["observation_id"] = fresh.data["observation_id"].clone();
                        view = fresh_view;
                        latest = Some(fresh);
                    }
                    Err(error) => {
                        status = "unknown";
                        reason = Some(error);
                        latest = None;
                        view = Value::Null;
                    }
                },
                Err(error) => {
                    status = "unknown";
                    reason = Some(error);
                    latest = None;
                    view = Value::Null;
                }
            }
            items.push(item);
            if status != "completed" {
                break;
            }
        }
        let batch = json!({"version":"gui2tui.agent.batch.v1","status":status,
            "items":items,"requested":requests.len(),"reason":reason,"atomic":false,
            "authority":"original publication, exact frozen bindings; fresh validation per operation"});
        let response = json!({"batch":batch,"tui_view":view,
            "observation":latest.as_ref().map(|fresh| &fresh.data)});
        Ok((response, latest))
    }

    async fn agent_execute_inner(
        &mut self,
        observation: AgentObservation,
        operation_id: &str,
        parameter: Option<Value>,
    ) -> Result<Value, AgentExecutionError> {
        if observation.created.elapsed() > Duration::from_secs(120) {
            return Err("Observation expired; observe again".into());
        }
        let candidate = observation
            .candidates
            .iter()
            .find(|candidate| candidate.id == operation_id)
            .ok_or("Unknown or non-invocable observation-local operation ID")?;
        validate_parameter(&candidate.operation, parameter.as_ref())?;
        let profile =
            observation.data["version"] == crate::transcompile::document::wire::DOCUMENT_VERSION;
        let fresh_started = Instant::now();
        self.refresh_surface_snapshot().await?;
        let fresh_ms = fresh_started.elapsed().as_secs_f64() * 1000.0;
        let delivery_started = Instant::now();
        let fresh_precondition = self.agent_target_precondition(candidate.target);
        if fresh_precondition != candidate.precondition {
            let changed = changed_precondition_fields(&candidate.precondition, &fresh_precondition);
            return Err(format!(
                "Target changed while deciding ({}); observe again",
                changed.join(", ")
            )
            .into());
        }
        candidate
            .authority
            .validate_before_invocation(&self.runtime, &self.cache, &self.scopes)
            .map_err(|e| format!("Operation authority rejected: {e:?}"))?;
        let target = candidate.target;
        let locator = candidate.locator.clone();
        if let PublicOperation::Action(name) = &candidate.operation {
            if parameter.is_some() {
                return Err("Action takes no parameter".into());
            }
            let fresh = self
                .backend
                .refresh_node(&locator, false)
                .await
                .map_err(|e| e.to_string())?;
            if !super::super::action::is_public_action_target(&fresh.states)
                || fresh.actions.iter().filter(|a| &a.name == name).count() != 1
            {
                return Err("Public action no longer available".into());
            }
            let action = fresh.actions.into_iter().find(|a| &a.name == name).unwrap();
            let accepted = self
                .invoke_action_with_transition(
                    target,
                    UiIntent::Activate,
                    locator,
                    action,
                    name.clone(),
                )
                .await;
            return Ok(receipt_timings(
                json!({"accepted": if accepted { Some(true) } else { None }, "effect": "unknown", "message": self.status,
                "operation_id": operation_id, "observation_id": observation.data["observation_id"]}),
                profile,
                fresh_ms,
                delivery_started.elapsed().as_secs_f64() * 1000.0,
            ));
        }
        let ticket = self
            .runtime
            .begin(
                crate::runtime::OperationKind::TextInteraction,
                crate::modality::CancellationToken::default(),
            )
            .map_err(|e| e.to_string())?;
        let result = self.agent_deliver(candidate, parameter, &ticket).await;
        self.runtime
            .complete(&ticket)
            .map_err(|e| AgentExecutionError::Unknown(e.to_string()))?;
        let effect = result.map_err(AgentExecutionError::Unknown)?;
        let confirmed = readback_confirms_request(&effect);
        let already_satisfied = effect["selection_outcome_version"]
            == "gui2tui.selection-outcome.v1"
            && effect["already_satisfied"] == true
            && effect["write_invoked"] == false;
        let mut receipt = json!({"accepted": if confirmed { Some(true) } else { None },
            "effect": if confirmed { "confirmed" } else { "unknown" }, "readback": effect,
            "operation_id": operation_id, "observation_id": observation.data["observation_id"]});
        if already_satisfied {
            // Confirmed current state is not a delivered or accepted write.
            receipt["accepted"] = Value::Null;
            receipt["delivery"] = json!("not_attempted");
            receipt["effect"] = json!("already_satisfied");
        }
        Ok(receipt_timings(
            receipt,
            profile,
            fresh_ms,
            delivery_started.elapsed().as_secs_f64() * 1000.0,
        ))
    }

    async fn agent_deliver(
        &mut self,
        candidate: &AgentCandidate,
        parameter: Option<Value>,
        ticket: &crate::runtime::OperationTicket,
    ) -> Result<Value, String> {
        let target = candidate.target;
        let locator = &candidate.locator;
        let authority_check = || {
            if !self.runtime.validates_ticket(ticket) {
                return Err(ActuationError::AuthorityRejected(
                    "Operation ticket retired".into(),
                ));
            }
            candidate
                .authority
                .validate_before_invocation(&self.runtime, &self.cache, &self.scopes)
                .map_err(|e| ActuationError::AuthorityRejected(format!("{e:?}")))
        };
        authority_check().map_err(|e| e.to_string())?;
        match &candidate.operation {
            PublicOperation::EditText { multiline } => {
                let text = parameter
                    .and_then(|p| p.as_str().map(str::to_owned))
                    .ok_or("Text parameter required")?;
                if text.len() > crate::backend::external_text_limit() {
                    return Err("Text exceeds configured edit budget".into());
                }
                if *multiline {
                    let expected = self
                        .backend
                        .read_complete_multiline_text(locator)
                        .await
                        .map_err(|e| e.to_string())?;
                    let resolved = resolve_cached_node_operation(
                        &self.cache,
                        SemanticOperation::ReplaceComplexText {
                            target,
                            expected: expected.clone(),
                            text: text.clone(),
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    if !matches!(resolved, BackendOperation::SetComplexTextContents { .. }) {
                        return Err("Unsupported text operation".into());
                    }
                    authority_check().map_err(|e| e.to_string())?;
                    let readback = self
                        .backend
                        .replace_complete_multiline_text(locator, &expected, &text)
                        .await
                        .map_err(|e| e.to_string())?;
                    if readback.resulting != text
                        || crate::backend::text_evidence::has_ambiguous_padding(&readback.resulting)
                    {
                        let node = self.cache.node(target).ok_or("Target disappeared")?;
                        let binding = SceneBinding {
                            runtime_id: target,
                            backend_locator: locator.clone(),
                            semantic_role: node.role.clone(),
                            actions: node.actions.clone(),
                            capability: InteractionCapability::EditText,
                            default_intent: UiIntent::BeginEdit,
                        };
                        let result = self
                            .native_input
                            .edit(
                                &self.backend,
                                &self.cache,
                                &binding,
                                SemanticActuation::Edit(text),
                                &authority_check,
                            )
                            .await
                            .map_err(|e| e.to_string())?;
                        return Ok(
                            json!({"text": result.observed_text, "text_evidence": result.readback_evidence}),
                        );
                    }
                    Ok(json!({"text": readback.resulting}))
                } else {
                    let node = self.cache.node(target).ok_or("Target disappeared")?;
                    let binding = SceneBinding {
                        runtime_id: target,
                        backend_locator: locator.clone(),
                        semantic_role: node.role.clone(),
                        actions: node.actions.clone(),
                        capability: InteractionCapability::EditText,
                        default_intent: UiIntent::BeginEdit,
                    };
                    resolve_cached_node_operation(
                        &self.cache,
                        SemanticOperation::ReplaceText {
                            target,
                            text: text.clone(),
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    let result = self
                        .native_input
                        .edit(
                            &self.backend,
                            &self.cache,
                            &binding,
                            SemanticActuation::Edit(text),
                            &authority_check,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(
                        json!({"text": result.observed_text, "text_evidence": result.readback_evidence}),
                    )
                }
            }
            PublicOperation::Selection { parent, selected } => {
                if parameter.is_some() {
                    return Err("Selection takes no parameter".into());
                }
                if !self
                    .cache
                    .runtime_id(parent)
                    .is_some_and(|id| self.scopes.allows_node(id))
                {
                    return Err("Selection owner outside scope".into());
                }
                let outcome = self
                    .backend
                    .set_child_selected(parent, locator, *selected)
                    .await
                    .map_err(|e| e.to_string())?;
                let already_satisfied =
                    outcome == crate::backend::SelectionMembershipOutcome::AlreadySatisfied;
                Ok(
                    json!({"selection_outcome_version":"gui2tui.selection-outcome.v1",
                    "selected": selected, "already_satisfied": already_satisfied,
                    "write_invoked": !already_satisfied}),
                )
            }
            PublicOperation::Value { increase } => {
                if parameter.is_some() {
                    return Err("Adjustment takes no parameter".into());
                }
                let value = self
                    .backend
                    .adjust_value(locator, *increase)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(
                    json!({"previous": value.previous, "requested": value.requested, "resulting": value.resulting}),
                )
            }
            PublicOperation::SetValue => {
                let requested = parameter
                    .and_then(|p| p.as_f64())
                    .filter(|v| v.is_finite())
                    .ok_or("Finite numeric parameter required")?;
                let value = self
                    .backend
                    .value_parameters(locator)
                    .await
                    .map_err(|e| e.to_string())?;
                authority_check().map_err(|e| e.to_string())?;
                let value = self
                    .backend
                    .set_value(locator, value.0, requested)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(
                    json!({"previous": value.previous, "requested": value.requested, "resulting": value.resulting}),
                )
            }
            _ => Err("Operation unsupported by agent protocol v1".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::{DebugInfo, SemanticNode};

    fn node(path: &str, kind: Option<TextInputKind>) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(0),
            backend_locator: BackendLocator::new(":1.8", path),
            index_in_parent: None,
            role: SemanticRole::TextInput,
            name: Some("secret-marker".into()),
            description: Some("secret-marker".into()),
            value: Some("secret-marker".into()),
            text_input_kind: kind,
            states: vec![SemanticState::Enabled],
            actions: vec![],
            capabilities: vec![],
            children: vec![],
            truncations: vec![],
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn numeric_readback_must_match_the_requested_effect() {
        for resulting in [30.0, 64.0] {
            assert!(!readback_confirms_request(
                &json!({"previous": 30.0, "requested": 65.0, "resulting": resulting})
            ));
        }
        assert!(readback_confirms_request(
            &json!({"previous": 30.0, "requested": 65.0, "resulting": 65.0})
        ));
        assert!(readback_confirms_request(&json!({"text": ""})));
        assert!(readback_confirms_request(&json!({"selected": true})));
    }

    #[test]
    fn agent_redacts_password_subtree_metadata_and_values() {
        let mut root = node("/password", Some(TextInputKind::Password));
        root.children.push(node("/password/child", None));
        let cache = SemanticCache::from_snapshot(root).unwrap();
        for node in cache.nodes() {
            let exported = node_data(&cache, node);
            assert_eq!(exported["redacted"], true);
            assert!(!exported.to_string().contains("secret-marker"));
            assert!(
                !public_text_data(exported, Ok("secret-marker".into()))
                    .to_string()
                    .contains("secret-marker")
            );
        }
    }

    #[test]
    fn agent_exports_complete_public_text_instead_of_presentation_preview() {
        let text = "长段落🧪".repeat(300);
        let data = public_text_data(
            json!({"value": "preview…", "partial": false}),
            Ok(text.clone()),
        );
        assert_eq!(data["value"], text);
        assert_eq!(data["value_complete"], true);
        assert_eq!(data["partial"], false);
        assert_eq!(public_text_data(data, Ok(String::new()))["value"], "");
    }

    #[test]
    fn agent_does_not_publish_a_preview_as_complete_after_a_failed_read() {
        let data = public_text_data(
            json!({"value": "old preview…", "partial": false}),
            Err("public target unavailable".into()),
        );
        assert!(data["value"].is_null());
        assert_eq!(data["partial"], true);
        assert_eq!(data["value_complete"], false);
        assert!(!data.to_string().contains("old preview"));
    }
}

#[cfg(test)]
mod precondition_diagnostics_tests {
    use super::*;
    #[test]
    fn rejected_precondition_reports_categories_without_source_values() {
        let before = json!({"node":{"value":"SECRET-BEFORE","states":["Enabled"]},"ancestors":["PRIVATE-TITLE"]});
        let after = json!({"node":{"value":"SECRET-AFTER","states":["Enabled"]},"ancestors":["PRIVATE-OTHER"]});
        let changed = changed_precondition_fields(&before, &after);
        assert_eq!(changed, vec!["ancestors", "node.value"]);
        assert!(!format!("{changed:?}").contains("SECRET"));
        assert!(changed_precondition_fields(&before, &before).is_empty());
    }
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    #[test]
    fn batch_preflight_rejects_bad_later_parameters_new_ids_and_duplicates() {
        let published = vec![
            (
                "edit".into(),
                PublicOperation::EditText { multiline: false },
            ),
            ("number".into(), PublicOperation::SetValue),
        ];
        assert!(
            validate_batch_requests(
                &published,
                &[
                    ("edit".into(), Some(json!("公开文本"))),
                    ("number".into(), Some(json!(42)))
                ]
            )
            .is_ok()
        );
        for requests in [
            vec![
                ("edit".into(), Some(json!("ok"))),
                ("number".into(), Some(json!("bad"))),
            ],
            vec![("unpublished".into(), None)],
            vec![
                ("edit".into(), Some(json!("a"))),
                ("edit".into(), Some(json!("b"))),
            ],
        ] {
            assert!(validate_batch_requests(&published, &requests).is_err());
        }
        assert!(validate_batch_requests(&published, &[]).is_err());
    }
}
