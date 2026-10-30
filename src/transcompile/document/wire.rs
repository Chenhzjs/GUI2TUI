//! Versioned, full serialization of the shared document. No organization rules
//! or GUI I/O: every field below is a projection of this same observation.
use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const DOCUMENT_VERSION: &str = "gui2tui.agent.document.v1";
pub const SOURCE_IDENTITY_VERSION: &str = "gui2tui.source_locator_refs.v1";

/// Add only owner-issued, non-callable provenance to this same document.
pub(crate) fn to_agent_document_with_source_refs(
    doc: &TuiDocument,
    refs: &BTreeMap<RuntimeNodeId, crate::runtime::SourceLocatorRef>,
) -> Result<Value, &'static str> {
    if !refs.keys().eq(doc.nodes.keys())
        || refs
            .values()
            .map(|r| r.token())
            .collect::<HashSet<_>>()
            .len()
            != refs.len()
        || refs.iter().any(|(id, r)| {
            !r.matches_source(
                &doc.observation.session,
                doc.observation.generation,
                &doc.nodes[id].backend_locator,
            )
        })
    {
        return Err("Public source reference inventory is incomplete or ambiguous");
    }
    let mut data = to_agent_document(doc);
    for row in data["nodes"].as_array_mut().expect("serialized node array") {
        let id = RuntimeNodeId::new(row["id"].as_u64().expect("serialized source ID"));
        row["locator_ref"] = json!(refs[&id].token());
    }
    data["source_identity"] = json!({
        "version":SOURCE_IDENTITY_VERSION,
        "namespace":{"session":doc.observation.session,"generation":doc.observation.generation},
        "basis":"exact_backend_locator_within_runtime_session_and_application_generation",
        "reference_kind":"read_only_locator_reference",
        "lifetime":"not_a_logical_entity_lifetime_or_atomic_snapshot_guarantee",
        "execution_authority":"none; execute requires current observation, operation and ticket"
    });
    Ok(data)
}

fn text_read(read: &TextRead) -> Value {
    match read {
        TextRead::Complete(text) => json!({"status":"complete", "text":text}),
        TextRead::NotRequested => json!({"status":"not_requested"}),
        TextRead::Unavailable => json!({"status":"unavailable"}),
    }
}

fn region(region: &SemanticRegion) -> Value {
    json!({"id":region.id.get(), "kind":region.kind.to_string(),
        "source_nodes":region.source_nodes.iter().map(|id|id.get()).collect::<Vec<_>>(),
        "label":region.label, "descriptions":region.descriptions, "errors":region.errors,
        "logical_group":region.logical_group.iter().map(|id|id.get()).collect::<Vec<_>>(),
        "children":region.children.iter().map(self::region).collect::<Vec<_>>(),
        "interactions":region.interactions.iter().map(|i|json!({"source":i.source.get(), "intent":format!("{:?}",i.intent)})).collect::<Vec<_>>(),
        "confidence":format!("{:?}",region.confidence), "modality":format!("{:?}",region.modality),
        "command_path":region.command_path})
}

fn command_group(group: &CommandGroup) -> Value {
    json!({"source":group.source.get(), "label":group.label, "scope":group.scope.0.get(),
        "children":group.children.iter().map(|entry| match entry {
            CommandEntry::Group(group) => json!({"kind":"group", "group":command_group(group)}),
            CommandEntry::Command(command) => json!({"kind":"command", "source":command.source.get(),
                "label":command.label, "scope":command.scope.0.get(), "intent":format!("{:?}",command.intent),
                "enabled":command.enabled, "showing":command.visible, "shortcut":command.shortcut}),
        }).collect::<Vec<_>>()})
}

pub fn agent_supports(operation: &PublicOperation) -> bool {
    !matches!(
        operation,
        PublicOperation::EditRange | PublicOperation::ReadSelection
    )
}

