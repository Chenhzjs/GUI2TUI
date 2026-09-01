use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use clap::ValueEnum;
use thiserror::Error;

use super::{BulkAccessibleRecord, CacheWireFormat, InspectOptions};
use crate::semantic::{
    BackendLocator, DebugInfo, RuntimeIdAllocator, SemanticCapability, SemanticNode, SemanticRole,
    SemanticState, TextInputKind, TreeTruncation,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum BootstrapStrategy {
    #[default]
    Auto,
    Cache,
    Walk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapUsed {
    Cache,
    Walk,
}

impl std::fmt::Display for BootstrapUsed {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(match self {
            Self::Cache => "AT-SPI Cache",
            Self::Walk => "recursive walk",
        })
    }
}

#[derive(Clone, Debug)]
pub struct BootstrapMetrics {
    pub strategy: BootstrapUsed,
    pub node_count: usize,
    pub cache_format: Option<CacheWireFormat>,
    pub cache_items: usize,
    pub cache_rpc: Duration,
    pub enrichment: Duration,
    pub enrichment_rpc_count: usize,
    pub reconstruction: Duration,
    pub total: Duration,
    pub orphans_ignored: usize,
    pub fallback_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BootstrapResult {
    pub root: SemanticNode,
    pub metrics: BootstrapMetrics,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReconstructionError {
    #[error("bulk cache contains duplicate object {0}")]
    DuplicateLocator(BackendLocator),
    #[error("bulk cache does not contain selected application root {0}")]
    MissingRoot(BackendLocator),
    #[error("bulk cache contains a parent cycle at {0}")]
    ParentCycle(BackendLocator),
    #[error(
        "bulk cache is incomplete: bulk_items={bulk_items} reachable_nodes={reachable_nodes} missing_child_refs={missing_child_refs} invalid_parent_links={invalid_parent_links}; first mismatch at {locator}: record advertises {expected} children but only {cached} are cached"
    )]
    IncompleteChildren {
        locator: BackendLocator,
        expected: usize,
        cached: usize,
        bulk_items: usize,
        reachable_nodes: usize,
        missing_child_refs: usize,
        invalid_parent_links: usize,
    },
    #[error(
        "bulk cache contains an unrealized document skeleton: bulk_items={bulk_items} reachable_nodes={reachable_nodes} invalid_parent_links={invalid_parent_links}; document {locator} advertises no exposed children"
    )]
    UnrealizedDocument {
        locator: BackendLocator,
        bulk_items: usize,
        reachable_nodes: usize,
        invalid_parent_links: usize,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReconstructionStats {
    pub input_records: usize,
    pub reachable_records: usize,
    pub orphans_ignored: usize,
}

pub fn reconstruct_tree(
    records: Vec<BulkAccessibleRecord>,
    application: &BackendLocator,
    options: InspectOptions,
) -> Result<(SemanticNode, ReconstructionStats), ReconstructionError> {
    let mut by_locator = HashMap::new();
    for record in records.into_iter().filter(|record| {
        record.locator.bus_name() == application.bus_name()
            && (record.locator == *application || record.application.as_ref() == Some(application))
    }) {
        let locator = record.locator.clone();
        if by_locator.insert(locator.clone(), record).is_some() {
            return Err(ReconstructionError::DuplicateLocator(locator));
        }
    }
    let input_records = by_locator.len();
    if !by_locator.contains_key(application) {
        return Err(ReconstructionError::MissingRoot(application.clone()));
    }

    let mut children: HashMap<BackendLocator, Vec<BackendLocator>> = HashMap::new();
    for record in by_locator.values() {
        if let Some(explicit) = &record.explicit_children {
            for child in explicit {
                if by_locator.contains_key(child) {
                    children
                        .entry(record.locator.clone())
                        .or_default()
                        .push(child.clone());
                }
            }
        } else if let Some(parent) = &record.parent
            && by_locator.contains_key(parent)
        {
            children
                .entry(parent.clone())
                .or_default()
                .push(record.locator.clone());
        }
    }
    for child_list in children.values_mut() {
        child_list.sort_by(|left, right| {
            let left_record = &by_locator[left];
            let right_record = &by_locator[right];
            left_record
                .index_in_parent
                .unwrap_or(usize::MAX)
                .cmp(&right_record.index_in_parent.unwrap_or(usize::MAX))
                .then_with(|| left.object_path().cmp(right.object_path()))
        });
        child_list.dedup();
    }

    let mut reachable = HashSet::new();
    let mut pending = vec![application.clone()];
    while let Some(locator) = pending.pop() {
        if reachable.insert(locator.clone())
            && let Some(descendants) = children.get(&locator)
        {
            pending.extend(descendants.iter().cloned());
        }
    }
    let invalid_parent_links = by_locator
        .values()
        .filter(|record| {
            record.locator != *application
                && record
                    .parent
                    .as_ref()
                    .is_some_and(|parent| !by_locator.contains_key(parent))
        })
        .count();

    // GetItems is a cache inventory, not a guarantee that every realized
    // descendant is resident. Reject a partial ordinary subtree so Auto can
    // fall back to the recursive source-of-truth walk. Virtualized containers
    // explicitly advertising ManagesDescendants are allowed to be partial.
    let missing = reachable
        .iter()
        .filter_map(|locator| {
            let record = &by_locator[locator];
            if locator == application || record.states.contains(atspi::State::ManagesDescendants) {
                return None;
            }
            record.child_count.and_then(|expected| {
                let cached = children.get(locator).map_or(0, Vec::len);
                (cached < expected).then(|| (locator.clone(), expected, cached))
            })
        })
        .collect::<Vec<_>>();
    if let Some((locator, expected, cached)) = missing.first() {
        return Err(ReconstructionError::IncompleteChildren {
            locator: locator.clone(),
            expected: *expected,
            cached: *cached,
            bulk_items: input_records,
            reachable_nodes: reachable.len(),
            missing_child_refs: missing
                .iter()
                .map(|(_, expected, cached)| expected.saturating_sub(*cached))
                .sum(),
            invalid_parent_links,
        });
    }

    // A reachable childless document can be a transient browser Cache
    // skeleton even when every advertised child count is internally
    // consistent. Accepting it would produce a fast but materially incomplete
    // scene. A recursive walk is the generic correctness fallback; genuinely
    // empty documents remain correct, only slower during bootstrap.
    if let Some(locator) = reachable.iter().find(|locator| {
        let record = &by_locator[*locator];
        SemanticRole::from(record.role) == SemanticRole::Document
            && children.get(*locator).is_none_or(Vec::is_empty)
    }) {
        return Err(ReconstructionError::UnrealizedDocument {
            locator: locator.clone(),
            bulk_items: input_records,
            reachable_nodes: reachable.len(),
            invalid_parent_links,
        });
    }

    let mut context = BuildContext {
        by_locator: &by_locator,
        children: &children,
        options,
        visited: HashSet::new(),
        nodes: 0,
        ids: RuntimeIdAllocator::default(),
    };
    let root = build_node(application, None, 0, &mut context)?;
    let stats = ReconstructionStats {
        input_records,
        reachable_records: context.nodes,
        orphans_ignored: input_records.saturating_sub(context.nodes),
    };
    Ok((root, stats))
}

struct BuildContext<'a> {
    by_locator: &'a HashMap<BackendLocator, BulkAccessibleRecord>,
    children: &'a HashMap<BackendLocator, Vec<BackendLocator>>,
    options: InspectOptions,
    visited: HashSet<BackendLocator>,
    nodes: usize,
    ids: RuntimeIdAllocator,
}

