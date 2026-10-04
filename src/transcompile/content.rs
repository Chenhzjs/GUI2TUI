use crate::{
    content::ContentCatalog,
    semantic::SemanticCache,
    tui::action::{InteractionCapability, UiIntent},
};

use super::{
    PresentationStrategy, SceneBinding, SceneElement, SceneElementId, SceneElementKind, TuiScene,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentCompressionMetrics {
    pub before_elements: usize,
    pub after_elements: usize,
    pub summaries: usize,
    pub preserved_bound_elements: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentReachabilityAudit {
    pub headings: usize,
    pub links: usize,
    pub form_controls: usize,
    pub opaque_items: usize,
    pub reachable: usize,
    pub unreachable: Vec<String>,
}

pub fn audit_content_reachability(
    scene: &TuiScene,
    content: &ContentCatalog,
) -> ContentReachabilityAudit {
    let reader_roots: std::collections::HashSet<_> = scene
        .elements
        .iter()
        .filter_map(|element| match element.kind {
            SceneElementKind::DocumentSummary { .. } => {
                element.binding.as_ref().map(|binding| binding.runtime_id)
            }
            _ => None,
        })
        .collect();
    let bound_controls: std::collections::HashSet<_> = scene
        .elements
        .iter()
        .filter_map(|element| element.binding.as_ref().map(|binding| binding.runtime_id))
        .collect();
    let mut audit = ContentReachabilityAudit::default();
    for model in content.visible_models() {
        let reader_reachable = reader_roots.contains(&model.root);
        for (kind, ids) in [
            ("heading", &model.navigation.headings),
            ("link", &model.navigation.links),
            ("opaque", &model.navigation.opaque),
        ] {
            match kind {
                "heading" => audit.headings += ids.len(),
                "link" => audit.links += ids.len(),
                _ => audit.opaque_items += ids.len(),
            }
            if reader_reachable {
                audit.reachable += ids.len();
            } else {
                audit.unreachable.extend(
                    ids.iter()
                        .map(|id| format!("{kind} block={id} root={}", model.root)),
                );
            }
        }
        audit.form_controls += model.navigation.form_fields.len();
        for source in &model.navigation.form_fields {
            if bound_controls.contains(source) {
                audit.reachable += 1;
            } else {
                audit
                    .unreachable
                    .push(format!("form-control runtime={source} root={}", model.root));
            }
        }
    }
    audit
}

pub fn format_content_reachability(audit: &ContentReachabilityAudit) -> String {
    let total = audit.headings + audit.links + audit.form_controls + audit.opaque_items;
    let mut output = format!(
        "content semantic targets: headings={} links={} form-controls={} opaque={} total={}\nreachable: {}\nunreachable: {}\n",
        audit.headings,
        audit.links,
        audit.form_controls,
        audit.opaque_items,
        total,
        audit.reachable,
        audit.unreachable.len(),
    );
    for target in &audit.unreachable {
        output.push_str(&format!("  {target}\n"));
    }
    output
}

pub fn compress_content_scene(
    scene: &mut TuiScene,
    cache: &SemanticCache,
    content: &ContentCatalog,
) -> ContentCompressionMetrics {
    let before_elements = scene.elements.len();
    if content.visible_models().next().is_none() {
        return ContentCompressionMetrics {
            before_elements,
            after_elements: before_elements,
            ..Default::default()
        };
    }
    let mut next_id = scene
        .elements
        .iter()
        .map(|element| element.id.get())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut summaries = Vec::new();
    for model in content.visible_models() {
        let Some(root) = cache.node(model.root) else {
            continue;
        };
        let summary = model.summary();
        summaries.push(SceneElement {
            id: SceneElementId::new(next_id),
            kind: SceneElementKind::DocumentSummary {
                title: model
                    .metadata
                    .title
                    .clone()
                    .or_else(|| root.name.clone())
                    .unwrap_or_else(|| "Document".to_owned()),
                blocks: summary.blocks,
                headings: summary.headings,
                links: summary.links,
                forms: summary.forms,
                completeness: format!("{:?}", model.completeness),
                external_edit: root
                    .capabilities
                    .contains(&crate::semantic::SemanticCapability::EditComplexText),
            },
            sources: vec![model.root],
            binding: Some(SceneBinding {
                runtime_id: model.root,
                backend_locator: root.backend_locator.clone(),
                semantic_role: root.role.clone(),
                actions: root.actions.clone(),
                capability: InteractionCapability::BrowseContent,
                default_intent: UiIntent::BeginRead,
            }),
            strategy: PresentationStrategy::StructuredSummary,
        });
        next_id = next_id.saturating_add(1);
    }
    let mut elements = Vec::with_capacity(scene.elements.len() + summaries.len());
    // Reader is an explicit projection now. Keep the normalized Surface
    // elements, including structural groups and unbound content rows, and add
    // one bounded Reader entry point per model. Removing every content-only
    // element here used to turn a Document into a linear Reader by default.
    elements.extend(scene.elements.iter().cloned());
    // Append the Reader entry point so initial focus remains on the ordinary
    // Surface controls rather than silently entering the reading projection.
    elements.extend(summaries);
    let summaries_count = summaries_len(content);
    let preserved_bound_elements = elements
        .iter()
        .filter(|element| {
            element.binding.is_some()
                && !matches!(element.kind, SceneElementKind::DocumentSummary { .. })
        })
        .count();
    scene.replace_elements(elements);
    ContentCompressionMetrics {
        before_elements,
        after_elements: scene.elements.len(),
        summaries: summaries_count,
        preserved_bound_elements,
    }
}

fn summaries_len(content: &ContentCatalog) -> usize {
    content.visible_models().count()
}

#[cfg(test)]
mod tests {
    use crate::{
        content::ContentCatalog,
        semantic::{
            BackendLocator, DebugInfo, SemanticAction, SemanticCache, SemanticCapability,
            SemanticNode, SemanticRole, SemanticState, TextInputKind,
        },
        transcompile::{SceneElementKind, analyze_regions, compile_scene},
    };

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: crate::semantic::RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.7", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: Some(name.to_owned()),
            description: None,
            value: None,
            text_input_kind: None,
            states: Vec::new(),
            actions: Vec::new(),
            capabilities: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo::default(),
        }
    }

    #[test]
    fn document_body_is_compressed_but_bound_controls_remain_reachable() {
        let mut document = node(1, SemanticRole::Document, "Article");
        document
            .children
            .push(node(2, SemanticRole::Paragraph, "Body paragraph"));
        let mut button = node(3, SemanticRole::Button, "Subscribe");
        button.actions.push(SemanticAction {
            index: 0,
            name: "click".to_owned(),
            description: None,
            keybinding: None,
        });
        document.children.push(button);
        let cache = SemanticCache::from_snapshot(document).unwrap();
        let tree = cache.materialize_tree().unwrap();
        let analysis = analyze_regions(&tree);
        let mut scene = compile_scene(&tree, &analysis);
        let catalog = ContentCatalog::analyze(&cache);
        let metrics = compress_content_scene(&mut scene, &cache, &catalog);
        assert!(metrics.after_elements <= metrics.before_elements + 1);
        assert!(
            scene.elements.iter().any(|element| {
                matches!(element.kind, SceneElementKind::DocumentSummary { .. })
            })
        );
        assert!(scene.elements.iter().any(|element| {
            matches!(
                element.kind,
                SceneElementKind::Text { ref text } if text == "Body paragraph"
            )
        }));
        assert!(scene.elements.iter().any(|element| {
            matches!(
                element.kind,
                SceneElementKind::Button { ref label } if label == "Subscribe"
            ) && element.binding.as_ref().is_some_and(|binding| {
                binding.semantic_role == SemanticRole::Button
                    && binding.backend_locator.object_path() == "/node/3"
            })
        }));
        let audit = audit_content_reachability(&scene, &catalog);
        assert_eq!(audit.form_controls, 1);
        assert_eq!(audit.reachable, 1);
        assert!(audit.unreachable.is_empty());
    }

    #[test]
    fn surface_and_reader_share_the_document_source_without_replacing_regions() {
        let mut window = node(1, SemanticRole::Window, "Application");
        let mut toolbar = node(2, SemanticRole::Toolbar, "Toolbar");
        let mut toolbar_input = node(
            3,
            SemanticRole::Unknown("search-box".to_owned()),
            "Global search",
        );
        toolbar_input.text_input_kind = Some(TextInputKind::Plain);
        toolbar_input
            .capabilities
            .push(SemanticCapability::EditText);
        toolbar.children.push(toolbar_input);

        let mut document = node(4, SemanticRole::Document, "Article");
        document.states.push(SemanticState::Other("showing".into()));
        document
            .children
            .push(node(5, SemanticRole::Navigation, "Navigation"));
        let mut form = node(6, SemanticRole::Form, "Search Form");
        let mut wrapper = node(7, SemanticRole::Container, "");
        let mut query = node(8, SemanticRole::TextInput, "Query");
        query.text_input_kind = Some(TextInputKind::Plain);
        query.capabilities.push(SemanticCapability::EditText);
        let mut submit = node(9, SemanticRole::Button, "Submit");
        submit.actions.push(SemanticAction {
            index: 0,
            name: "click".into(),
            description: None,
            keybinding: None,
        });
        wrapper.children = vec![query, submit];
        form.children.push(wrapper);
        let mut main = node(10, SemanticRole::Main, "Main");
        let mut list = node(11, SemanticRole::List, "Results");
        list.capabilities
            .push(SemanticCapability::SelectCurrentChild);
        let mut item = node(12, SemanticRole::ListItem, "Result");
        item.states = vec![
            SemanticState::Enabled,
            SemanticState::Other("showing".into()),
        ];
        item.actions.push(SemanticAction {
            index: 0,
            name: "toggle".into(),
            description: None,
            keybinding: None,
        });
        list.children.push(item);
        main.children.push(list);
        document.children.extend([form, main]);
        window.children.extend([toolbar, document]);

        let cache = SemanticCache::from_snapshot(window).unwrap();
        let tree = cache.materialize_tree().unwrap();
        let analysis = analyze_regions(&tree);
        let mut scene = compile_scene(&tree, &analysis);
        let catalog = ContentCatalog::analyze(&cache);
        compress_content_scene(&mut scene, &cache, &catalog);

        for label in ["Toolbar", "Article", "Navigation", "Search Form", "Main"] {
            assert!(scene.elements.iter().any(|element| {
                matches!(&element.kind, SceneElementKind::Group { label: value } if value == label)
            }), "missing surface region {label}");
        }
        assert!(scene.elements.iter().any(|element| {
            matches!(&element.kind, SceneElementKind::Field { label, .. } if label == "Query")
                && element.binding.is_some()
        }));
        assert!(scene.elements.iter().any(|element| {
            matches!(&element.kind, SceneElementKind::Button { label } if label == "Submit")
                && element.binding.is_some()
        }));
        let summary = scene
            .elements
            .iter()
            .find(|element| matches!(element.kind, SceneElementKind::DocumentSummary { .. }))
            .expect("Reader projection entry point");
        assert_eq!(
            summary.binding.as_ref().unwrap().runtime_id,
            summary.sources[0]
        );
        assert!(
            summary
                .sources
                .contains(&summary.binding.as_ref().unwrap().runtime_id)
        );
    }
}
