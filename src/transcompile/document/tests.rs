use super::*;
use crate::{
    semantic::{
        BackendRelation, DebugInfo, SemanticAction, SemanticNode, SemanticRole, SemanticState,
    },
    transcompile::{SemanticRegionKind, analyze_regions_with_graph},
};

fn node(
    path: &str,
    role: SemanticRole,
    name: Option<&str>,
    children: Vec<SemanticNode>,
) -> SemanticNode {
    SemanticNode {
        runtime_id: RuntimeNodeId::new(0),
        backend_locator: BackendLocator::new(":1.90", path),
        index_in_parent: None,
        role,
        name: name.map(str::to_owned),
        description: None,
        value: None,
        text_input_kind: None,
        states: vec![SemanticState::Enabled],
        actions: Vec::new(),
        capabilities: Vec::new(),
        children,
        truncations: Vec::new(),
        debug: DebugInfo::default(),
    }
}

fn action(name: &str) -> SemanticAction {
    SemanticAction {
        index: 17,
        name: name.into(),
        description: None,
        keybinding: None,
    }
}

struct Fixture {
    cache: SemanticCache,
    scopes: InteractionScopes,
    redacted: HashSet<RuntimeNodeId>,
    reads: BTreeMap<RuntimeNodeId, TextRead>,
    values: BTreeMap<RuntimeNodeId, ValueDomain>,
    metadata: BTreeMap<RuntimeNodeId, MetadataRead>,
    positions: BTreeMap<RuntimeNodeId, crate::content::TablePositionRead>,
    roots: Vec<RuntimeNodeId>,
    operations: Vec<DocumentOperation>,
    observation: DocumentObservation,
}

impl Fixture {
    fn new(tree: SemanticNode) -> Self {
        let mut cache = SemanticCache::from_snapshot(tree).unwrap();
        let ids: Vec<_> = cache.nodes().map(|n| n.runtime_id).collect();
        for id in ids {
            cache.set_relations(id, Vec::new()).unwrap();
        }
        let scopes = InteractionScopes::analyze(&cache, &RelationalSemanticGraph::new(&cache));
        Self {
            cache,
            scopes,
            redacted: HashSet::new(),
            reads: BTreeMap::new(),
            values: BTreeMap::new(),
            metadata: BTreeMap::new(),
            positions: BTreeMap::new(),
            roots: Vec::new(),
            operations: Vec::new(),
            observation: DocumentObservation {
                session: RuntimeSessionId::default(),
                generation: ApplicationGenerationId(1),
                observation_id: 1,
                started_at_ms: 1,
                finished_at_ms: 2,
            },
        }
    }

    fn id(&self, path: &str) -> RuntimeNodeId {
        self.cache
            .runtime_id(&BackendLocator::new(":1.90", path))
            .unwrap()
    }

    fn input(&self) -> DocumentInput<'_> {
        DocumentInput {
            observation: self.observation.clone(),
            cache: &self.cache,
            scopes: &self.scopes,
            redacted: &self.redacted,
            text_reads: &self.reads,
            value_domains: &self.values,
            metadata_reads: &self.metadata,
            table_positions: &self.positions,
            content_roots: &self.roots,
            operations: &self.operations,
        }
    }

    fn op(&mut self, path: &str, name: &str) {
        let target = self.id(path);
        self.operations.push(DocumentOperation {
            id: format!("operation-{}", self.operations.len()),
            target,
            locator: self.cache.node(target).unwrap().backend_locator.clone(),
            operation: PublicOperation::Action(name.into()),
            parameter_domain: ParameterDomain::None,
        });
    }
}

fn full_fixture() -> Fixture {
    let mut body = node("/a/body", SemanticRole::Paragraph, None, Vec::new());
    body.debug.interfaces.push("Text".into());
    body.value = Some("preview only".into());
    let mut button = node(
        "/a/remove",
        SemanticRole::Button,
        Some("Remove"),
        Vec::new(),
    );
    button.actions = vec![action("press"), action("show menu")];
    // No Showing/Visible. Legacy Human surface policy regards this as noncurrent.
    button.states.extend([
        SemanticState::Other("sensitive".into()),
        SemanticState::Other("focusable".into()),
    ]);
    let mut deep = button;
    for depth in (0..12).rev() {
        deep = node(
            &format!("/a/wrapper{depth}"),
            SemanticRole::Container,
            None,
            vec![deep],
        );
    }
    let a = node(
        "/a",
        SemanticRole::Document,
        Some("Report A"),
        vec![body, deep],
    );
    let mut b_body = node("/b/body", SemanticRole::Paragraph, None, Vec::new());
    b_body.debug.interfaces.push("Text".into());
    let mut b_button = node(
        "/b/remove",
        SemanticRole::Button,
        Some("Remove"),
        Vec::new(),
    );
    b_button.actions = vec![action("press")];
    let b = node(
        "/b",
        SemanticRole::Document,
        Some("Report B"),
        vec![b_body, b_button],
    );
    let mut slider = node("/value", SemanticRole::Slider, None, Vec::new());
    slider.capabilities.push(SemanticCapability::Value);
    let mut edit = node("/edit", SemanticRole::TextInput, None, Vec::new());
    edit.capabilities.push(SemanticCapability::EditText);
    let mut fixture = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("Workspace"),
        vec![a, b, slider, edit],
    ));
    fixture.roots = vec![fixture.id("/a"), fixture.id("/b")];
    fixture.reads.insert(
        fixture.id("/a/body"),
        TextRead::Complete(format!("{}尾部🧪", "line\n".repeat(240))),
    );
    fixture.reads.insert(
        fixture.id("/b/body"),
        TextRead::Complete("Report B complete tail".into()),
    );
    fixture.op("/a/remove", "press");
    fixture.op("/a/remove", "show menu");
    fixture.op("/b/remove", "press");
    for (path, operation, parameter_domain) in [
        (
            "/value",
            PublicOperation::SetValue,
            ParameterDomain::Number(ValueDomain::Known {
                current: 2.0,
                min: 0.0,
                max: 10.0,
                step: 0.5,
            }),
        ),
        (
            "/edit",
            PublicOperation::EditText { multiline: false },
            ParameterDomain::Text,
        ),
    ] {
        let target = fixture.id(path);
        fixture.operations.push(DocumentOperation {
            id: format!("operation-{}", fixture.operations.len()),
            target,
            locator: fixture.cache.node(target).unwrap().backend_locator.clone(),
            operation,
            parameter_domain,
        });
    }
    fixture
}

