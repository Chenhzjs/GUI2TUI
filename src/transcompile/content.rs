use crate::{
    content::ContentCatalog,
    semantic::{RuntimeNodeId, SemanticCache},
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
    for model in content.models() {
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
    if content.models().next().is_none() {
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
    for model in content.models() {
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
    elements.extend(summaries);
    let content_roots: std::collections::HashSet<RuntimeNodeId> =
        content.models().map(|model| model.root).collect();
    elements.extend(
        scene
            .elements
            .iter()
            .filter(|element| {
                // Content-only presentation is replaced by a bounded Reader.
                // Existing semantic bindings (forms, choices, links, commands)
                // remain reachable.
                let content_root_presentation = element
                    .binding
                    .as_ref()
                    .is_some_and(|binding| content_roots.contains(&binding.runtime_id));
                !content_root_presentation
                    && (element.binding.is_some()
                        || element.sources.is_empty()
                        || !element
                            .sources
                            .iter()
                            .all(|source| content.is_content_source(*source)))
            })
            .cloned(),
    );
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
    content.models().count()
}

#[cfg(test)]
mod tests {
    use crate::{
        content::ContentCatalog,
        semantic::{
            BackendLocator, DebugInfo, SemanticAction, SemanticCache, SemanticNode, SemanticRole,
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
        assert!(metrics.after_elements < metrics.before_elements + 1);
        assert!(
            scene.elements.iter().any(|element| {
                matches!(element.kind, SceneElementKind::DocumentSummary { .. })
            })
        );
        assert!(!scene.elements.iter().any(|element| {
            element
                .sources
                .contains(&crate::semantic::RuntimeNodeId::new(2))
                && element.binding.is_none()
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
}