fn build_node(
    locator: &BackendLocator,
    index_in_parent: Option<usize>,
    depth: usize,
    context: &mut BuildContext<'_>,
) -> Result<SemanticNode, ReconstructionError> {
    if !context.visited.insert(locator.clone()) {
        return Err(ReconstructionError::ParentCycle(locator.clone()));
    }
    context.nodes += 1;
    let record = &context.by_locator[locator];
    let (role, input_kind) = semantic_role_and_input_kind(record);
    let capabilities = semantic_capabilities(record, &role, input_kind);
    let mut truncations = Vec::new();
    let mut semantic_children = Vec::new();
    let child_locators = context.children.get(locator).cloned().unwrap_or_default();
    if depth < context.options.max_depth {
        for (position, child) in child_locators.iter().enumerate() {
            if context.nodes >= context.options.max_nodes {
                truncations.push(TreeTruncation::MaxNodes {
                    limit: context.options.max_nodes,
                });
                break;
            }
            semantic_children.push(build_node(child, Some(position), depth + 1, context)?);
        }
    } else if !child_locators.is_empty() {
        truncations.push(TreeTruncation::MaxDepth {
            limit: context.options.max_depth,
        });
    }

    Ok(SemanticNode {
        runtime_id: context.ids.allocate(),
        backend_locator: locator.clone(),
        index_in_parent,
        role,
        name: record.name.clone(),
        description: record.description.clone(),
        value: if input_kind == Some(TextInputKind::Password) {
            None
        } else {
            record.value.clone()
        },
        text_input_kind: input_kind,
        states: record.states.iter().map(SemanticState::from).collect(),
        actions: record.actions.clone(),
        capabilities,
        children: semantic_children,
        truncations,
        debug: DebugInfo {
            atspi_role: record.role.name().to_owned(),
            bus_name: locator.bus_name().to_owned(),
            object_path: locator.object_path().to_owned(),
            interfaces: record
                .interfaces
                .iter()
                .map(|interface| format!("{interface:?}"))
                .collect(),
            geometry: None,
        },
    })
}