#[test]
fn cached_value_completeness_requires_same_source_complete_public_text() {
    let fixture = full_fixture();
    let id = fixture.id("/a/body");
    let mut document = build_document(&fixture.input()).unwrap();
    let value_flag = |doc: &TuiDocument| {
        wire::to_agent_document(doc)["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == id.get())
            .unwrap()["value_complete"]
            .clone()
    };
    assert_eq!(value_flag(&document), serde_json::json!(false));
    let original_cache = document.nodes[&id].value.clone();
    document
        .text_reads
        .insert(id, TextRead::Complete(original_cache.clone().unwrap()));
    assert_eq!(value_flag(&document), serde_json::json!(true));
    document.text_reads.insert(id, TextRead::Unavailable);
    assert_eq!(value_flag(&document), serde_json::Value::Null);
    assert_eq!(document.nodes[&id].value, original_cache);
}

#[test]
fn complete_sources_survive_multiple_documents_deep_ancestry_and_noncurrent_policy() {
    let fixture = full_fixture();
    // Independent denominator: the source cache, explicit root/read/operation
    // inventory above, not document output or scene reachability.
    let input = fixture.input();
    let document = build_document(&input).unwrap();
    assert_eq!(document.nodes.len(), fixture.cache.node_count());
    for source in fixture.cache.nodes() {
        assert_eq!(&document.nodes[&source.runtime_id], source);
    }
    assert_eq!(document.operations, fixture.operations);
    assert_eq!(document.content.models().count(), 2);
    assert_eq!(document.text_reads, fixture.reads);
    assert!(
        matches!(&document.text_reads[&fixture.id("/a/body")], TextRead::Complete(text) if text.ends_with("尾部🧪"))
    );
    assert!(document.completeness.complete_for_available_payloads);
    assert_eq!(
        document.completeness.observed_coverage,
        ObservedCoverage::CompleteForDeclaredScope
    );
    fn contains(region: &SemanticRegion, target: RuntimeNodeId) -> bool {
        region.source_nodes.contains(&target)
            || region.children.iter().any(|child| contains(child, target))
    }
    let target = fixture.id("/a/remove");
    assert!(contains(&document.regions.root, target));
    let tree = fixture.cache.materialize_tree().unwrap();
    let human = analyze_regions_with_graph(&tree, &RelationalSemanticGraph::new(&fixture.cache));
    assert!(!contains(&human.root, target));
    // Complete ancestry is in the source table even if grouping collapses a wrapper.
    let mut parent = document.nodes[&target].parent;
    let mut count = 0;
    while let Some(id) = parent {
        count += 1;
        parent = document.nodes[&id].parent;
    }
    assert_eq!(count, 14);
    assert_eq!(document.sources[&target].operations.len(), 2);
}

#[test]
fn sensitive_sources_are_removed_before_relationship_labels_content_and_commands() {
    let mut secret = node(
        "/password",
        SemanticRole::TextInput,
        Some("SECRET_PAYLOAD"),
        vec![node(
            "/password/child",
            SemanticRole::Label,
            Some("SECRET_CHILD"),
            Vec::new(),
        )],
    );
    secret.text_input_kind = Some(TextInputKind::Password);
    secret.actions.push(action("SECRET_ACTION"));
    let mut public = node("/public", SemanticRole::TextInput, None, Vec::new());
    public.capabilities.push(SemanticCapability::EditText);
    let redacted = node(
        "/redacted",
        SemanticRole::Document,
        Some("SECRET_REDACTED"),
        vec![node(
            "/redacted/child",
            SemanticRole::Label,
            Some("SECRET_DESCENDANT"),
            Vec::new(),
        )],
    );
    let mut fixture = Fixture::new(node(
        "/root",
        SemanticRole::Window,
        Some("Workspace"),
        vec![secret, public, redacted],
    ));
    fixture.redacted.insert(fixture.id("/redacted"));
    fixture.roots.push(fixture.id("/redacted"));
    fixture.reads.insert(
        fixture.id("/password/child"),
        TextRead::Complete("SECRET_TEXT".into()),
    );
    let mut runtime = crate::runtime::RuntimeSession::default();
    fixture.observation.generation =
        runtime.open_application(BackendLocator::new(":1.90", "/root"));
    fixture.observation.session = runtime.id.clone();
    for kind in [
        SemanticRelationKind::LabelledBy,
        SemanticRelationKind::DescribedBy,
        SemanticRelationKind::ErrorMessage,
        SemanticRelationKind::MemberOf,
    ] {
        fixture
            .cache
            .set_relations(
                fixture.id("/public"),
                vec![BackendRelation {
                    kind,
                    targets: vec![BackendLocator::new(":1.90", "/password/child")],
                }],
            )
            .unwrap();
        let mut document = build_document(&fixture.input()).unwrap();
        super::annotations::apply(&mut document, super::annotations::AnnotationMode::Contrast);
        let refs = document
            .nodes
            .values()
            .map(|n| {
                (
                    n.runtime_id,
                    runtime.source_locator_ref(&n.backend_locator).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let augmented = wire::to_agent_document_with_source_refs(&document, &refs).unwrap();
        assert!(!augmented.to_string().contains("SECRET_"));
        assert_eq!(
            augmented["nodes"].as_array().unwrap().len(),
            document.nodes.len()
        );
        assert!(
            !wire::to_agent_document(&document)
                .to_string()
                .contains("SECRET_")
        );
        assert!(!format!("{document:?}").contains("SECRET_"));
        assert!(!document.nodes.contains_key(&fixture.id("/password")));
        assert!(!document.nodes.contains_key(&fixture.id("/password/child")));
        assert!(!document.nodes.contains_key(&fixture.id("/redacted/child")));
        assert!(
            matches!(&document.relations[&fixture.id("/public")], DocumentRelations::Known(relations)
            if relations[0].targets == vec![RelationEndpoint::Redacted])
        );
    }
}

#[test]
fn source_locator_refs_preserve_wire_and_survive_snapshot_id_changes() {
    let mut runtime = crate::runtime::RuntimeSession::default();
    let generation = runtime.open_application(BackendLocator::new(":1.90", "/window"));
    let mut first = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        None,
        vec![node("/a", SemanticRole::Label, Some("public"), vec![])],
    ));
    let mut second = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        None,
        vec![
            node("/extra", SemanticRole::Label, None, vec![]),
            node("/a", SemanticRole::Label, Some("public"), vec![]),
        ],
    ));
    first.observation.session = runtime.id.clone();
    second.observation.session = runtime.id.clone();
    first.observation.generation = generation;
    second.observation.generation = generation;
    second.observation.observation_id = 2;
    assert_ne!(first.id("/a"), second.id("/a"));
    let mut target_refs = Vec::new();
    for fixture in [&first, &second] {
        let doc = build_document(&fixture.input()).unwrap();
        let refs = doc
            .nodes
            .values()
            .map(|n| {
                (
                    n.runtime_id,
                    runtime.source_locator_ref(&n.backend_locator).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        target_refs.push(refs[&fixture.id("/a")].clone());
        let original = wire::to_agent_document(&doc);
        let mut augmented = wire::to_agent_document_with_source_refs(&doc, &refs).unwrap();
        assert_eq!(
            augmented["source_identity"]["version"],
            wire::SOURCE_IDENTITY_VERSION
        );
        assert!(!augmented.to_string().contains("atspi1_"));
        augmented.as_object_mut().unwrap().remove("source_identity");
        for n in augmented["nodes"].as_array_mut().unwrap() {
            assert!(n.as_object_mut().unwrap().remove("locator_ref").is_some());
        }
        assert_eq!(augmented, original);
        let mut missing = refs.clone();
        missing.pop_first();
        assert!(wire::to_agent_document_with_source_refs(&doc, &missing).is_err());
        let mut aliased = refs.clone();
        let ids = refs.keys().copied().collect::<Vec<_>>();
        aliased.insert(ids[0], refs[&ids[1]].clone());
        aliased.insert(ids[1], refs[&ids[0]].clone());
        assert!(wire::to_agent_document_with_source_refs(&doc, &aliased).is_err());
        let mut foreign = crate::runtime::RuntimeSession::default();
        foreign.open_application(BackendLocator::new(":1.90", "/window"));
        let mut wrong_namespace = refs.clone();
        wrong_namespace.insert(
            ids[0],
            foreign
                .source_locator_ref(&doc.nodes[&ids[0]].backend_locator)
                .unwrap(),
        );
        assert!(wire::to_agent_document_with_source_refs(&doc, &wrong_namespace).is_err());
    }
    assert_eq!(target_refs[0], target_refs[1]);
}

#[test]
fn independent_inventory_detects_missing_payload_endpoint_and_wrong_binding() {
    let fixture = full_fixture();
    let input = fixture.input();
    let document = build_document(&input).unwrap();
    let mut broken = document.clone();
    broken.nodes.remove(&fixture.id("/a/wrapper0"));
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "node inventory"
    );
    let mut broken = document.clone();
    broken.text_reads.insert(
        fixture.id("/a/body"),
        TextRead::Complete("truncated".into()),
    );
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "full text payload"
    );
    let mut broken = document.clone();
    broken.operations[0].locator = fixture
        .cache
        .node(fixture.id("/b/remove"))
        .unwrap()
        .backend_locator
        .clone();
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "operation inventory or binding"
    );
    let mut broken = document.clone();
    broken.content = ContentCatalog::default();
    assert!(validate_document(&input, &broken).is_err());
    let mut broken = document.clone();
    broken
        .regions
        .root
        .source_nodes
        .push(RuntimeNodeId::new(u64::MAX));
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "region source reference"
    );
    let mut broken = document.clone();
    broken.relations.insert(
        fixture.id("/a/remove"),
        DocumentRelations::Known(vec![DocumentRelation {
            kind: SemanticRelationKind::MemberOf,
            targets: vec![RelationEndpoint::Node(RuntimeNodeId::new(u64::MAX))],
        }]),
    );
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "relation evidence"
    );
    let mut wrong = full_fixture();
    wrong.operations[0].target = wrong.id("/b/remove");
    assert_eq!(
        build_document(&wrong.input()).unwrap_err().kind,
        "operation identity or scope"
    );
    let mut missing_root = full_fixture();
    missing_root.roots.pop();
    assert_eq!(
        build_document(&missing_root.input()).unwrap_err().kind,
        "unlisted public document source"
    );
}

