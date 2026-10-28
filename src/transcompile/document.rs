//! Pure, observation-owned shared logical document. No terminal state, backend
//! I/O, execution registry, or consumer-specific organization lives here.
//!
//! P1 internal contract only: this is not a new wire protocol. Runtime adapters
//! must supply the independent read/operation inventories before construction.
use std::collections::{BTreeMap, HashSet};

use crate::{
    content::{ContentBlockId, ContentCatalog, ContentCompleteness, TextContentState},
    runtime::{ApplicationGenerationId, RuntimeSessionId},
    semantic::{
        BackendLocator, CachedSemanticNode, CollectionCompleteness, RelationState,
        RelationalSemanticGraph, RuntimeNodeId, SemanticCache, SemanticCapability,
        SemanticRelationKind, SemanticRole, TextInputKind, collection_completeness,
    },
    tui::palette::PublicOperation,
};

use super::{
    ChoiceCatalog, CommandEntry, CommandGroup, CommandHierarchy, InteractionScope,
    InteractionScopeId, InteractionScopes, RegionAnalysis, SemanticRegion,
    analyze::analyze_document_regions,
};

pub mod annotations;
pub mod wire;

/// One public capability vocabulary for Human commands and Agent descriptors.
/// Exact execution authority remains owned by the runtime, never this catalogue.
pub fn public_operations(
    cache: &SemanticCache,
    scopes: &InteractionScopes,
    values: &BTreeMap<RuntimeNodeId, ValueDomain>,
) -> Vec<DocumentOperation> {
    let mut nodes: Vec<_> = cache
        .nodes()
        .filter(|n| {
            scopes.allows_node(n.runtime_id)
                && !secret_source(cache, n.runtime_id)
                && crate::tui::action::is_public_action_target(&n.states)
        })
        .collect();
    nodes.sort_by_key(|n| n.runtime_id);
    let mut operations = Vec::new();
    for node in nodes {
        let mut push = |operation, parameter_domain| {
            operations.push(DocumentOperation {
                id: format!("op-{}", operations.len()),
                target: node.runtime_id,
                locator: node.backend_locator.clone(),
                operation,
                parameter_domain,
            });
        };
        let multiline = node
            .capabilities
            .contains(&SemanticCapability::EditComplexText);
        if multiline || node.capabilities.contains(&SemanticCapability::EditText) {
            push(PublicOperation::EditRange, ParameterDomain::TextRange);
            push(PublicOperation::ReadSelection, ParameterDomain::None);
            push(
                PublicOperation::EditText { multiline },
                ParameterDomain::Text,
            );
        }
        for action in &node.actions {
            // Repeated public names remain visible in source actions. They do
            // not define an exact dispatchable named operation (never index 0).
            if !action.name.is_empty()
                && node
                    .actions
                    .iter()
                    .filter(|a| a.name == action.name)
                    .count()
                    == 1
            {
                push(
                    PublicOperation::Action(action.name.clone()),
                    ParameterDomain::None,
                );
            }
        }
        if let Some(owner) = node.parent.and_then(|id| cache.node(id))
            && scopes.allows_node(owner.runtime_id)
            && owner
                .capabilities
                .contains(&SemanticCapability::SelectChildren)
        {
            for selected in [true, false] {
                push(
                    PublicOperation::Selection {
                        parent: owner.backend_locator.clone(),
                        selected,
                    },
                    ParameterDomain::None,
                );
            }
        }
        if node.capabilities.contains(&SemanticCapability::Value) {
            push(
                PublicOperation::SetValue,
                ParameterDomain::Number(
                    values
                        .get(&node.runtime_id)
                        .cloned()
                        .unwrap_or(ValueDomain::Unknown),
                ),
            );
            for increase in [true, false] {
                push(PublicOperation::Value { increase }, ParameterDomain::None);
            }
        }
    }
    operations
}