fn semantic_role_and_input_kind(
    record: &BulkAccessibleRecord,
) -> (SemanticRole, Option<TextInputKind>) {
    let role = SemanticRole::from_atspi(
        record.role,
        record.interfaces.contains(atspi::Interface::EditableText),
    );
    let input_kind =
        (role == SemanticRole::TextInput).then_some(if record.role == atspi::Role::PasswordText {
            TextInputKind::Password
        } else {
            TextInputKind::Plain
        });
    (role, input_kind)
}

fn semantic_capabilities(
    record: &BulkAccessibleRecord,
    role: &SemanticRole,
    input_kind: Option<TextInputKind>,
) -> Vec<SemanticCapability> {
    let mut capabilities = Vec::new();
    if record.interfaces.contains(atspi::Interface::Selection) {
        capabilities.push(SemanticCapability::SelectChildren);
    }
    if *role == SemanticRole::TextInput
        && input_kind == Some(TextInputKind::Plain)
        && record.interfaces.contains(atspi::Interface::EditableText)
        && record.interfaces.contains(atspi::Interface::Text)
        && record.states.contains(atspi::State::Editable)
        && !record.states.contains(atspi::State::MultiLine)
    {
        capabilities.push(SemanticCapability::EditText);
    }
    capabilities
}

#[cfg(test)]
mod tests {
    use atspi::{Interface, InterfaceSet, Role, StateSet};

    use super::*;

    fn record(path: &str, parent: Option<&str>, index: Option<usize>) -> BulkAccessibleRecord {
        BulkAccessibleRecord {
            locator: BackendLocator::new(":1.2", path),
            application: Some(BackendLocator::new(":1.2", "/root")),
            parent: parent.map(|path| BackendLocator::new(":1.2", path)),
            index_in_parent: index,
            child_count: None,
            explicit_children: None,
            interfaces: InterfaceSet::new(Interface::Accessible),
            name: Some(path.to_owned()),
            role: if path == "/root" {
                Role::Application
            } else {
                Role::Button
            },
            description: None,
            states: StateSet::empty(),
            actions: Vec::new(),
            value: None,
        }
    }

    #[test]
    fn reconstructs_parent_relationship_and_child_index_order() {
        let root = BackendLocator::new(":1.2", "/root");
        let records = vec![
            record("/b", Some("/root"), Some(1)),
            record("/root", None, None),
            record("/a", Some("/root"), Some(0)),
        ];
        let (tree, stats) = reconstruct_tree(
            records,
            &root,
            InspectOptions {
                verbose: false,
                max_depth: 10,
                max_nodes: 10,
            },
        )
        .unwrap();
        assert_eq!(tree.children[0].backend_locator.object_path(), "/a");
        assert_eq!(tree.children[1].backend_locator.object_path(), "/b");
        assert_eq!(stats.orphans_ignored, 0);
    }

    #[test]
    fn bulk_multiline_documents_cannot_receive_atomic_edit_capability() {
        let mut text = record("/text", Some("/root"), Some(0));
        text.role = Role::Entry;
        text.interfaces.insert(Interface::Text);
        text.interfaces.insert(Interface::EditableText);
        text.states.insert(atspi::State::Editable);
        assert_eq!(
            semantic_capabilities(&text, &SemanticRole::TextInput, Some(TextInputKind::Plain)),
            vec![SemanticCapability::EditText]
        );
        text.states.insert(atspi::State::MultiLine);
        assert!(
            semantic_capabilities(&text, &SemanticRole::TextInput, Some(TextInputKind::Plain))
                .is_empty()
        );
    }