#[test]
fn unknown_unavailable_and_partial_remain_distinct_without_hiding_operations() {
    let mut fixture = full_fixture();
    let a = fixture.id("/a/body");
    let b = fixture.id("/b/body");
    fixture.reads.remove(&a);
    fixture.reads.insert(b, TextRead::Unavailable);
    fixture.cache.invalidate_relations(a);
    fixture.cache.mark_relations_unavailable(b).unwrap();
    fixture
        .cache
        .set_relations(
            fixture.id("/b/remove"),
            vec![BackendRelation {
                kind: SemanticRelationKind::MemberOf,
                targets: vec![BackendLocator::new(":1.90", "/not_in_observation")],
            }],
        )
        .unwrap();
    let document = build_document(&fixture.input()).unwrap();
    assert_eq!(document.text_reads[&a], TextRead::NotRequested);
    assert_eq!(document.text_reads[&b], TextRead::Unavailable);
    assert_eq!(document.relations[&a], DocumentRelations::Unknown);
    assert_eq!(document.relations[&b], DocumentRelations::Unavailable);
    assert!(!document.completeness.complete_for_available_payloads);
    assert_eq!(
        document.completeness.observed_coverage,
        ObservedCoverage::Partial
    );
    assert_eq!(document.operations, fixture.operations);
    fixture.reads.insert(a, TextRead::Complete(String::new()));
    fixture.cache.set_relations(a, Vec::new()).unwrap();
    let document = build_document(&fixture.input()).unwrap();
    assert!(document.completeness.complete_for_available_payloads);
    assert_eq!(
        document.completeness.observed_coverage,
        ObservedCoverage::Partial
    );
    fixture.operations[3].parameter_domain = ParameterDomain::Number(ValueDomain::Unknown);
    assert_eq!(
        build_document(&fixture.input()).unwrap().operations,
        fixture.operations
    );
    let mut list = node(
        "/list",
        SemanticRole::List,
        Some("Realized items"),
        Vec::new(),
    );
    list.states
        .push(SemanticState::Other("manages-descendants".into()));
    let virtualized = Fixture::new(list);
    let doc = build_document(&virtualized.input()).unwrap();
    assert!(doc.completeness.complete_for_available_payloads);
    assert_eq!(
        doc.completeness.observed_coverage,
        ObservedCoverage::Partial
    );
}