pub fn operation_kind(operation: &PublicOperation) -> Value {
    match operation {
        PublicOperation::Action(name) => json!({"kind":"action", "name":name}),
        PublicOperation::EditText { multiline } => {
            json!({"kind":"replace_text", "multiline":multiline, "parameter":"text"})
        }
        PublicOperation::Selection { selected, .. } => {
            json!({"kind":"selection", "selected":selected})
        }
        PublicOperation::Value { increase } => json!({"kind":"adjust_value", "increase":increase}),
        PublicOperation::SetValue => json!({"kind":"set_value", "parameter":"value"}),
        PublicOperation::EditRange => json!({"kind":"edit_range"}),
        PublicOperation::ReadSelection => json!({"kind":"read_selection"}),
    }
}

fn parameter_domain(domain: &ParameterDomain) -> Value {
    match domain {
        ParameterDomain::None => json!({"kind":"none"}),
        ParameterDomain::Text => json!({"kind":"text"}),
        ParameterDomain::TextRange => json!({"kind":"text_range", "bounds":"unknown"}),
        ParameterDomain::Number(domain) => match domain {
            ValueDomain::Known {
                current,
                min,
                max,
                step,
            } => {
                json!({"kind":"number", "status":"known", "current":current,"min":min,"max":max,"step":step})
            }
            ValueDomain::Unknown => json!({"kind":"number", "status":"unknown"}),
            ValueDomain::Unavailable => json!({"kind":"number", "status":"unavailable"}),
        },
    }
}

/// Thin protocol expression of the shared core label, preserving v1 shape.
pub fn operation_label(doc: &TuiDocument, target: RuntimeNodeId) -> Value {
    let label = doc
        .operation_label(target)
        .expect("validated public operation target");
    let origin = match label.origin {
        OperationLabelOrigin::PublicName => "public_name",
        OperationLabelOrigin::PublicDescription => "public_description",
        OperationLabelOrigin::ContentContext => "content_context",
        OperationLabelOrigin::SemanticRegion => "semantic_region",
        OperationLabelOrigin::RoleFallback => "role_fallback",
    };
    let mut data = json!({"source_label":label.source_label,"display_label":label.display_label,
        "origin":origin,"source_nodes":label.source_nodes.iter().map(|id|id.get()).collect::<Vec<_>>()});
    if let Some((region, confidence)) = label.region {
        data["region"] = json!(region.get());
        data["confidence"] = json!(format!("{confidence:?}"));
    }
    data
}