pub fn secret_source(cache: &SemanticCache, id: RuntimeNodeId) -> bool {
    let mut current = Some(id);
    while let Some(id) = current {
        let Some(node) = cache.node(id) else {
            return true;
        };
        if node.text_input_kind == Some(TextInputKind::Password) {
            return true;
        }
        current = node.parent;
    }
    false
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentObservation {
    pub session: RuntimeSessionId,
    pub generation: ApplicationGenerationId,
    pub observation_id: u64,
    /// Milliseconds since Unix epoch, supplied by the observation owner.
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextRead {
    NotRequested,
    Complete(String),
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetadataRead {
    Unknown,
    Known(crate::backend::DocumentProbe),
    Unavailable,
}

/// Public interface evidence, never a client literal-candidate list.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueDomain {
    Known {
        current: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Unknown,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterDomain {
    None,
    Text,
    Number(ValueDomain),
    TextRange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocumentOperation {
    pub id: String,
    pub target: RuntimeNodeId,
    /// Internal exact binding evidence. A future serializer must not export a
    /// callable backend object; this descriptor itself grants no authority.
    pub locator: BackendLocator,
    pub operation: PublicOperation,
    pub parameter_domain: ParameterDomain,
}

/// Shared explanation of an exact public named action, never an execution rule.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ActionPresentation {
    pub version: &'static str,
    pub display_label: String,
    pub target_role: String,
    pub effect_semantics: &'static str,
    pub interpretation_basis: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_keybinding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
}

impl ActionPresentation {
    pub fn detail_text(&self) -> String {
        let mut text = format!(
            "{}\nTarget role: {}\nEffect semantics: {}",
            self.display_label, self.target_role, self.effect_semantics
        );
        if let Some(description) = &self.provider_description {
            text.push_str(&format!("\nPublic action description: {description}"));
        }
        if let Some(keybinding) = &self.provider_keybinding {
            text.push_str(&format!("\nPublic keybinding: {keybinding}"));
        }
        if let Some(note) = self.note {
            text.push_str(&format!("\n{note}"));
        }
        text
    }
}

pub struct DocumentInput<'a> {
    pub observation: DocumentObservation,
    pub cache: &'a SemanticCache,
    pub scopes: &'a InteractionScopes,
    /// Additional runtime redactions; passwords and all descendants are always
    /// excluded even when the owner supplies an empty set.
    pub redacted: &'a HashSet<RuntimeNodeId>,
    /// Results for all public Text sources, collected independently of layout.
    pub text_reads: &'a BTreeMap<RuntimeNodeId, TextRead>,
    pub value_domains: &'a BTreeMap<RuntimeNodeId, ValueDomain>,
    pub metadata_reads: &'a BTreeMap<RuntimeNodeId, MetadataRead>,
    /// Exact public cell positions, collected before pure organization.
    pub table_positions: &'a BTreeMap<RuntimeNodeId, crate::content::TablePositionRead>,
    /// Independent content-source inventory, before Human primary selection.
    /// Includes same-window and nested roots. Sensitive/out-of-scope roots are
    /// excluded here using the same disclosure rule as every other source.
    pub content_roots: &'a [RuntimeNodeId],
    /// Current supported-vocabulary descriptors from G2T, not inferred actions.
    pub operations: &'a [DocumentOperation],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationEndpoint {
    Node(RuntimeNodeId),
    OutsideScope(RuntimeNodeId),
    Redacted,
    /// Preserve distinct public references internally, without guessing a node.
    /// Wire adapters can assign opaque boundary IDs; locators confer no authority.
    Unresolved(BackendLocator),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentRelation {
    pub kind: SemanticRelationKind,
    pub targets: Vec<RelationEndpoint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentRelations {
    Unknown,
    Unavailable,
    Known(Vec<DocumentRelation>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverageGap {
    TreeIncomplete(RuntimeNodeId),
    PartialCollection(RuntimeNodeId),
    UnknownCollection(RuntimeNodeId),
    TextNotRequested(RuntimeNodeId),
    TextUnavailable(RuntimeNodeId),
    RelationsUnknown(RuntimeNodeId),
    RelationsUnavailable(RuntimeNodeId),
    UnresolvedRelation(RuntimeNodeId),
    ContentUnknown(RuntimeNodeId),
    ContentPartial(RuntimeNodeId),
    ValueNotRequested(RuntimeNodeId),
    ValueUnavailable(RuntimeNodeId),
    MetadataUnknown(RuntimeNodeId),
    MetadataUnavailable(RuntimeNodeId),
    TablePositionNotRequested(RuntimeNodeId),
    TablePositionUnavailable(RuntimeNodeId),
    TableSpanUnavailable(RuntimeNodeId),
    TableSpanNotRequested(RuntimeNodeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedCoverage {
    CompleteForDeclaredScope,
    Partial,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentCompleteness {
    pub complete_for_available_payloads: bool,
    pub observed_coverage: ObservedCoverage,
    pub gaps: Vec<CoverageGap>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceReferences {
    /// Content blocks are addressed by (model root, block ID), not a global
    /// arena index; nested models may refer to the same exact public payload.
    pub content: Vec<(RuntimeNodeId, ContentBlockId)>,
    pub operations: Vec<String>,
}

/// Canonical public operation wording, independent of any terminal scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationLabelOrigin {
    PublicName,
    SemanticRegion,
    RoleFallback,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationLabel {
    pub source_label: Option<String>,
    pub display_label: String,
    pub origin: OperationLabelOrigin,
    pub source_nodes: Vec<RuntimeNodeId>,
    pub region: Option<(super::RegionId, super::RegionConfidence)>,
}

#[derive(Clone, Debug)]
pub struct TuiDocument {
    pub decision_annotations: Option<serde_json::Value>,
    pub observation: DocumentObservation,
    /// Both currently follow InteractionScopes::allows_node. They remain
    /// explicit provenance fields, not a new authorization mechanism.
    pub readable_scope: InteractionScopeId,
    pub invocable_scope: InteractionScopeId,
    pub scopes: Vec<InteractionScope>,
    /// All public facts, including ungrouped, unnamed and non-showing nodes.
    /// Boundary ancestors have identity/role/structure only; no payload.
    pub nodes: BTreeMap<RuntimeNodeId, CachedSemanticNode>,
    pub boundary_nodes: HashSet<RuntimeNodeId>,
    pub relations: BTreeMap<RuntimeNodeId, DocumentRelations>,
    pub text_reads: BTreeMap<RuntimeNodeId, TextRead>,
    pub value_domains: BTreeMap<RuntimeNodeId, ValueDomain>,
    pub metadata_reads: BTreeMap<RuntimeNodeId, MetadataRead>,
    pub table_positions: BTreeMap<RuntimeNodeId, crate::content::TablePositionRead>,
    pub regions: RegionAnalysis,
    pub content: ContentCatalog,
    pub tables: Vec<crate::content::SemanticTableStructure>,
    pub virtual_collections: Vec<crate::content::VirtualCollectionStructure>,
    pub commands: CommandHierarchy,
    pub choices: ChoiceCatalog,
    pub operations: Vec<DocumentOperation>,
    pub sources: BTreeMap<RuntimeNodeId, SourceReferences>,
    pub completeness: DocumentCompleteness,
}

impl TuiDocument {
    /// The same source-grounded explanation for Human palettes and Agent export.
    /// An action name on an input does not establish submission/navigation.
    pub fn action_presentation(
        &self,
        target: RuntimeNodeId,
        operation: &PublicOperation,
    ) -> Option<ActionPresentation> {
        let PublicOperation::Action(name) = operation else {
            return None;
        };
        let node = self.nodes.get(&target)?;
        if self.boundary_nodes.contains(&target)
            || node.text_input_kind == Some(TextInputKind::Password)
        {
            return None;
        }
        let mut matches = node.actions.iter().filter(|action| action.name == *name);
        let action = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let input = node.role == crate::semantic::SemanticRole::TextInput
            || (node.text_input_kind == Some(TextInputKind::Plain)
                && node.capabilities.contains(&SemanticCapability::EditText));
        let input_activation = input && name.eq_ignore_ascii_case("activate");
        Some(ActionPresentation {
            version: "gui2tui.action-semantics.v1",
            display_label: if input_activation { format!("Input action: {name}") }
                else { format!("Action: {name}") },
            target_role: node.role.to_string(),
            effect_semantics: "provider_defined",
            interpretation_basis: "public_role_and_action_metadata",
            provider_description: action.description.clone(),
            provider_keybinding: action.keybinding.clone(),
            note: input_activation.then_some(
                "Submission/navigation is not established by this name.\nCheck public readback; this action targets the input control."),
        })
    }

    pub fn operation_label(&self, target: RuntimeNodeId) -> Option<OperationLabel> {
        let node = self.nodes.get(&target)?;
        if self.boundary_nodes.contains(&target) {
            return None;
        }
        if let Some(label) = node.name.as_deref().filter(|s| !s.trim().is_empty()) {
            return Some(OperationLabel {
                source_label: Some(label.to_owned()),
                display_label: label.to_owned(),
                origin: OperationLabelOrigin::PublicName,
                source_nodes: vec![target],
                region: None,
            });
        }
        fn find(region: &SemanticRegion, target: RuntimeNodeId) -> Option<&SemanticRegion> {
            if region.source_nodes.contains(&target) && region.label.is_some() {
                return Some(region);
            }
            region.children.iter().find_map(|child| find(child, target))
        }
        Some(match find(&self.regions.root, target) {
            Some(region) => OperationLabel {
                source_label: None,
                display_label: region.label.clone().expect("named region"),
                origin: OperationLabelOrigin::SemanticRegion,
                source_nodes: region.source_nodes.clone(),
                region: Some((region.id, region.confidence)),
            },
            None => OperationLabel {
                source_label: None,
                display_label: format!("Unnamed {}", node.role),
                origin: OperationLabelOrigin::RoleFallback,
                source_nodes: vec![target],
                region: None,
            },
        })
    }

    /// Reconstitute the privacy-checked source tree for existing Human compiler
    /// interfaces. This only adapts storage; it does not organize a second tree.
    pub fn semantic_tree(&self) -> crate::semantic::SemanticNode {
        fn visit(doc: &TuiDocument, id: RuntimeNodeId) -> crate::semantic::SemanticNode {
            let n = &doc.nodes[&id];
            crate::semantic::SemanticNode {
                runtime_id: id,
                backend_locator: n.backend_locator.clone(),
                index_in_parent: n.index_in_parent,
                role: n.role.clone(),
                name: n.name.clone(),
                description: n.description.clone(),
                value: n.value.clone(),
                text_input_kind: n.text_input_kind,
                states: n.states.clone(),
                actions: n.actions.clone(),
                capabilities: n.capabilities.clone(),
                children: n.children.iter().map(|id| visit(doc, *id)).collect(),
                truncations: n.truncations.clone(),
                debug: n.debug.clone(),
            }
        }
        let root = self
            .nodes
            .values()
            .find(|n| n.parent.is_none())
            .expect("validated source root");
        visit(self, root.runtime_id)
    }

    pub fn human_regions(&self, cache: &SemanticCache) -> RegionAnalysis {
        fn current(cache: &SemanticCache, mut id: RuntimeNodeId) -> bool {
            loop {
                let Some(n) = cache.node(id) else {
                    return false;
                };
                if super::analyze::is_noncurrent_surface(&n.states) {
                    return false;
                }
                // Closed anonymous popup placeholders are not useful Human rows.
                // Their source facts and public operations remain in the document/F3.
                let state = |value: &str| {
                    n.states.iter().any(
                        |s| matches!(s, crate::semantic::SemanticState::Other(v) if v == value),
                    )
                };
                if n.role == crate::semantic::SemanticRole::Menu
                    && n.name
                        .as_deref()
                        .is_none_or(|name| !super::presentation::has_display_text(name))
                    && n.children.is_empty()
                    && state("expandable")
                    && !state("showing")
                    && !state("visible")
                    && !n.states.contains(&crate::semantic::SemanticState::Expanded)
                    && !n.states.contains(&crate::semantic::SemanticState::Focused)
                {
                    return false;
                }
                match n.parent {
                    Some(parent) => id = parent,
                    None => return true,
                }
            }
        }
        fn filter(region: &mut SemanticRegion, cache: &SemanticCache) {
            region
                .children
                .retain(|child| child.source_nodes.iter().any(|id| current(cache, *id)));
            for child in &mut region.children {
                filter(child, cache)
            }
        }
        let mut analysis = self.regions.clone();
        filter(&mut analysis.root, cache);
        analysis
    }
}

/// Diagnostics contain identities and fixed categories, never source payloads.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("logical document validation failed: {kind} at {node:?}")]
pub struct DocumentError {
    pub kind: &'static str,
    pub node: Option<RuntimeNodeId>,
}

fn error(kind: &'static str, node: RuntimeNodeId) -> DocumentError {
    DocumentError {
        kind,
        node: Some(node),
    }
}

fn is_redacted(input: &DocumentInput<'_>, id: RuntimeNodeId) -> bool {
    let mut current = Some(id);
    while let Some(id) = current {
        let Some(node) = input.cache.node(id) else {
            return true;
        };
        if input.redacted.contains(&id) || node.text_input_kind == Some(TextInputKind::Password) {
            return true;
        }
        current = node.parent;
    }
    false
}

fn public_ids(input: &DocumentInput<'_>) -> HashSet<RuntimeNodeId> {
    input
        .cache
        .nodes()
        .filter(|node| {
            input.scopes.allows_node(node.runtime_id) && !is_redacted(input, node.runtime_id)
        })
        .map(|node| node.runtime_id)
        .collect()
}

fn relation_record(input: &DocumentInput<'_>, id: RuntimeNodeId) -> DocumentRelations {
    match input.cache.relation_state(id) {
        None | Some(RelationState::Unknown) => DocumentRelations::Unknown,
        Some(RelationState::Unavailable) => DocumentRelations::Unavailable,
        Some(RelationState::Known(relations)) => DocumentRelations::Known(
            relations
                .iter()
                .map(|r| DocumentRelation {
                    kind: r.kind.clone(),
                    targets: r
                        .targets
                        .iter()
                        .map(|target| match target.runtime_id {
                            Some(id) if is_redacted(input, id) => RelationEndpoint::Redacted,
                            Some(id) if input.scopes.allows_node(id) => RelationEndpoint::Node(id),
                            Some(id) => RelationEndpoint::OutsideScope(id),
                            None => RelationEndpoint::Unresolved(target.locator.clone()),
                        })
                        .collect(),
                })
                .collect(),
        ),
    }
}

fn validate_operation(
    input: &DocumentInput<'_>,
    op: &DocumentOperation,
) -> Result<(), DocumentError> {
    let node = input
        .cache
        .node(op.target)
        .ok_or_else(|| error("missing operation target", op.target))?;
    if is_redacted(input, op.target)
        || !input.scopes.allows_node(op.target)
        || node.backend_locator != op.locator
    {
        return Err(error("operation identity or scope", op.target));
    }
    let compatible = match &op.operation {
        PublicOperation::Action(name) => {
            !name.is_empty()
                && node.actions.iter().filter(|a| a.name == *name).count() == 1
                && op.parameter_domain == ParameterDomain::None
        }
        PublicOperation::EditText { multiline } => {
            node.capabilities.contains(&if *multiline {
                SemanticCapability::EditComplexText
            } else {
                SemanticCapability::EditText
            }) && op.parameter_domain == ParameterDomain::Text
        }
        PublicOperation::EditRange => {
            (node.capabilities.contains(&SemanticCapability::EditText)
                || node
                    .capabilities
                    .contains(&SemanticCapability::EditComplexText))
                && op.parameter_domain == ParameterDomain::TextRange
        }
        PublicOperation::ReadSelection => {
            (node.capabilities.contains(&SemanticCapability::EditText)
                || node
                    .capabilities
                    .contains(&SemanticCapability::EditComplexText))
                && op.parameter_domain == ParameterDomain::None
        }
        PublicOperation::SetValue => {
            node.capabilities.contains(&SemanticCapability::Value)
                && matches!(op.parameter_domain, ParameterDomain::Number(_))
        }
        PublicOperation::Value { .. } => {
            node.capabilities.contains(&SemanticCapability::Value)
                && op.parameter_domain == ParameterDomain::None
        }
        PublicOperation::Selection { parent, .. } => {
            node.parent
                .and_then(|id| input.cache.node(id))
                .is_some_and(|owner| {
                    owner.backend_locator == *parent
                        && input.scopes.allows_node(owner.runtime_id)
                        && !is_redacted(input, owner.runtime_id)
                        && owner
                            .capabilities
                            .contains(&SemanticCapability::SelectChildren)
                })
                && op.parameter_domain == ParameterDomain::None
        }
    };
    if !compatible {
        return Err(error("operation capability or parameter domain", op.target));
    }
    if let ParameterDomain::Number(ValueDomain::Known {
        current,
        min,
        max,
        step,
    }) = op.parameter_domain
        && (![current, min, max, step].iter().all(|v| v.is_finite()) || min > max || step <= 0.0)
    {
        return Err(error("invalid public value domain", op.target));
    }
    Ok(())
}

pub fn build_document(input: &DocumentInput<'_>) -> Result<TuiDocument, DocumentError> {
    input
        .cache
        .validate()
        .map_err(|_| error("invalid source cache", input.cache.root_id()))?;
    if input.observation.started_at_ms > input.observation.finished_at_ms {
        return Err(error("invalid collection interval", input.cache.root_id()));
    }
    let public = public_ids(input);
    for id in input.text_reads.keys() {
        if input.cache.node(*id).is_none() {
            return Err(error("missing text source", *id));
        }
    }
    for (id, read) in input.table_positions {
        if input
            .cache
            .node(*id)
            .is_none_or(|n| n.role != SemanticRole::Cell)
        {
            return Err(error("invalid table position source", *id));
        }
        if let crate::content::TablePositionRead::Known {
            row,
            column,
            row_span,
            column_span,
        } = read
            && (*row > i32::MAX as usize
                || *column > i32::MAX as usize
                || [row_span, column_span]
                    .iter()
                    .any(|span| span.is_some_and(|v| v == 0 || v > i32::MAX as usize)))
        {
            return Err(error("invalid public table position", *id));
        }
    }
    // Scope ownership can differ from AX containment: a modal Dialog may be a
    // sibling of its owning Window. Preserve that boundary without its payload.
    let mut scope_boundaries = HashSet::new();
    for id in &public {
        let mut scope = input.scopes.scope_for_node(*id);
        let mut seen = HashSet::new();
        while let Some(scope_id) = scope {
            if !seen.insert(scope_id) {
                return Err(error("scope parent cycle", *id));
            }
            let owner = input
                .scopes
                .scope(scope_id)
                .ok_or_else(|| error("missing source scope", *id))?;
            scope_boundaries.insert(owner.root);
            scope = owner.parent;
        }
    }
    let view = input.cache.organization_view(&public, &scope_boundaries);
    let tree = view
        .materialize_tree()
        .map_err(|_| error("invalid organization view", view.root_id()))?;
    let regions = analyze_document_regions(&tree, &RelationalSemanticGraph::new(&view));
    let mut seen_roots = HashSet::new();
    let mut roots = Vec::new();
    for id in input.content_roots {
        if input.cache.node(*id).is_none() {
            return Err(error("missing content source", *id));
        }
        if public.contains(id) {
            if !seen_roots.insert(*id) {
                return Err(error("duplicate content source", *id));
            }
            roots.push(*id);
        }
    }
    // A Human primary-only root list is not an acceptable source inventory.
    for id in &public {
        if input
            .cache
            .node(*id)
            .is_some_and(|node| node.role == SemanticRole::Document)
            && !seen_roots.contains(id)
        {
            return Err(error("unlisted public document source", *id));
        }
    }
    let mut content = ContentCatalog::analyze_document(&view, &roots);
    let mut sources: BTreeMap<_, _> = public
        .iter()
        .map(|id| (*id, SourceReferences::default()))
        .collect();
    let mut text_reads: BTreeMap<_, _> = input
        .text_reads
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, read)| (*id, read.clone()))
        .collect();
    for node in view
        .nodes()
        .filter(|node| public.contains(&node.runtime_id))
    {
        if node
            .debug
            .interfaces
            .iter()
            .any(|interface| interface == "Text")
        {
            text_reads.insert(
                node.runtime_id,
                input
                    .text_reads
                    .get(&node.runtime_id)
                    .cloned()
                    .unwrap_or(TextRead::NotRequested),
            );
        }
    }
    let mut value_domains: BTreeMap<_, _> = input
        .value_domains
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, v)| (*id, v.clone()))
        .collect();
    for id in &public {
        if input
            .cache
            .node(*id)
            .is_some_and(|node| node.debug.interfaces.iter().any(|i| i == "Value"))
        {
            value_domains.entry(*id).or_insert(ValueDomain::Unknown);
        }
    }
    // Keep the exact source read once. Models reference it by source; Summary
    // remains a labelled preview, never evidence of a complete Text read.
    let mut metadata_reads: BTreeMap<_, _> = input
        .metadata_reads
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, m)| (*id, m.clone()))
        .collect();
    let roots: Vec<_> = content.models().map(|model| model.root).collect();
    for root in roots {
        let index = content.by_root_index(root).expect("catalog root");
        let model = content.model_mut(index).expect("catalog model");
        if view
            .node(root)
            .is_some_and(|n| n.debug.interfaces.iter().any(|i| i == "Document"))
        {
            metadata_reads.entry(root).or_insert(MetadataRead::Unknown);
        }
        if let Some(MetadataRead::Known(probe)) = metadata_reads.get(&root) {
            model.metadata.locale = probe.locale.clone();
            model.metadata.current_page = probe.current_page;
            model.metadata.page_count = probe.page_count;
            model.metadata.attributes = probe.attributes.clone();
        }
        let ids: Vec<_> = model.blocks.ids().collect();
        for id in ids {
            let block = model.blocks.get_mut(id).expect("content block");
            if let Some(read) = text_reads.get(&block.source) {
                block.text = match read {
                    TextRead::Complete(text) => TextContentState::Loaded(text.clone()),
                    TextRead::NotRequested => TextContentState::Unknown,
                    TextRead::Unavailable => TextContentState::Unavailable,
                };
            }
            if let Some(refs) = sources.get_mut(&block.source) {
                refs.content.push((root, id));
            }
        }
    }
    let mut ids = HashSet::new();
    for op in input.operations {
        validate_operation(input, op)?;
        if op.id.is_empty() || !ids.insert(&op.id) {
            return Err(error("duplicate operation ID", op.target));
        }
        sources
            .get_mut(&op.target)
            .expect("validated public target")
            .operations
            .push(op.id.clone());
    }
    let mut scopes: Vec<_> = input
        .scopes
        .scopes()
        .filter(|scope| view.node(scope.root).is_some())
        .cloned()
        .collect();
    for scope in &mut scopes {
        if !public.contains(&scope.root) {
            scope.label = None;
        }
        scope.children.retain(|id| view.node(id.0).is_some());
    }
    scopes.sort_by_key(|scope| scope.root);
    let mut document = TuiDocument {
        decision_annotations: None,
        observation: input.observation.clone(),
        readable_scope: input.scopes.active(),
        invocable_scope: input.scopes.active(),
        scopes,
        nodes: view
            .nodes()
            .map(|node| (node.runtime_id, node.clone()))
            .collect(),
        boundary_nodes: view
            .nodes()
            .filter(|node| !public.contains(&node.runtime_id))
            .map(|node| node.runtime_id)
            .collect(),
        relations: public
            .iter()
            .map(|id| (*id, relation_record(input, *id)))
            .collect(),
        text_reads,
        value_domains,
        metadata_reads,
        table_positions: input
            .table_positions
            .iter()
            .filter(|(id, _)| public.contains(id))
            .map(|(id, read)| (*id, *read))
            .collect(),
        regions,
        content,
        tables: {
            let mut tables: Vec<_> = view
                .nodes()
                .filter(|n| public.contains(&n.runtime_id))
                .filter_map(|n| {
                    crate::content::SemanticTableStructure::analyze(
                        &view,
                        n.runtime_id,
                        input.table_positions,
                    )
                })
                .collect();
            tables.sort_by_key(|t| t.owner);
            tables
        },
        virtual_collections: {
            let mut collections: Vec<_> = view
                .nodes()
                .filter(|n| public.contains(&n.runtime_id))
                .filter_map(|n| {
                    crate::content::VirtualCollectionStructure::analyze(&view, n.runtime_id)
                })
                .filter(|c| c.completeness != CollectionCompleteness::Complete)
                .collect();
            collections.sort_by_key(|c| c.owner);
            collections
        },
        commands: CommandHierarchy::build(&view, input.scopes),
        choices: ChoiceCatalog::discover(&view),
        operations: input.operations.to_vec(),
        sources,
        completeness: DocumentCompleteness {
            complete_for_available_payloads: false,
            observed_coverage: ObservedCoverage::Unknown,
            gaps: Vec::new(),
        },
    };
    document.completeness = completeness(&document);
    validate_document(input, &document)?;
    Ok(document)
}

fn completeness(document: &TuiDocument) -> DocumentCompleteness {
    let mut gaps = Vec::new();
    let mut materialized = true;
    let mut partial = false;
    for (id, node) in &document.nodes {
        if document.boundary_nodes.contains(id) {
            continue;
        }
        if !node.truncations.is_empty() {
            partial = true;
            gaps.push(CoverageGap::TreeIncomplete(*id));
        }
        match collection_completeness(node) {
            CollectionCompleteness::PartialRealized => {
                partial = true;
                gaps.push(CoverageGap::PartialCollection(*id));
            }
            CollectionCompleteness::Unknown => gaps.push(CoverageGap::UnknownCollection(*id)),
            CollectionCompleteness::Complete => {}
        }
        match document.text_reads.get(id) {
            Some(TextRead::NotRequested) => {
                materialized = false;
                gaps.push(CoverageGap::TextNotRequested(*id));
            }
            Some(TextRead::Unavailable) => {
                partial = true;
                gaps.push(CoverageGap::TextUnavailable(*id));
            }
            _ => {}
        }
        match document.metadata_reads.get(id) {
            Some(MetadataRead::Unknown) => {
                materialized = false;
                gaps.push(CoverageGap::MetadataUnknown(*id));
            }
            Some(MetadataRead::Unavailable) => {
                partial = true;
                gaps.push(CoverageGap::MetadataUnavailable(*id));
            }
            Some(MetadataRead::Known(probe)) if !probe.unavailable_fields.is_empty() => {
                partial = true;
                gaps.push(CoverageGap::MetadataUnavailable(*id));
            }
            _ => {}
        }
        match document.value_domains.get(id) {
            Some(ValueDomain::Unknown) => {
                materialized = false;
                gaps.push(CoverageGap::ValueNotRequested(*id));
            }
            Some(ValueDomain::Unavailable) => {
                partial = true;
                gaps.push(CoverageGap::ValueUnavailable(*id));
            }
            _ => {}
        }
        match document.relations.get(id) {
            Some(DocumentRelations::Unknown) | None => {
                materialized = false;
                gaps.push(CoverageGap::RelationsUnknown(*id));
            }
            Some(DocumentRelations::Unavailable) => {
                partial = true;
                gaps.push(CoverageGap::RelationsUnavailable(*id));
            }
            Some(DocumentRelations::Known(relations)) => {
                if relations
                    .iter()
                    .flat_map(|r| &r.targets)
                    .any(|t| matches!(t, RelationEndpoint::Unresolved(_)))
                {
                    partial = true;
                    gaps.push(CoverageGap::UnresolvedRelation(*id));
                }
            }
        }
    }
    for table in &document.tables {
        for cell in &table.cells {
            if cell.row_span.is_none() || cell.column_span.is_none() {
                if document.table_positions.contains_key(&cell.source) {
                    partial = true;
                    gaps.push(CoverageGap::TableSpanUnavailable(cell.source));
                } else {
                    materialized = false;
                    gaps.push(CoverageGap::TableSpanNotRequested(cell.source));
                }
            }
            match cell.position_source {
                crate::content::TablePositionSource::NotRequested => {
                    materialized = false;
                    gaps.push(CoverageGap::TablePositionNotRequested(cell.source));
                }
                crate::content::TablePositionSource::Unavailable => {
                    partial = true;
                    gaps.push(CoverageGap::TablePositionUnavailable(cell.source));
                }
                _ => {}
            }
        }
    }
    for model in document.content.models() {
        match model.completeness {
            ContentCompleteness::Unknown => gaps.push(CoverageGap::ContentUnknown(model.root)),
            ContentCompleteness::PartialRealized => {
                partial = true;
                gaps.push(CoverageGap::ContentPartial(model.root));
            }
            ContentCompleteness::Complete => {}
        }
    }
    DocumentCompleteness {
        complete_for_available_payloads: materialized,
        observed_coverage: if partial {
            ObservedCoverage::Partial
        } else if gaps.is_empty() {
            ObservedCoverage::CompleteForDeclaredScope
        } else {
            ObservedCoverage::Unknown
        },
        gaps,
    }
}

/// Compare with the pre-organization inventory, rather than counting only what
/// survived the builder. Also usable on a mutated copy as a negative control.
pub fn validate_document(
    input: &DocumentInput<'_>,
    doc: &TuiDocument,
) -> Result<(), DocumentError> {
    let public = public_ids(input);
    // Independently enumerate ancestors from the source arena. Do not call the
    // projection used by construction: a projection omission must be detectable.
    let mut expected_ids = public.clone();
    expected_ids.insert(input.cache.root_id());
    for id in &public {
        let mut scope = input.scopes.scope_for_node(*id);
        let mut seen = HashSet::new();
        while let Some(scope_id) = scope {
            if !seen.insert(scope_id) {
                return Err(error("scope parent cycle", *id));
            }
            let owner = input
                .scopes
                .scope(scope_id)
                .ok_or_else(|| error("missing source scope", *id))?;
            expected_ids.insert(owner.root);
            scope = owner.parent;
        }
    }
    for id in expected_ids.clone() {
        let mut parent = input.cache.node(id).and_then(|n| n.parent);
        while let Some(id) = parent {
            expected_ids.insert(id);
            parent = input.cache.node(id).and_then(|n| n.parent);
        }
    }
    if doc.observation != input.observation
        || doc.readable_scope != input.scopes.active()
        || doc.invocable_scope != input.scopes.active()
    {
        return Err(error("observation identity", input.cache.root_id()));
    }
    if doc.nodes.len() != expected_ids.len() {
        return Err(error("node inventory", input.cache.root_id()));
    }
    if doc.boundary_nodes != expected_ids.difference(&public).copied().collect() {
        return Err(error("boundary inventory", input.cache.root_id()));
    }
    for id in &expected_ids {
        let source = input.cache.node(*id).expect("source ancestor");
        let node = doc
            .nodes
            .get(id)
            .ok_or_else(|| error("missing source node", *id))?;
        let children: Vec<_> = source
            .children
            .iter()
            .filter(|id| expected_ids.contains(id))
            .copied()
            .collect();
        if public.contains(id) {
            let mut expected = source.clone();
            expected.children = children;
            if node != &expected {
                return Err(error("source facts or ancestry", *id));
            }
        } else if node.runtime_id != *id
            || node.backend_locator != source.backend_locator
            || node.parent != source.parent
            || node.role != source.role
            || node.children != children
            || node.name.is_some()
            || node.description.is_some()
            || node.value.is_some()
            || !node.states.is_empty()
            || !node.actions.is_empty()
            || !node.capabilities.is_empty()
            || !node.truncations.is_empty()
            || node.debug != Default::default()
        {
            return Err(error("boundary payload or ancestry", *id));
        }
        if doc.boundary_nodes.contains(id) == public.contains(id) {
            return Err(error("boundary disclosure", source.runtime_id));
        }
    }
    if doc.relations.len() != public.len() || doc.sources.len() != public.len() {
        return Err(error("source inventory", input.cache.root_id()));
    }
    let mut text_count = 0;
    for id in &public {
        if doc.relations.get(id) != Some(&relation_record(input, *id)) {
            return Err(error("relation evidence", *id));
        }
        let source = input.cache.node(*id).expect("public source");
        if source.debug.interfaces.iter().any(|i| i == "Text") || input.text_reads.contains_key(id)
        {
            text_count += 1;
            if doc.text_reads.get(id)
                != Some(input.text_reads.get(id).unwrap_or(&TextRead::NotRequested))
            {
                return Err(error("full text payload", *id));
            }
        }
        let refs = doc
            .sources
            .get(id)
            .ok_or_else(|| error("missing source references", *id))?;
        let expected_ops: Vec<_> = input
            .operations
            .iter()
            .filter(|op| op.target == *id)
            .map(|op| op.id.clone())
            .collect();
        if refs.operations != expected_ops {
            return Err(error("operation references", *id));
        }
        for (root, block) in &refs.content {
            if doc
                .content
                .get(*root)
                .and_then(|model| model.block(*block))
                .is_none_or(|b| b.source != *id)
            {
                return Err(error("content reference", *id));
            }
        }
    }
    if doc.text_reads.len() != text_count {
        return Err(error("text inventory", input.cache.root_id()));
    }
    let expected_positions: BTreeMap<_, _> = input
        .table_positions
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, read)| (*id, *read))
        .collect();
    if doc.table_positions != expected_positions {
        return Err(error("table position inventory", input.cache.root_id()));
    }
    let mut expected_tables: Vec<_> = public
        .iter()
        .filter(|id| {
            input
                .cache
                .node(**id)
                .is_some_and(|n| n.role == SemanticRole::Table)
        })
        .copied()
        .collect();
    expected_tables.sort();
    if doc.tables.iter().map(|t| t.owner).collect::<Vec<_>>() != expected_tables {
        return Err(error("table inventory", input.cache.root_id()));
    }
    for table in &doc.tables {
        let source = input.cache.node(table.owner).expect("public table");
        let mut expected_cells = Vec::new();
        for id in &source.children {
            let Some(n) = input
                .cache
                .node(*id)
                .filter(|n| public.contains(&n.runtime_id))
            else {
                continue;
            };
            if n.role == SemanticRole::Cell {
                expected_cells.push(*id);
            }
            if n.role == SemanticRole::Row {
                expected_cells.extend(
                    n.children
                        .iter()
                        .filter(|id| {
                            public.contains(id)
                                && input
                                    .cache
                                    .node(**id)
                                    .is_some_and(|n| n.role == SemanticRole::Cell)
                        })
                        .copied(),
                );
            }
        }
        if table.cells.iter().map(|c| c.source).collect::<Vec<_>>() != expected_cells {
            return Err(error("table cell inventory", table.owner));
        }
        for cell in &table.cells {
            if cell.label_source.is_some_and(|id| !public.contains(&id)) {
                return Err(error("table label source", cell.source));
            }
            if let Some(crate::content::TablePositionRead::Known { row, column, .. }) =
                doc.table_positions.get(&cell.source)
                && (cell.row != Some(*row)
                    || cell.column != Some(*column)
                    || cell.position_source != crate::content::TablePositionSource::PublicTableCell)
            {
                return Err(error("public table position changed", cell.source));
            }
        }
    }
    let mut expected_collections: Vec<_> = public
        .iter()
        .filter(|id| {
            input.cache.node(**id).is_some_and(|n| {
                matches!(
                    n.role,
                    SemanticRole::List | SemanticRole::Tree | SemanticRole::Table
                ) && collection_completeness(n) != CollectionCompleteness::Complete
            })
        })
        .copied()
        .collect();
    expected_collections.sort();
    if doc
        .virtual_collections
        .iter()
        .map(|c| c.owner)
        .collect::<Vec<_>>()
        != expected_collections
    {
        return Err(error("collection inventory", input.cache.root_id()));
    }
    for collection in &doc.virtual_collections {
        let source = input
            .cache
            .node(collection.owner)
            .expect("public collection");
        let items: Vec<_> = source
            .children
            .iter()
            .filter(|id| public.contains(id))
            .copied()
            .collect();
        if collection.realized_items != items
            || collection.known_total.is_some()
            || collection
                .active_descendant
                .is_some_and(|id| !public.contains(&id))
        {
            return Err(error("collection source references", collection.owner));
        }
    }
    if doc.operations != input.operations {
        return Err(error(
            "operation inventory or binding",
            input.cache.root_id(),
        ));
    }
    let mut expected_values: BTreeMap<_, _> = input
        .value_domains
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, v)| (*id, v.clone()))
        .collect();
    for id in &public {
        if input
            .cache
            .node(*id)
            .is_some_and(|node| node.debug.interfaces.iter().any(|i| i == "Value"))
        {
            expected_values.entry(*id).or_insert(ValueDomain::Unknown);
        }
    }
    if doc.value_domains != expected_values {
        return Err(error("public value domains", input.cache.root_id()));
    }
    for op in &doc.operations {
        validate_operation(input, op)?;
    }
    fn region_refs(region: &SemanticRegion, doc: &TuiDocument) -> Result<(), DocumentError> {
        for id in region
            .source_nodes
            .iter()
            .chain(&region.logical_group)
            .chain(region.interactions.iter().map(|i| &i.source))
        {
            if !doc.nodes.contains_key(id) {
                return Err(error("region source reference", *id));
            }
        }
        for child in &region.children {
            region_refs(child, doc)?;
        }
        Ok(())
    }
    region_refs(&doc.regions.root, doc)?;
    let scope_ids: HashSet<_> = doc.scopes.iter().map(|scope| scope.id).collect();
    let expected_scopes: HashSet<_> = input
        .scopes
        .scopes()
        .filter(|scope| expected_ids.contains(&scope.root))
        .map(|scope| scope.id)
        .collect();
    if scope_ids != expected_scopes || doc.scopes.len() != expected_scopes.len() {
        return Err(error("scope inventory", input.cache.root_id()));
    }
    for scope in &doc.scopes {
        let original = input
            .scopes
            .scope(scope.id)
            .ok_or_else(|| error("scope identity", scope.root))?;
        let mut expected = original.clone();
        if !public.contains(&scope.root) {
            expected.label = None;
        }
        expected.children.retain(|id| expected_ids.contains(&id.0));
        if *scope != expected
            || !doc.nodes.contains_key(&scope.root)
            || scope.parent.is_some_and(|id| !scope_ids.contains(&id))
        {
            return Err(error("scope evidence", scope.root));
        }
    }
    fn command_refs(
        group: &CommandGroup,
        doc: &TuiDocument,
        scope_ids: &HashSet<InteractionScopeId>,
    ) -> Result<(), DocumentError> {
        if !doc.nodes.contains_key(&group.source) || !scope_ids.contains(&group.scope) {
            return Err(error("command group reference", group.source));
        }
        for entry in &group.children {
            match entry {
                CommandEntry::Group(group) => command_refs(group, doc, scope_ids)?,
                CommandEntry::Command(command) => {
                    if doc
                        .nodes
                        .get(&command.source)
                        .is_none_or(|node| node.backend_locator != command.backend_locator)
                        || !scope_ids.contains(&command.scope)
                    {
                        return Err(error("command source reference", command.source));
                    }
                }
            }
        }
        Ok(())
    }
    command_refs(&doc.commands.root, doc, &scope_ids)?;
    for choice in doc.choices.choices() {
        if !public.contains(&choice.owner)
            || choice.current.is_some_and(|id| !public.contains(&id))
            || choice
                .options
                .options()
                .iter()
                .any(|option| !public.contains(&option.runtime_id))
        {
            return Err(error("choice source reference", choice.owner));
        }
    }
    let expected_roots: HashSet<_> = input
        .content_roots
        .iter()
        .filter(|id| public.contains(id))
        .copied()
        .collect();
    let mut expected_metadata: BTreeMap<_, _> = input
        .metadata_reads
        .iter()
        .filter(|(id, _)| public.contains(id))
        .map(|(id, m)| (*id, m.clone()))
        .collect();
    for root in &expected_roots {
        if input
            .cache
            .node(*root)
            .is_some_and(|n| n.debug.interfaces.iter().any(|i| i == "Document"))
        {
            expected_metadata
                .entry(*root)
                .or_insert(MetadataRead::Unknown);
        }
    }
    if doc.metadata_reads != expected_metadata {
        return Err(error("document metadata evidence", input.cache.root_id()));
    }
    let actual_roots: HashSet<_> = doc.content.models().map(|model| model.root).collect();
    if actual_roots != expected_roots || doc.content.models().count() != expected_roots.len() {
        return Err(error("content source inventory", input.cache.root_id()));
    }
    for model in doc.content.models() {
        if !public.contains(&model.root) {
            return Err(error("content root reference", model.root));
        }
        if let Some(MetadataRead::Known(probe)) = doc.metadata_reads.get(&model.root) {
            if probe
                .locator
                .as_ref()
                .is_some_and(|locator| *locator != doc.nodes[&model.root].backend_locator)
                || model.metadata.locale != probe.locale
                || model.metadata.current_page != probe.current_page
                || model.metadata.page_count != probe.page_count
                || model.metadata.attributes != probe.attributes
            {
                return Err(error("content metadata binding", model.root));
            }
        }
        if model.roots.iter().any(|id| model.block(*id).is_none()) {
            return Err(error("content block root", model.root));
        }
        for block in model.ordered_blocks() {
            if !public.contains(&block.source)
                || block.children.iter().any(|id| model.block(*id).is_none())
                || block
                    .interactive_sources
                    .iter()
                    .any(|id| !public.contains(id))
            {
                return Err(error("content source reference", block.source));
            }
            if !doc
                .sources
                .get(&block.source)
                .is_some_and(|refs| refs.content.contains(&(model.root, block.id)))
            {
                return Err(error("missing content reference", block.source));
            }
            if let Some(read) = doc.text_reads.get(&block.source) {
                let valid = match (read, &block.text) {
                    (TextRead::Complete(a), TextContentState::Loaded(b)) => a == b,
                    (TextRead::NotRequested, TextContentState::Unknown)
                    | (TextRead::Unavailable, TextContentState::Unavailable) => true,
                    _ => false,
                };
                if !valid {
                    return Err(error("content text evidence", block.source));
                }
            }
        }
    }
    if doc.completeness != completeness(doc) {
        return Err(error("completeness evidence", input.cache.root_id()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