#[test]
fn modal_scope_keeps_boundary_identity_without_outside_payload_or_operations() {
    let mut modal = node(
        "/dialog",
        SemanticRole::Dialog,
        Some("Current dialog"),
        vec![node(
            "/dialog/control",
            SemanticRole::Label,
            Some("Current label"),
            Vec::new(),
        )],
    );
    modal.states.extend([
        SemanticState::Other("showing".into()),
        SemanticState::Other("modal".into()),
    ]);
    let mut outside = node(
        "/outside",
        SemanticRole::Button,
        Some("OUTSIDE_PAYLOAD"),
        Vec::new(),
    );
    outside.actions.push(action("press"));
    let mut fixture = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("OUTSIDE_SHELL"),
        vec![outside, modal],
    ));
    fixture
        .cache
        .set_relations(
            fixture.id("/dialog/control"),
            vec![BackendRelation {
                kind: SemanticRelationKind::MemberOf,
                targets: vec![BackendLocator::new(":1.90", "/outside")],
            }],
        )
        .unwrap();
    let doc = build_document(&fixture.input()).unwrap();
    assert!(!format!("{doc:?}").contains("OUTSIDE_"));
    assert!(doc.boundary_nodes.contains(&fixture.id("/window")));
    assert_eq!(doc.nodes[&fixture.id("/window")].role, SemanticRole::Window);
    assert_eq!(doc.nodes[&fixture.id("/dialog")].role, SemanticRole::Dialog);
    assert!(
        matches!(&doc.relations[&fixture.id("/dialog/control")], DocumentRelations::Known(relations)
        if relations[0].targets == vec![RelationEndpoint::OutsideScope(fixture.id("/outside"))])
    );
    fixture.op("/outside", "press");
    assert_eq!(
        build_document(&fixture.input()).unwrap_err().kind,
        "operation identity or scope"
    );
}

#[test]
fn sibling_modal_preserves_owning_scope_boundary_without_reading_its_content() {
    let window = node(
        "/window",
        SemanticRole::Window,
        Some("OUTSIDE_OWNER_PAYLOAD"),
        vec![node(
            "/window/content",
            SemanticRole::Label,
            Some("OUTSIDE_BODY_PAYLOAD"),
            Vec::new(),
        )],
    );
    let mut dialog = node(
        "/dialog",
        SemanticRole::Dialog,
        Some("Current dialog"),
        vec![node(
            "/dialog/control",
            SemanticRole::Label,
            Some("Current label"),
            Vec::new(),
        )],
    );
    dialog.states.extend([
        SemanticState::Other("showing".into()),
        SemanticState::Other("modal".into()),
    ]);
    let fixture = Fixture::new(node(
        "/app",
        SemanticRole::Application,
        Some("OUTSIDE_APP_PAYLOAD"),
        vec![window, dialog],
    ));
    let doc = build_document(&fixture.input()).unwrap();
    assert!(doc.boundary_nodes.contains(&fixture.id("/window")));
    assert!(!doc.nodes.contains_key(&fixture.id("/window/content")));
    assert_eq!(doc.nodes[&fixture.id("/window")].name, None);
    assert!(!format!("{doc:?}").contains("OUTSIDE_"));
    assert_eq!(
        doc.scopes
            .iter()
            .find(|s| s.root == fixture.id("/dialog"))
            .unwrap()
            .parent,
        Some(InteractionScopeId(fixture.id("/window")))
    );
    validate_document(&fixture.input(), &doc).unwrap();
}

#[test]
fn human_window_policy_cannot_modify_the_document_or_operation_inventory() {
    use super::super::{
        SpatialEvidenceIndex, SpatialProbeBudget, compile_scene, infer_layout,
        realize_responsive_layout,
    };
    let fixture = full_fixture();
    let input = fixture.input();
    let doc = build_document(&input).unwrap();
    let before = format!("{doc:?}");
    let tree = fixture.cache.materialize_tree().unwrap();
    let _scene = compile_scene(&tree, &doc.regions);
    let evidence = SpatialEvidenceIndex::from_tree(
        &tree,
        fixture.observation.generation,
        SpatialProbeBudget::default(),
    );
    let layout = infer_layout(&doc.regions, &tree, &evidence);
    let wide = realize_responsive_layout(&layout.plan, 140, 40, None);
    let narrow = realize_responsive_layout(
        &layout.plan,
        32,
        10,
        layout.plan.regions.first().map(|region| region.id),
    );
    assert_ne!(wide.class, narrow.class);
    // Layout and renderer consumers borrow the document; neither is a builder
    // input. This checks source retention through an actual Human projection.
    validate_document(&input, &doc).unwrap();
    assert_eq!(format!("{doc:?}"), before);
    assert_eq!(doc.operations.len(), 5);
    let reader_catalog = doc.content.for_human(&fixture.cache);
    assert!(reader_catalog.get(fixture.id("/a")).is_some());
    assert!(reader_catalog.get(fixture.id("/b")).is_some());
    assert!(doc.regions.root.kind == SemanticRegionKind::ApplicationShell);
}