pub fn to_agent_document(doc: &TuiDocument) -> Value {
    let mut boundaries = BTreeMap::<String, Value>::new();
    let relations: Vec<_> = doc.relations.iter().map(|(source,state)| {
        let mut row = json!({"source":source.get()});
        match state {
            DocumentRelations::Unknown => row["status"] = json!("unknown"),
            DocumentRelations::Unavailable => row["status"] = json!("unavailable"),
            DocumentRelations::Known(edges) => {
                row["status"] = json!("known");
                row["edges"] = json!(edges.iter().map(|edge|json!({"kind":edge.kind.to_string(),
                    "targets":edge.targets.iter().map(|endpoint|match endpoint {
                        RelationEndpoint::Node(id) => json!({"node":id.get()}),
                        RelationEndpoint::OutsideScope(id) => json!({"boundary":"outside_scope","node":id.get()}),
                        RelationEndpoint::Redacted => json!({"boundary":"redacted"}),
                        RelationEndpoint::Unresolved(locator) => {
                            let digest: String = Sha256::digest(locator.encode().as_bytes()).iter().map(|b|format!("{b:02x}")).collect();
                            let id=format!("external-{digest}");
                            boundaries.insert(id.clone(),json!({"id":id,"status":"unresolved_public_reference","payload":"unavailable"}));
                            json!({"boundary_reference":id})
                        }
                    }).collect::<Vec<_>>() })).collect::<Vec<_>>());
            }
        }
        row
    }).collect();
    let nodes: Vec<_> = doc.nodes.values().map(|node|json!({
        "id":node.runtime_id.get(),"parent":node.parent.map(|id|id.get()),
        "children":node.children.iter().map(|id|id.get()).collect::<Vec<_>>(),"index_in_parent":node.index_in_parent,
        "role":node.role.to_string(),"native_role":node.debug.atspi_role,"label":node.name,"description":node.description,
        "value":node.value,"value_source":"semantic_cache",
        "value_read_status":crate::semantic::value_read_status(node.value.as_deref(),node.text_input_kind,&node.debug.interfaces,&node.truncations),
        // A cache preview proves exact text only when this observation also
        // contains the same source's complete, byte-equal public Text payload.
        "value_complete":match (node.value.as_deref(),doc.text_reads.get(&node.runtime_id)) {
            (Some(value),Some(TextRead::Complete(text)))=>Some(value==text.as_str()),
            _=>None,
        },
        "boundary_only":doc.boundary_nodes.contains(&node.runtime_id),
        "text_input_kind":node.text_input_kind.map(|k|format!("{k:?}")),
        "states":node.states.iter().map(|s|format!("{s:?}")).collect::<Vec<_>>(),
        "interfaces":node.debug.interfaces,"capabilities":node.capabilities.iter().map(|c|format!("{c:?}")).collect::<Vec<_>>(),
        "actions":node.actions.iter().map(|a|json!({"name":a.name,"advertised_index":a.index,"description":a.description,"keybinding":a.keybinding,
            "name_ambiguous":node.actions.iter().filter(|peer|peer.name==a.name).count()!=1})).collect::<Vec<_>>(),
        "partial":!node.truncations.is_empty(),"truncations":node.truncations.iter().map(|t|match t{
            crate::semantic::TreeTruncation::ManagedDescendants=>json!({"kind":"provider_managed"}),
            crate::semantic::TreeTruncation::MaxDepth{limit}=>json!({"kind":"max_depth","limit":limit}),
            crate::semantic::TreeTruncation::MaxNodes{limit}=>json!({"kind":"max_nodes","limit":limit}),
            crate::semantic::TreeTruncation::OperationTimeout{operation,..}=>json!({"kind":"timeout","operation":operation}),
            crate::semantic::TreeTruncation::ReadFailure{operation,..}=>json!({"kind":"read_failure","operation":operation})
        }).collect::<Vec<_>>(),
        "geometry":node.debug.geometry.as_ref().map(|g|json!({"x":g.x,"y":g.y,"width":g.width,"height":g.height,"authority":"presentation_evidence_only"}))
    })).collect();
    let operations: Vec<_> = doc
        .operations
        .iter()
        .map(|op| {
            let mut data = operation_kind(&op.operation);
            data["id"] = json!(op.id);
            data["target"] = json!(op.target.get());
            data["parameter_domain"] = parameter_domain(&op.parameter_domain);
            data["label_provenance"] = operation_label(doc, op.target);
            data["label"] = data["label_provenance"]["display_label"].clone();
            data["agent_invocable"] = json!(agent_supports(&op.operation));
            if let Some(presentation) = doc.action_presentation(op.target, &op.operation) {
                data["action_semantics"] = serde_json::to_value(presentation)
                    .expect("public action presentation is serializable");
            }
            if let PublicOperation::Selection { parent, .. } = &op.operation {
                data["owner"] = json!(
                    doc.nodes
                        .values()
                        .find(|n| n.backend_locator == *parent)
                        .map(|n| n.runtime_id.get())
                );
            }
            data
        })
        .collect();
    let content: Vec<_> = doc.content.models().map(|model|json!({"root":model.root.get(),"kind":format!("{:?}",model.kind),
        "metadata":{"public_probe_status":match doc.metadata_reads.get(&model.root){Some(MetadataRead::Known(_))=>"queried",Some(MetadataRead::Unknown)=>"unknown",Some(MetadataRead::Unavailable)=>"unavailable",None=>"not_exposed"},"title":model.metadata.title,"locale":model.metadata.locale,"current_page":model.metadata.current_page,"page_count":model.metadata.page_count,"attributes":model.metadata.attributes},
        "completeness":format!("{:?}",model.completeness),"roots":model.roots.iter().map(|id|id.get()).collect::<Vec<_>>(),
        "blocks":model.ordered_blocks().map(|block|json!({"id":block.id.get(),"source":block.source.get(),"kind":format!("{:?}",block.kind),
            "label":block.label,"children":block.children.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "interactive_sources":block.interactive_sources.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "text":if doc.text_reads.contains_key(&block.source){json!({"payload_ref":{"text_source":block.source.get()}})}else{match &block.text{
                TextContentState::Loaded(text)=>json!({"status":"loaded","text":text}),TextContentState::Summary(text)=>json!({"status":"summary","text":text}),
                TextContentState::Unknown=>json!({"status":"unknown"}),TextContentState::Unavailable=>json!({"status":"unavailable"})}}
        })).collect::<Vec<_>>(),
        "navigation":{"headings":model.navigation.headings.iter().map(|id|id.get()).collect::<Vec<_>>(),"links":model.navigation.links.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "form_fields":model.navigation.form_fields.iter().map(|id|id.get()).collect::<Vec<_>>(),"lists":model.navigation.lists.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "tables":model.navigation.tables.iter().map(|id|id.get()).collect::<Vec<_>>(),"landmarks":model.navigation.landmarks.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "comments":model.navigation.comments.iter().map(|id|id.get()).collect::<Vec<_>>(),"opaque":model.navigation.opaque.iter().map(|id|id.get()).collect::<Vec<_>>()}
    })).collect();
    let mut choices: Vec<_> = doc.choices.choices().collect();
    choices.sort_by_key(|c| c.owner);
    let choice_rows = choices.iter().map(|c|json!({"owner":c.owner.get(),"current":c.current.map(|id|id.get()),"current_source":c.current_source,"completeness":format!("{:?}",c.completeness),
            "disclosure":format!("{:?}",c.disclosure),"dismiss":format!("{:?}",c.dismiss),"options_status":match c.options{super::super::ChoiceOptions::Available(_)=>"available",super::super::ChoiceOptions::Partial(_)=>"partial",super::super::ChoiceOptions::Unavailable=>"unavailable"},
            "options":c.options.options().iter().map(|o|json!({"id":o.runtime_id.get(),"label":o.label,"selected":o.selected,"enabled":o.enabled,
                "selection":o.selection.as_ref().map(|s|match s{super::super::ChoiceSelectionStrategy::ParentSelection{parent,child}=>json!({"kind":"parent_selection","parent":parent.get(),"child":child.get()}),
                    super::super::ChoiceSelectionStrategy::ChildSemanticAction{child,action}=>json!({"kind":"named_action","child":child.get(),"name":action.name})})})).collect::<Vec<_>>()
        })).collect::<Vec<_>>();
    let mut data = json!({"version":DOCUMENT_VERSION,"observation_id":doc.observation.observation_id,
        "session":doc.observation.session,"generation":doc.observation.generation,
        "collection":{"started_at_ms":doc.observation.started_at_ms,"finished_at_ms":doc.observation.finished_at_ms,"atomic_snapshot":false},
        "readable_scope":doc.readable_scope.0.get(),"interaction_scope":doc.invocable_scope.0.get(),
        "scopes":doc.scopes.iter().map(|s|json!({"id":s.id.0.get(),"root":s.root.get(),"parent":s.parent.map(|id|id.0.get()),
            "children":s.children.iter().map(|id|id.0.get()).collect::<Vec<_>>(),"kind":format!("{:?}",s.kind),"label":s.label,"active":s.active,"modal":s.modal})).collect::<Vec<_>>(),
        "completeness":{"materialization":if doc.completeness.complete_for_available_payloads{"complete_for_available_payloads"}else{"incomplete_build"},
            "observed_coverage":match doc.completeness.observed_coverage{ObservedCoverage::CompleteForDeclaredScope=>"complete_for_declared_scope",ObservedCoverage::Partial=>"partial",ObservedCoverage::Unknown=>"unknown"},"gaps":doc.completeness.gaps.iter().map(|g|format!("{g:?}")).collect::<Vec<_>>(),
            "whole_application_coverage":"not_claimed","requires_expansion":false},
        "document_probes":doc.metadata_reads.iter().map(|(source,read)|json!({"source":source.get(),"read":match read{
            MetadataRead::Unknown=>json!({"status":"unknown"}),MetadataRead::Unavailable=>json!({"status":"unavailable"}),
            MetadataRead::Known(p)=>json!({"status":"queried","unavailable_fields":p.unavailable_fields,"document_interface":p.document_interface,"text_interface":p.text_interface,"hypertext_interface":p.hypertext_interface,
                "locale":p.locale,"current_page":p.current_page,"page_count":p.page_count,"attributes":p.attributes,"character_count":p.character_count,"hyperlink_count":p.hyperlink_count})
        }})).collect::<Vec<_>>(),
        "value_domains":doc.value_domains.iter().map(|(source,domain)|json!({"source":source.get(),"domain":parameter_domain(&ParameterDomain::Number(domain.clone()))})).collect::<Vec<_>>(),
        "nodes":nodes,"relations":relations,"boundary_references":boundaries.values().collect::<Vec<_>>(),
        "text_payloads":doc.text_reads.iter().map(|(source,read)|json!({"source":source.get(),"read":text_read(read)})).collect::<Vec<_>>(),
        "table_position_reads":doc.table_positions.iter().map(|(id, read)|json!({"source":id.get(),"read":match read {
            crate::content::TablePositionRead::Known { row, column, row_span, column_span } => json!({"status":"known","row":row,"column":column,"row_span":row_span,"column_span":column_span,"origin":"public_table_cell_exact_owner_and_reverse_locator"}),
            crate::content::TablePositionRead::Unavailable => json!({"status":"unavailable"}),
        }})).collect::<Vec<_>>(),
        "structures_version":"tui.document.structures.v1",
        "tables":doc.tables.iter().map(|table|json!({"owner":table.owner.get(),"rows":table.rows,"columns":table.columns,
            "counts_source":"complete_structural_extent_otherwise_unknown", "completeness":format!("{:?}",table.completeness),
            "row_selection":table.row_selection,"column_headers":table.column_headers,
            "cells":table.cells.iter().map(|cell|json!({"source":cell.source.get(),"row":cell.row,"column":cell.column,
                "position_source":format!("{:?}",cell.position_source),"label":cell.label,"label_source":cell.label_source.map(|id|id.get()),
                "row_span":cell.row_span,"column_span":cell.column_span,"span_status":if cell.row_span.is_some() && cell.column_span.is_some(){"known"}else if doc.table_positions.contains_key(&cell.source){"unavailable"}else{"not_requested"},"selected":cell.selected})).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "virtual_collections":doc.virtual_collections.iter().map(|c|json!({"owner":c.owner.get(),"completeness":format!("{:?}",c.completeness),
            "realized_items":c.realized_items.iter().map(|id|id.get()).collect::<Vec<_>>(),"selected_items":c.selected_items.iter().map(|id|id.get()).collect::<Vec<_>>(),
            "known_total":c.known_total,"active_descendant":c.active_descendant.map(|id|id.get()),"active_descendant_status":"not_collected"})).collect::<Vec<_>>(),
        "content":content,"regions":region(&doc.regions.root),"commands":command_group(&doc.commands.root),
        "choices":choice_rows,
        "operations":operations,"sources":doc.sources.iter().map(|(id,refs)|json!({"source":id.get(),"content_refs":refs.content.iter().map(|(root,block)|json!({"root":root.get(),"block":block.get()})).collect::<Vec<_>>(),"operation_refs":refs.operations})).collect::<Vec<_>>(),
        "shared_organization_policy":"tui.document.v1","execution_authority":"current G2T registry only; document is not authority"});
    if let Some(annotations) = &doc.decision_annotations {
        data["decision_annotations"] = annotations.clone();
    }
    data
}