    #[test]
    fn ignores_orphans_but_rejects_duplicates_and_missing_root() {
        let root = BackendLocator::new(":1.2", "/root");
        let records = vec![
            record("/root", None, None),
            record("/orphan", Some("/missing"), Some(0)),
        ];
        let (_, stats) = reconstruct_tree(
            records,
            &root,
            InspectOptions {
                verbose: false,
                max_depth: 10,
                max_nodes: 10,
            },
        )
        .unwrap();
        assert_eq!(stats.orphans_ignored, 1);

        let duplicate = vec![record("/root", None, None), record("/root", None, None)];
        assert!(matches!(
            reconstruct_tree(
                duplicate,
                &root,
                InspectOptions {
                    verbose: false,
                    max_depth: 10,
                    max_nodes: 10
                }
            ),
            Err(ReconstructionError::DuplicateLocator(_))
        ));
        assert!(matches!(
            reconstruct_tree(
                Vec::new(),
                &root,
                InspectOptions {
                    verbose: false,
                    max_depth: 10,
                    max_nodes: 10
                }
            ),
            Err(ReconstructionError::MissingRoot(_))
        ));
    }

    #[test]
    fn transient_record_remains_when_reachable() {
        let root = BackendLocator::new(":1.2", "/root");
        let mut transient = record("/popup", Some("/root"), Some(0));
        transient.states = [atspi::State::Transient].into_iter().collect();
        let (tree, _) = reconstruct_tree(
            vec![record("/root", None, None), transient],
            &root,
            InspectOptions {
                verbose: false,
                max_depth: 10,
                max_nodes: 10,
            },
        )
        .unwrap();
        assert!(
            matches!(tree.children[0].states.as_slice(), [SemanticState::Other(state)] if state == "transient")
        );
    }

    #[test]
    fn incomplete_cache_falls_back_but_managed_descendants_may_be_partial() {
        let root = BackendLocator::new(":1.2", "/root");
        let mut container = record("/container", Some("/root"), Some(0));
        container.child_count = Some(2);
        let records = vec![record("/root", None, None), container.clone()];
        assert!(matches!(
            reconstruct_tree(
                records,
                &root,
                InspectOptions {
                    verbose: false,
                    max_depth: 10,
                    max_nodes: 10
                }
            ),
            Err(ReconstructionError::IncompleteChildren {
                expected: 2,
                cached: 0,
                bulk_items: 2,
                reachable_nodes: 2,
                missing_child_refs: 2,
                invalid_parent_links: 0,
                ..
            })
        ));

        container.states = [atspi::State::ManagesDescendants].into_iter().collect();
        assert!(
            reconstruct_tree(
                vec![record("/root", None, None), container],
                &root,
                InspectOptions {
                    verbose: false,
                    max_depth: 10,
                    max_nodes: 10
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn incomplete_orphan_does_not_reject_a_complete_reachable_tree() {
        let root = BackendLocator::new(":1.2", "/root");
        let mut orphan = record("/orphan", Some("/missing"), Some(0));
        orphan.child_count = Some(5);
        let (_, stats) = reconstruct_tree(
            vec![record("/root", None, None), orphan],
            &root,
            InspectOptions {
                verbose: false,
                max_depth: 10,
                max_nodes: 10,
            },
        )
        .unwrap();
        assert_eq!(stats.reachable_records, 1);
        assert_eq!(stats.orphans_ignored, 1);
    }

    #[test]
    fn reachable_empty_document_skeleton_requires_walk_fallback() {
        let root = BackendLocator::new(":1.2", "/root");
        let mut document = record("/document", Some("/root"), Some(0));
        document.role = atspi::Role::DocumentWeb;
        assert!(matches!(
            reconstruct_tree(
                vec![record("/root", None, None), document],
                &root,
                InspectOptions {
                    verbose: false,
                    max_depth: 10,
                    max_nodes: 10,
                }
            ),
            Err(ReconstructionError::UnrealizedDocument {
                bulk_items: 2,
                reachable_nodes: 2,
                invalid_parent_links: 0,
                ..
            })
        ));
    }
}