#[test]
fn wire_is_complete_and_deterministic_with_shared_payloads_and_label_provenance() {
    let mut fixture = full_fixture();
    fixture.values.insert(
        fixture.id("/value"),
        ValueDomain::Known {
            current: 2.0,
            min: 0.0,
            max: 10.0,
            step: 0.5,
        },
    );
    fixture.metadata.insert(
        fixture.id("/a"),
        MetadataRead::Known(crate::backend::DocumentProbe {
            locator: Some(
                fixture
                    .cache
                    .node(fixture.id("/a"))
                    .unwrap()
                    .backend_locator
                    .clone(),
            ),
            document_interface: true,
            locale: Some("en".into()),
            page_count: Some(2),
            unavailable_fields: vec!["current_page"],
            ..Default::default()
        }),
    );
    let input = fixture.input();
    let document = build_document(&input).unwrap();
    let wire = super::wire::to_agent_document(&document);
    assert_eq!(wire["version"], super::wire::DOCUMENT_VERSION);
    assert_eq!(
        wire["nodes"].as_array().unwrap().len(),
        fixture.cache.node_count()
    );
    assert_eq!(wire["content"].as_array().unwrap().len(), 2);
    assert_eq!(
        wire["operations"].as_array().unwrap().len(),
        fixture.operations.len()
    );
    assert!(wire["text_payloads"].as_array().unwrap().iter().any(|p| {
        p["read"]["text"]
            .as_str()
            .is_some_and(|s| s.ends_with("尾部🧪"))
    }));
    assert_eq!(wire["operations"][3]["parameter_domain"]["step"], 0.5);
    assert_eq!(
        wire["operations"][0]["label_provenance"]["origin"],
        "public_name"
    );
    assert_eq!(
        wire["document_probes"][0]["read"]["unavailable_fields"][0],
        "current_page"
    );
    assert_eq!(
        document.completeness.observed_coverage,
        ObservedCoverage::Partial
    );
    assert!(!wire.to_string().contains("atspi1_"));
    assert_eq!(
        wire,
        super::wire::to_agent_document(&build_document(&input).unwrap())
    );
    let mut broken = document.clone();
    let index = broken.content.by_root_index(fixture.id("/a")).unwrap();
    broken.content.model_mut(index).unwrap().metadata.locale = Some("wrong".into());
    assert_eq!(
        validate_document(&input, &broken).unwrap_err().kind,
        "content metadata binding"
    );
}

#[test]
fn public_catalog_keeps_parameterized_operations_and_exposes_ambiguous_source_actions() {
    let fixture = full_fixture();
    let operations = public_operations(&fixture.cache, &fixture.scopes, &fixture.values);
    assert!(
        operations
            .iter()
            .any(|op| op.target == fixture.id("/edit")
                && op.parameter_domain == ParameterDomain::Text)
    );
    assert!(
        operations
            .iter()
            .any(|op| matches!(op.operation, PublicOperation::EditRange))
    );
    assert!(operations.iter().any(|op| matches!(
        op.parameter_domain,
        ParameterDomain::Number(ValueDomain::Unknown)
    )));
    let mut button = node("/same", SemanticRole::Button, Some("Remove"), Vec::new());
    button.actions = vec![action("press"), action("press"), action("show menu")];
    let duplicate = Fixture::new(button);
    let operations = public_operations(&duplicate.cache, &duplicate.scopes, &duplicate.values);
    assert_eq!(operations.len(), 1);
    assert_eq!(
        operations[0].operation,
        PublicOperation::Action("show menu".into())
    );
    assert_eq!(
        duplicate
            .cache
            .node(duplicate.cache.root_id())
            .unwrap()
            .actions
            .len(),
        3
    );
}

#[test]
fn frozen_public_projection_reuses_the_shared_annotation_organizer() {
    use super::annotations::{self, AnnotationMode};
    use serde_json::json;
    let fixture = full_fixture();
    let mut doc = build_document(&fixture.input()).unwrap();
    let original = wire::to_agent_document(&doc);
    for mode in [
        AnnotationMode::Off,
        AnnotationMode::Relations,
        AnnotationMode::Contrast,
    ] {
        annotations::apply(&mut doc, mode);
        assert_eq!(
            annotations::from_public_projection(&original, mode).unwrap(),
            doc.decision_annotations
        );
    }
    let unchanged = wire::to_agent_document(&build_document(&fixture.input()).unwrap());
    assert_eq!(original, unchanged);
    let mut broken = original.clone();
    broken["operations"][0]["target"] = json!(u64::MAX);
    assert!(annotations::from_public_projection(&broken, AnnotationMode::Contrast).is_err());
    broken = original.clone();
    broken["nodes"][0]["redacted"] = json!(true);
    assert!(annotations::from_public_projection(&broken, AnnotationMode::Relations).is_err());
    broken = original.clone();
    broken["text_payloads"][0]["source"] = json!(u64::MAX);
    assert!(annotations::from_public_projection(&broken, AnnotationMode::Contrast).is_err());
    assert!(annotations::from_public_projection(&json!({}), AnnotationMode::Contrast).is_err());
}

