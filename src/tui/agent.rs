//! Native observation/dispatch boundary for external decision clients.
//! Model output is an observation-local operation ID, never an AT-SPI command.
use super::*;
use crate::semantic::{CachedSemanticNode, TextInputKind};
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

struct AgentCandidate {
    target: RuntimeNodeId,
    locator: BackendLocator,
    operation: PublicOperation,
    authority: OperationAuthority,
    precondition: Value,
}

fn secret_node(cache: &SemanticCache, node: &CachedSemanticNode) -> bool {
    let mut current = Some(node.runtime_id);
    while let Some(id) = current {
        let Some(node) = cache.node(id) else { break };
        if node.text_input_kind == Some(TextInputKind::Password) {
            return true;
        }
        current = node.parent;
    }
    false
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
        let mut pending = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                !secret_node(&self.cache, node)
                    && node.debug.interfaces.iter().any(|name| name == "Text")
            })
            .map(|(index, node)| (index, node.backend_locator.clone()));
        let mut reads = tokio::task::JoinSet::new();
        loop {
            // Schedule every public Text node; concurrency does not limit coverage.
            while reads.len() < 32 {
                let Some((index, locator)) = pending.next() else {
                    break;
                };
                let backend = self.backend.clone();
                reads.spawn(
                    async move { (index, backend.read_semantic_text_block(&locator).await) },
                );
            }
            let Some(read) = reads.join_next().await else {
                break;
            };
            let (index, result) =
                read.map_err(|error| format!("Public text read failed: {error}"))?;
            if matches!(result, Err(BackendError::SecretContentDisabled(_))) {
                return Err("Text target became sensitive; observe again".into());
            }
            exported[index] = public_text_data(
                exported[index].take(),
                result
                    .map(|range| range.text)
                    .map_err(|error| error.to_string()),
            );
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

    /// Consume an exact candidate after refreshing the authoritative tree. The
    /// caller consumes the observation even on failure; uncertain writes are never replayed.
    pub async fn agent_execute(
        &mut self,
        observation: AgentObservation,
        operation_id: &str,
        parameter: Option<Value>,
    ) -> Result<Value, AgentExecutionError> {
        if observation.created.elapsed() > Duration::from_secs(120) {
            return Err("Observation expired; observe again".into());
        }
        let index = observation.data["operations"]
            .as_array()
            .and_then(|ops| {
                ops.iter()
                    .position(|op| op["id"].as_str() == Some(operation_id))
            })
            .ok_or("Unknown observation-local operation ID")?;
        let candidate = &observation.candidates[index];
        validate_parameter(&candidate.operation, parameter.as_ref())?;
        self.refresh_surface_snapshot().await?;
        if self.agent_target_precondition(candidate.target) != candidate.precondition {
            return Err("Target changed while deciding; observe again".into());
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
            return Ok(
                json!({"accepted": if accepted { Some(true) } else { None }, "effect": "unknown", "message": self.status,
                "operation_id": operation_id, "observation_id": observation.data["observation_id"]}),
            );
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
        Ok(
            json!({"accepted": if confirmed { Some(true) } else { None },
            "effect": if confirmed { "confirmed" } else { "unknown" }, "readback": effect,
            "operation_id": operation_id, "observation_id": observation.data["observation_id"]}),
        )
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
                self.backend
                    .set_child_selected(parent, locator, *selected)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(json!({"selected": selected}))
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