#[test]
fn research_annotations_preserve_all_facts_and_prefer_owned_content_on_ties() {
    use super::annotations::{self, AnnotationMode};
    let mut fixture = full_fixture();
    // Both ownership and an incidental counter separate this pair. Coverage
    // alone cannot prove task relevance; the declared tie order uses ownership.
    for (path, value) in [("/a/remove", "count=1"), ("/b/remove", "count=2")] {
        let old = fixture.cache.node(fixture.id(path)).unwrap();
        let mut fresh = node(path, old.role.clone(), old.name.as_deref(), Vec::new());
        fresh.states = old.states.clone();
        fresh.actions = old.actions.clone();
        fresh.value = Some(value.into());
        fixture.cache.refresh_node(fresh).unwrap();
    }
    let mut doc = build_document(&fixture.input()).unwrap();
    let original = wire::to_agent_document(&doc);
    annotations::apply(&mut doc, AnnotationMode::Relations);
    let contexts = doc.decision_annotations.as_ref().unwrap()["contexts"].clone();
    assert_eq!(
        doc.decision_annotations.as_ref().unwrap()["version"],
        "tui.decision_annotations.v2"
    );
    let canonical_nodes: BTreeMap<_, _> = original["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_u64().unwrap(), n))
        .collect();
    let canonical_reads: BTreeMap<_, _> = original["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["source"].as_u64().unwrap(), r))
        .collect();
    for context in contexts.as_array().unwrap() {
        let target = RuntimeNodeId::new(context["target"].as_u64().unwrap());
        let mut path = vec![target];
        let mut parent = fixture.cache.node(target).unwrap().parent;
        while let Some(id) = parent {
            path.push(id);
            parent = fixture.cache.node(id).unwrap().parent;
        }
        path.reverse();
        let expected_ancestors: Vec<_> = path[..path.len() - 1].iter().map(|id| id.get()).collect();
        let actual_ancestors: Vec<_> = context["ancestors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                assert_eq!(r.as_object().unwrap().len(), 1);
                let id = r["node_ref"].as_u64().unwrap();
                assert!(canonical_nodes.contains_key(&id));
                id
            })
            .collect();
        assert_eq!(actual_ancestors, expected_ancestors);
        let relation_sources: Vec<_> = context["relations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|relation| {
                let source = relation["source"].as_u64().unwrap();
                assert_eq!(relation["read"]["relation_ref"], source);
                let read = canonical_reads[&source];
                let expected: std::collections::BTreeSet<_> = read["edges"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|e| e["targets"].as_array().into_iter().flatten())
                    .filter_map(|e| e["node"].as_u64())
                    .filter(|id| canonical_nodes.contains_key(id))
                    .collect();
                let actual: std::collections::BTreeSet<_> = relation["endpoints"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["node_ref"].as_u64().unwrap())
                    .collect();
                assert_eq!(actual, expected);
                source
            })
            .collect();
        assert_eq!(
            relation_sources,
            path.iter().map(|id| id.get()).collect::<Vec<_>>()
        );
    }
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    validate_document(&fixture.input(), &doc).unwrap();
    let note = doc.decision_annotations.as_ref().unwrap();
    assert_eq!(note["contexts"], contexts);
    assert_eq!(note["cohorts"].as_array().unwrap().len(), 1);
    assert_eq!(
        note["cohorts"][0]["distinguishing_facts"][0]["feature"],
        "content_owner"
    );
    assert_eq!(note["cohorts"][0]["task_relevance"], "not_evaluated");
    assert!(
        annotations::human_hint(&doc, fixture.id("/a/remove"))
            .unwrap()
            .contains("Report A")
    );
    let mut annotated = wire::to_agent_document(&doc);
    annotated
        .as_object_mut()
        .unwrap()
        .remove("decision_annotations");
    assert_eq!(annotated, original);
    doc.observation.observation_id = 23;
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    assert_eq!(
        doc.decision_annotations.as_ref().unwrap()["observation_id"],
        23
    );
    annotations::apply(&mut doc, AnnotationMode::Off);
    assert!(doc.decision_annotations.is_none());
}

fn equal_remove_fixture(values: [Option<&str>; 2]) -> Fixture {
    let children = ["/a", "/b"]
        .into_iter()
        .zip(values)
        .map(|(path, value)| {
            let mut button = node(path, SemanticRole::Button, Some("Remove"), Vec::new());
            button.actions = vec![action("press")];
            button.value = value.map(str::to_owned);
            button
        })
        .collect();
    let mut f = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("Workspace"),
        children,
    ));
    f.op("/a", "press");
    f.op("/b", "press");
    f
}

#[test]
fn incidental_values_do_not_claim_task_relevance_and_ids_do_not_resolve_peers() {
    use super::annotations::{self, AnnotationMode};
    let f = equal_remove_fixture([Some("count=1"), Some("count=2")]);
    let mut doc = build_document(&f.input()).unwrap();
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    let cohort = &doc.decision_annotations.as_ref().unwrap()["cohorts"][0];
    assert_eq!(cohort["distinguishing_facts"][0]["feature"], "value");
    assert_eq!(cohort["task_relevance"], "not_evaluated");
    let f = equal_remove_fixture([None, None]);
    let mut doc = build_document(&f.input()).unwrap();
    doc.relations
        .insert(f.id("/b"), DocumentRelations::Unavailable);
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    let cohort = &doc.decision_annotations.as_ref().unwrap()["cohorts"][0];
    assert!(
        cohort["distinguishing_facts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(cohort["unresolved_peers"].as_array().unwrap().len(), 1);
}

#[test]
fn relation_differences_require_known_public_endpoints() {
    use super::annotations::{self, AnnotationMode};
    let mut f = equal_remove_fixture([None, None]);
    // Distinct existing public labels, same role/name/actions on both targets.
    let mut doc = build_document(&f.input()).unwrap();
    let a = f.id("/a");
    let b = f.id("/b");
    let root = f.id("/window");
    // One known relation and one unavailable are not unequal known sets.
    doc.relations.insert(
        a,
        DocumentRelations::Known(vec![DocumentRelation {
            kind: SemanticRelationKind::LabelledBy,
            targets: vec![RelationEndpoint::Node(root)],
        }]),
    );
    doc.relations.insert(b, DocumentRelations::Unavailable);
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    assert_eq!(
        doc.decision_annotations.as_ref().unwrap()["cohorts"][0]["unresolved_peers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    doc.relations
        .insert(b, DocumentRelations::Known(Vec::new()));
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    assert_eq!(
        doc.decision_annotations.as_ref().unwrap()["cohorts"][0]["distinguishing_facts"][0]["feature"],
        "public_relations"
    );
    doc.relations.insert(
        b,
        DocumentRelations::Known(vec![DocumentRelation {
            kind: SemanticRelationKind::LabelledBy,
            targets: vec![RelationEndpoint::Unresolved(BackendLocator::new(
                ":1.100", "/missing",
            ))],
        }]),
    );
    annotations::apply(&mut doc, AnnotationMode::Contrast);
    assert_eq!(
        doc.decision_annotations.as_ref().unwrap()["cohorts"][0]["unresolved_peers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Keep the independent source validator's missing-endpoint negative control.
    f.cache
        .set_relations(
            a,
            vec![BackendRelation {
                kind: SemanticRelationKind::LabelledBy,
                targets: vec![f.cache.node(root).unwrap().backend_locator.clone()],
            }],
        )
        .unwrap();
    let mut doc = build_document(&f.input()).unwrap();
    doc.nodes.remove(&root);
    assert!(validate_document(&f.input(), &doc).is_err());
}

#[test]
fn shared_structures_retain_unpositioned_cells_and_ignore_human_cursors() {
    use crate::content::{
        SemanticTableModel, TablePositionRead, TablePositionSource, VirtualCollectionModel,
    };
    let mut f = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("Workspace"),
        vec![
            node(
                "/table",
                SemanticRole::Table,
                None,
                vec![
                    node("/a", SemanticRole::Cell, Some("A"), vec![]),
                    node(
                        "/b",
                        SemanticRole::Cell,
                        None,
                        vec![node("/btext", SemanticRole::Text, Some("Report B"), vec![])],
                    ),
                    node("/c", SemanticRole::Cell, Some("Tail"), vec![]),
                ],
            ),
            {
                let mut list = node(
                    "/list",
                    SemanticRole::List,
                    Some("Results"),
                    vec![
                        node("/item1", SemanticRole::ListItem, Some("One"), vec![]),
                        node("/item2", SemanticRole::ListItem, None, vec![]),
                    ],
                );
                list.states
                    .push(SemanticState::Other("manages-descendants".into()));
                list
            },
        ],
    ));
    let b = f.id("/b");
    f.positions.insert(
        b,
        TablePositionRead::Known {
            row: 7,
            column: 2,
            row_span: Some(1),
            column_span: Some(2),
        },
    );
    f.positions
        .insert(f.id("/c"), TablePositionRead::Unavailable);
    let doc = build_document(&f.input()).unwrap();
    let table = &doc.tables[0];
    assert_eq!(table.cells.len(), 3);
    assert_eq!(table.cells[0].row, None);
    assert_eq!(
        table.cells[0].position_source,
        TablePositionSource::NotRequested
    );
    assert_eq!(table.cells[1].row, Some(7));
    assert_eq!(table.cells[1].column_span, Some(2));
    assert_eq!(table.cells[1].label_source, Some(f.id("/btext")));
    assert_eq!(
        table.cells[2].position_source,
        TablePositionSource::Unavailable
    );
    assert_eq!(table.rows, None);
    let before = wire::to_agent_document(&doc);
    let mut human = SemanticTableModel::from_structure(table.clone());
    human.move_by(1, 0);
    assert_eq!(human.position.cell, Some(b));
    assert_eq!(human.structure, *table);
    let mut runtime = crate::content::ContentRuntime::from_shared(
        &f.cache,
        crate::content::ContentCacheBudget::default(),
        doc.content.for_human(&f.cache),
        &doc.tables,
        &doc.virtual_collections,
    );
    assert_eq!(runtime.table(table.owner).unwrap().structure, *table);
    runtime.adopt_document(
        &f.cache,
        &doc.content,
        &doc.tables,
        &doc.virtual_collections,
    );
    assert_eq!(runtime.table(table.owner).unwrap().structure, *table);
    let collection = doc
        .virtual_collections
        .iter()
        .find(|c| c.owner == f.id("/list"))
        .unwrap();
    let mut human_collection = VirtualCollectionModel::from_structure(collection.clone());
    human_collection.move_realized(1);
    assert_eq!(human_collection.structure, *collection);
    assert_eq!(wire::to_agent_document(&doc), before);
    assert!(before["tables"][0].get("position").is_none());
    assert!(before["virtual_collections"][0].get("current").is_none());
    let mut broken = doc.clone();
    broken.tables[0].cells.pop();
    assert!(validate_document(&f.input(), &broken).is_err());
    let mut broken = doc.clone();
    broken.virtual_collections.clear();
    assert!(validate_document(&f.input(), &broken).is_err());
    f.positions
        .insert(RuntimeNodeId::new(u64::MAX), TablePositionRead::Unavailable);
    assert!(build_document(&f.input()).is_err());
}

#[test]
fn shared_table_labels_never_derive_from_secret_descendants() {
    let mut secret = node(
        "/secret",
        SemanticRole::Text,
        Some("shared-table-secret"),
        vec![],
    );
    secret.text_input_kind = Some(TextInputKind::Password);
    let f = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("W"),
        vec![node(
            "/table",
            SemanticRole::Table,
            None,
            vec![node("/cell", SemanticRole::Cell, None, vec![secret])],
        )],
    ));
    let doc = build_document(&f.input()).unwrap();
    assert_eq!(doc.tables[0].cells.len(), 1);
    assert_eq!(doc.tables[0].cells[0].label_source, None);
    assert!(
        !wire::to_agent_document(&doc)
            .to_string()
            .contains("shared-table-secret")
    );
}

#[test]
fn closed_anonymous_menu_placeholders_do_not_crowd_human_controls() {
    let mut closed = node("/closed", SemanticRole::Menu, None, vec![]);
    closed
        .states
        .push(SemanticState::Other("expandable".into()));
    closed.actions.push(action("clickAncestor"));
    let mut showing = closed.clone();
    showing.backend_locator = BackendLocator::new(":1.90", "/showing");
    showing.states.push(SemanticState::Other("showing".into()));
    let mut named = closed.clone();
    named.backend_locator = BackendLocator::new(":1.90", "/named");
    named.name = Some("Useful menu".into());
    let mut apply = node("/apply", SemanticRole::Button, Some("Apply"), vec![]);
    apply.actions.push(action("click"));
    let mut f = Fixture::new(node(
        "/root",
        SemanticRole::Window,
        Some("Window"),
        vec![closed, showing, named, apply],
    ));
    f.operations = public_operations(&f.cache, &f.scopes, &f.values);
    let doc = build_document(&f.input()).unwrap();
    let before = wire::to_agent_document(&doc);
    let human = doc.human_regions(&f.cache);
    fn contains(r: &SemanticRegion, id: RuntimeNodeId) -> bool {
        r.source_nodes.contains(&id) || r.children.iter().any(|child| contains(child, id))
    }
    assert!(!contains(&human.root, f.id("/closed")));
    for path in ["/showing", "/named", "/apply"] {
        assert!(contains(&human.root, f.id(path)));
    }
    assert!(doc.operations.iter().any(|op| op.target == f.id("/closed")));
    assert_eq!(wire::to_agent_document(&doc), before);
}

#[test]
fn named_input_action_contract_is_shared_without_inventing_submission() {
    use crate::tui::palette::{CommandPalette, PaletteOutcome};
    use crossterm::event::{KeyCode, KeyEvent};
    let mut input = node("/input", SemanticRole::TextInput, Some("Query"), vec![]);
    input.text_input_kind = Some(TextInputKind::Plain);
    input.capabilities.push(SemanticCapability::EditText);
    let mut activate = action("activate");
    activate.description = Some("Activate public control".into());
    activate.keybinding = Some(";;".into());
    input.actions.push(activate.clone());
    input.actions.push(action("submit"));
    let mut button = node("/button", SemanticRole::Button, Some("Run"), vec![]);
    button.actions.push(activate.clone());
    let mut ambiguous = input.clone();
    ambiguous.backend_locator = BackendLocator::new(":1.90", "/ambiguous");
    ambiguous.actions.push(activate.clone());
    let mut password = input.clone();
    password.backend_locator = BackendLocator::new(":1.90", "/password");
    password.text_input_kind = Some(TextInputKind::Password);
    password.actions[0].description = Some("ACTION-CONTRACT-SECRET".into());
    let mut f = Fixture::new(node(
        "/root",
        SemanticRole::Window,
        Some("Window"),
        vec![input, button, ambiguous, password],
    ));
    f.operations = public_operations(&f.cache, &f.scopes, &f.values);
    let doc = build_document(&f.input()).unwrap();
    let operation = PublicOperation::Action("activate".into());
    let contract = doc.action_presentation(f.id("/input"), &operation).unwrap();
    assert_eq!(contract.display_label, "Input action: activate");
    assert_eq!(contract.effect_semantics, "provider_defined");
    assert_eq!(
        contract.provider_description.as_deref(),
        Some("Activate public control")
    );
    assert!(
        contract
            .note
            .unwrap()
            .contains("Submission/navigation is not established")
    );
    let other = doc
        .action_presentation(f.id("/button"), &operation)
        .unwrap();
    assert_eq!(other.display_label, "Action: activate");
    assert!(other.note.is_none());
    let explicit_submit = doc
        .action_presentation(f.id("/input"), &PublicOperation::Action("submit".into()))
        .unwrap();
    assert_eq!(explicit_submit.display_label, "Action: submit");
    assert!(explicit_submit.note.is_none());
    assert!(
        doc.action_presentation(f.id("/ambiguous"), &operation)
            .is_none()
    );
    assert!(
        doc.action_presentation(f.id("/password"), &operation)
            .is_none()
    );
    let wire = wire::to_agent_document(&doc);
    let descriptor = wire["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["target"] == f.id("/input").get() && op["name"] == "activate")
        .unwrap();
    assert_eq!(
        descriptor["action_semantics"],
        serde_json::to_value(&contract).unwrap()
    );
    assert_eq!(descriptor["kind"], "action");
    assert_eq!(descriptor["name"], "activate");
    assert_eq!(descriptor["parameter_domain"]["kind"], "none");
    assert_eq!(descriptor["agent_invocable"], true);
    assert!(!wire.to_string().contains("ACTION-CONTRACT-SECRET"));
    let locator = doc.nodes[&f.id("/input")].backend_locator.clone();
    let mut palette = CommandPalette::public_actions(
        doc.commands.clone(),
        doc.invocable_scope,
        vec![(
            f.id("/input"),
            locator.clone(),
            "Query › Input action: activate".into(),
            operation.clone(),
        )],
    );
    palette.set_action_presentations(|id, op| doc.action_presentation(id, op));
    assert!(
        palette.entries()[0]
            .short_label()
            .contains(&contract.display_label)
    );
    assert!(
        palette.entries()[0]
            .detail_text()
            .contains(&contract.detail_text())
    );
    assert!(matches!(palette.handle_key(KeyEvent::from(KeyCode::Enter)),
        PaletteOutcome::InvokeAction(id,loc,PublicOperation::Action(name))
            if id==f.id("/input") && loc==locator && name=="activate"));
    use crate::tui::renderer::{self, PaletteRender, RenderContext};
    use ratatui::{Terminal, backend::TestBackend};
    let scene =
        crate::transcompile::compile_scene(&doc.semantic_tree(), &doc.human_regions(&f.cache));
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            renderer::render(
                frame,
                RenderContext {
                    scene: &scene,
                    interaction_scope: None,
                    focused: None,
                    scroll_offset: 0,
                    status: "Contract check",
                    application_available: true,
                    external_text_handler_available: false,
                    edit_session: None,
                    palette: Some(PaletteRender {
                        query: "activate",
                        entries: palette.entries(),
                        selected: 0,
                        all_scopes: false,
                        scope_switchable: false,
                        detail_scroll: 0,
                    }),
                    choice: None,
                    content: None,
                    spatial: None,
                    active_region: None,
                    inline_content: None,
                },
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let human_tui = (0..40)
        .map(|y| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(human_tui.contains("Input action: activate"));
    assert!(human_tui.contains("Submission/navigation"));
    assert!(!human_tui.contains("ACTION-CONTRACT-SECRET"));
    // Captured only with --nocapture for a zero-provider thin-reader transport check.
    println!(
        "ACTION_CONTRACT_EVIDENCE {}",
        serde_json::json!({"document":wire,
        "human_result":palette.entries()[0].short_label(),
        "human_details":palette.entries()[0].detail_text(),"human_tui":human_tui,"operation":descriptor})
    );
}

#[test]
fn public_description_labels_actions_without_scene_or_showing_and_redacts_secrets() {
    let mut button = node("/described", SemanticRole::Button, None, vec![]);
    button.description = Some("Clear search text".into());
    button.actions.push(action("press"));
    let mut secret = button.clone();
    secret.backend_locator = BackendLocator::new(":1.90", "/secret");
    secret.role = SemanticRole::TextInput;
    secret.text_input_kind = Some(crate::semantic::TextInputKind::Password);
    secret.description = Some("private-description-marker".into());
    let fixture = Fixture::new(node(
        "/window",
        SemanticRole::Window,
        Some("Public window"),
        vec![button, secret],
    ));
    let doc = build_document(&fixture.input()).unwrap();
    let label = doc.operation_label(fixture.id("/described")).unwrap();
    assert_eq!(label.display_label, "Clear search text");
    assert_eq!(label.origin, OperationLabelOrigin::PublicDescription);
    let wire = wire::to_agent_document(&doc);
    assert!(!wire.to_string().contains("private-description-marker"));
    assert_eq!(
        wire["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == fixture.id("/described").get())
            .unwrap()["description"],
        "Clear search text"
    );
}
