use std::collections::{HashMap, HashSet};

use crate::{
    semantic::{
        RelationalSemanticGraph, RuntimeNodeId, SemanticCapability, SemanticNode, SemanticRole,
    },
    tui::action::{InteractionCapability, UiIntent, interaction_capability},
};

use super::region::{
    ModalityPolicy, RegionConfidence, RegionId, RegionInteraction, SemanticRegion,
    SemanticRegionKind,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionMetrics {
    pub semantic_nodes: usize,
    pub interactive_nodes: usize,
    pub regions: usize,
    pub direct_controls: usize,
    pub reconstructed: usize,
    pub compressed: usize,
    pub command_regions: usize,
    pub selection_regions: usize,
    pub opaque_regions: usize,
    pub unsupported_regions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionAnalysis {
    pub root: SemanticRegion,
    pub metrics: RegionMetrics,
}

#[derive(Default)]
struct RelationHints {
    labels: HashMap<RuntimeNodeId, (RuntimeNodeId, String)>,
    descriptions: HashMap<RuntimeNodeId, Vec<String>>,
    errors: HashMap<RuntimeNodeId, Vec<String>>,
    memberships: HashMap<RuntimeNodeId, Vec<RuntimeNodeId>>,
    consumed_labels: HashSet<RuntimeNodeId>,
}

impl RelationHints {
    fn from_graph(root: &SemanticNode, graph: &RelationalSemanticGraph<'_>) -> Self {
        fn collect(node: &SemanticNode, output: &mut Vec<RuntimeNodeId>) {
            output.push(node.runtime_id);
            for child in &node.children {
                collect(child, output);
            }
        }
        let mut ids = Vec::new();
        collect(root, &mut ids);
        let mut hints = Self::default();
        for id in ids {
            let labels: Vec<_> = graph
                .labels_for(id)
                .into_iter()
                .filter_map(|label_id| {
                    graph
                        .node(label_id)
                        .and_then(|node| node.name.clone().or_else(|| node.value.clone()))
                        .map(|label| (label_id, label))
                })
                .collect();
            if let [(label_id, label)] = labels.as_slice() {
                hints.labels.insert(id, (*label_id, label.clone()));
                hints.consumed_labels.insert(*label_id);
            }
            let texts = |targets: Vec<RuntimeNodeId>| {
                targets
                    .into_iter()
                    .filter_map(|target| {
                        graph
                            .node(target)
                            .and_then(|node| node.name.clone().or_else(|| node.value.clone()))
                    })
                    .collect::<Vec<_>>()
            };
            let descriptions = texts(graph.descriptions_for(id));
            if !descriptions.is_empty() {
                hints.descriptions.insert(id, descriptions);
            }
            let errors = texts(graph.errors_for(id));
            if !errors.is_empty() {
                hints.errors.insert(id, errors);
            }
            let memberships = graph.memberships_of(id);
            if !memberships.is_empty() {
                hints.memberships.insert(id, memberships);
            }
        }
        hints
    }
}

#[derive(Default)]
struct Analyzer {
    next_region: u64,
    metrics: RegionMetrics,
    relations: RelationHints,
}

pub fn analyze_regions(root: &SemanticNode) -> RegionAnalysis {
    let mut analyzer = Analyzer::default();
    analyzer.run(root)
}

pub fn analyze_regions_with_graph(
    root: &SemanticNode,
    graph: &RelationalSemanticGraph<'_>,
) -> RegionAnalysis {
    let mut analyzer = Analyzer {
        relations: RelationHints::from_graph(root, graph),
        ..Default::default()
    };
    analyzer.run(root)
}

impl Analyzer {
    fn run(&mut self, root: &SemanticNode) -> RegionAnalysis {
        self.metrics.semantic_nodes = count_nodes(root);
        self.metrics.interactive_nodes = count_interactive(root, &[]);
        let root = self.analyze_node(root, &[], &[]);
        self.metrics.regions = count_regions(&root);
        RegionAnalysis {
            root,
            metrics: self.metrics.clone(),
        }
    }
}

impl Analyzer {
    fn id(&mut self) -> RegionId {
        let id = RegionId::new(self.next_region);
        self.next_region += 1;
        id
    }

    fn analyze_node(
        &mut self,
        node: &SemanticNode,
        parent_capabilities: &[crate::semantic::SemanticCapability],
        command_path: &[String],
    ) -> SemanticRegion {
        if is_command_container(node) {
            return self.command_set(node, parent_capabilities, command_path);
        }
        if node.role == SemanticRole::List {
            return self.selection_region(node);
        }
        if node.role == SemanticRole::StatusBar {
            let mut region = SemanticRegion::terminal_native(
                self.id(),
                SemanticRegionKind::Status,
                vec![node.runtime_id],
            );
            region.label = semantic_label(node);
            region.children = self.analyze_children(node, command_path);
            return region;
        }
        if is_opaque_candidate(node) {
            self.metrics.opaque_regions += 1;
            let mut region = SemanticRegion::terminal_native(
                self.id(),
                SemanticRegionKind::OpaqueContent,
                vec![node.runtime_id],
            );
            region.label = semantic_label(node).or_else(|| Some("Graphical content".to_owned()));
            region.modality = ModalityPolicy::FidelityPreferred;
            return region;
        }
        if node.role == SemanticRole::TextInput
            && let Some((label_id, label)) = self.relations.labels.get(&node.runtime_id).cloned()
        {
            return self.relation_field(node, label_id, label, parent_capabilities, command_path);
        }
        if matches!(node.role, SemanticRole::Label | SemanticRole::Text)
            && let Some(control) = unique_descendant_text_input(node)
        {
            let mut field = SemanticRegion::terminal_native(
                self.id(),
                SemanticRegionKind::Field,
                vec![node.runtime_id, control.runtime_id],
            );
            field.label = node.name.clone().or_else(|| control.name.clone());
            field.confidence = RegionConfidence::Strong;
            field
                .children
                .push(self.control_region(control, parent_capabilities, command_path));
            self.metrics.reconstructed += 1;
            return field;
        }
        if matches!(node.role, SemanticRole::Label | SemanticRole::Text)
            && node.children.iter().any(has_interactive_descendant)
        {
            let mut group = SemanticRegion::terminal_native(
                self.id(),
                SemanticRegionKind::Group,
                vec![node.runtime_id],
            );
            group.label = semantic_label(node);
            group.children = self.analyze_children(node, command_path);
            self.metrics.reconstructed += 1;
            return group;
        }
        if node.role == SemanticRole::ComboBox {
            self.metrics.direct_controls += 1;
            let control = self.control_region(node, parent_capabilities, command_path);
            let expanded = node
                .states
                .contains(&crate::semantic::SemanticState::Expanded);
            let structural_popup = !expanded && node.children.len() > 1;
            let popup_children: Vec<_> = if expanded {
                node.children
                    .iter()
                    .filter(|child| matches!(child.role, SemanticRole::List | SemanticRole::Menu))
                    .collect()
            } else if structural_popup {
                node.children.iter().skip(1).collect()
            } else {
                Vec::new()
            };
            if !popup_children.is_empty() {
                let mut group = SemanticRegion::terminal_native(
                    self.id(),
                    SemanticRegionKind::Group,
                    vec![node.runtime_id],
                );
                group.label = node.name.clone();
                group.children.push(control);
                group.children.extend(
                    popup_children
                        .into_iter()
                        .map(|child| self.analyze_node(child, &node.capabilities, command_path)),
                );
                return group;
            }
            return control;
        }
        if is_direct_control(node) {
            self.metrics.direct_controls += 1;
            return self.control_region(node, parent_capabilities, command_path);
        }
        if matches!(node.role, SemanticRole::Label | SemanticRole::Text) {
            let mut region = SemanticRegion::terminal_native(
                self.id(),
                SemanticRegionKind::Content,
                vec![node.runtime_id],
            );
            region.label = semantic_label(node);
            return region;
        }

        let children = self.analyze_children(node, command_path);
        let field_count = children
            .iter()
            .filter(|child| child.kind == SemanticRegionKind::Field)
            .count();
        let form_control_count = children
            .iter()
            .filter(|child| {
                child.kind == SemanticRegionKind::Field
                    || (child.kind == SemanticRegionKind::Control && !child.interactions.is_empty())
            })
            .count();
        let kind = if field_count >= 2 && form_control_count >= 2 {
            self.metrics.reconstructed += 1;
            SemanticRegionKind::Form
        } else if matches!(node.role, SemanticRole::Application | SemanticRole::Window) {
            SemanticRegionKind::Navigation
        } else if matches!(node.role, SemanticRole::Container | SemanticRole::Dialog) {
            SemanticRegionKind::Group
        } else {
            self.metrics.unsupported_regions += 1;
            SemanticRegionKind::Unknown
        };
        let mut region = SemanticRegion::terminal_native(self.id(), kind, vec![node.runtime_id]);
        region.label = node.name.clone();
        region.children = children;
        region
    }

    fn analyze_children(
        &mut self,
        node: &SemanticNode,
        command_path: &[String],
    ) -> Vec<SemanticRegion> {
        let mut regions = Vec::new();
        let mut index = 0;
        while index < node.children.len() {
            let child = &node.children[index];
            if self.relations.consumed_labels.contains(&child.runtime_id)
                && child.children.is_empty()
            {
                index += 1;
                continue;
            }
            if child.role == SemanticRole::TextInput
                && let Some((label_id, label)) =
                    self.relations.labels.get(&child.runtime_id).cloned()
            {
                regions.push(self.relation_field(
                    child,
                    label_id,
                    label,
                    &node.capabilities,
                    command_path,
                ));
                index += 1;
                continue;
            }
            if matches!(child.role, SemanticRole::Label | SemanticRole::Text) {
                let mut end = index + 1;
                while node
                    .children
                    .get(end)
                    .is_some_and(|candidate| candidate.role == SemanticRole::RadioButton)
                {
                    end += 1;
                }
                if end.saturating_sub(index + 1) >= 2 {
                    let radios = &node.children[index + 1..end];
                    let mut group = SemanticRegion::terminal_native(
                        self.id(),
                        SemanticRegionKind::Selection,
                        std::iter::once(child.runtime_id)
                            .chain(radios.iter().map(|radio| radio.runtime_id))
                            .collect(),
                    );
                    group.label = semantic_label(child).or_else(|| Some("Options".to_owned()));
                    group.confidence = RegionConfidence::Strong;
                    group.children = radios
                        .iter()
                        .map(|radio| self.control_region(radio, &node.capabilities, command_path))
                        .collect();
                    self.metrics.selection_regions += 1;
                    self.metrics.reconstructed += 1;
                    regions.push(group);
                    index = end;
                    continue;
                }
            }
            if matches!(child.role, SemanticRole::Label | SemanticRole::Text)
                && let Some(control) = node.children.get(index + 1)
                && control.role == SemanticRole::TextInput
                && !self.relations.labels.contains_key(&control.runtime_id)
                && conservative_label_match(child, control, &node.children)
            {
                let mut field = SemanticRegion::terminal_native(
                    self.id(),
                    SemanticRegionKind::Field,
                    vec![child.runtime_id, control.runtime_id],
                );
                field.label = child.name.clone().or_else(|| control.name.clone());
                field.confidence = RegionConfidence::Strong;
                field
                    .children
                    .push(self.control_region(control, &node.capabilities, command_path));
                self.metrics.reconstructed += 1;
                regions.push(field);
                index += 2;
                continue;
            }
            regions.push(self.analyze_node(child, &node.capabilities, command_path));
            index += 1;
        }

        // Layout-only wrappers do not deserve terminal rows. Conservatively
        // flatten only unnamed groups with no interaction and one child.
        let mut compressed = Vec::new();
        for region in regions {
            if region.kind == SemanticRegionKind::Group
                && region.label.is_none()
                && region.interactions.is_empty()
                && region.children.len() == 1
            {
                self.metrics.compressed += 1;
                compressed.extend(region.children);
            } else {
                compressed.push(region);
            }
        }
        let mut summaries: Vec<SemanticRegion> = Vec::new();
        for region in compressed {
            if region.kind == SemanticRegionKind::Content
                && let Some(previous) = summaries.last_mut()
                && previous.kind == SemanticRegionKind::Content
            {
                previous.source_nodes.extend(region.source_nodes);
                if let Some(label) = region.label
                    && previous.label.as_deref() != Some(label.as_str())
                {
                    previous
                        .label
                        .get_or_insert_default()
                        .push_str(&format!(" · {label}"));
                }
                self.metrics.compressed += 1;
            } else {
                summaries.push(region);
            }
        }
        summaries
    }

    fn relation_field(
        &mut self,
        control: &SemanticNode,
        label_id: RuntimeNodeId,
        label: String,
        parent_capabilities: &[crate::semantic::SemanticCapability],
        command_path: &[String],
    ) -> SemanticRegion {
        let mut field = SemanticRegion::terminal_native(
            self.id(),
            SemanticRegionKind::Field,
            vec![label_id, control.runtime_id],
        );
        field.label = Some(label);
        field.confidence = RegionConfidence::Exact;
        field.descriptions = self
            .relations
            .descriptions
            .get(&control.runtime_id)
            .cloned()
            .unwrap_or_default();
        field.errors = self
            .relations
            .errors
            .get(&control.runtime_id)
            .cloned()
            .unwrap_or_default();
        field.logical_group = self
            .relations
            .memberships
            .get(&control.runtime_id)
            .cloned()
            .unwrap_or_default();
        field
            .children
            .push(self.control_region(control, parent_capabilities, command_path));
        self.metrics.reconstructed += 1;
        field
    }

    fn control_region(
        &mut self,
        node: &SemanticNode,
        parent_capabilities: &[crate::semantic::SemanticCapability],
        command_path: &[String],
    ) -> SemanticRegion {
        let capability = interaction_capability(
            &node.role,
            &node.actions,
            &node.capabilities,
            parent_capabilities,
        );
        let mut region = SemanticRegion::terminal_native(
            self.id(),
            SemanticRegionKind::Control,
            vec![node.runtime_id],
        );
        region.label = semantic_label(node);
        region.command_path = command_path.to_vec();
        if let Some(intent) = intent_for_capability(capability) {
            region.interactions.push(RegionInteraction {
                source: node.runtime_id,
                intent,
            });
        }
        region
    }

    fn selection_region(&mut self, node: &SemanticNode) -> SemanticRegion {
        self.metrics.selection_regions += 1;
        let mut region = SemanticRegion::terminal_native(
            self.id(),
            SemanticRegionKind::Selection,
            vec![node.runtime_id],
        );
        region.label = semantic_label(node).or_else(|| Some("Selection".to_owned()));
        region.children = node
            .children
            .iter()
            .map(|child| self.control_region(child, &node.capabilities, &[]))
            .collect();
        region
    }

    fn command_set(
        &mut self,
        node: &SemanticNode,
        parent_capabilities: &[crate::semantic::SemanticCapability],
        inherited_path: &[String],
    ) -> SemanticRegion {
        self.metrics.command_regions += 1;
        let mut region = SemanticRegion::terminal_native(
            self.id(),
            SemanticRegionKind::CommandSet,
            vec![node.runtime_id],
        );
        region.label = node.name.clone().or_else(|| Some("Commands".to_owned()));
        let mut path = inherited_path.to_vec();
        if let Some(name) = &node.name {
            path.push(name.clone());
        }
        collect_commands(self, node, parent_capabilities, &path, &mut region.children);
        region
    }
}

fn collect_commands(
    analyzer: &mut Analyzer,
    node: &SemanticNode,
    parent_capabilities: &[crate::semantic::SemanticCapability],
    path: &[String],
    output: &mut Vec<SemanticRegion>,
) {
    for child in &node.children {
        let mut child_path = path.to_vec();
        if matches!(child.role, SemanticRole::Menu | SemanticRole::MenuItem)
            && child
                .children
                .iter()
                .any(|nested| matches!(nested.role, SemanticRole::Menu | SemanticRole::MenuItem))
        {
            if let Some(name) = &child.name {
                child_path.push(name.clone());
            }
            collect_commands(analyzer, child, &node.capabilities, &child_path, output);
            continue;
        }
        if is_direct_control(child) && !child.actions.is_empty() {
            let mut command = analyzer.control_region(child, parent_capabilities, &child_path);
            command.command_path = child_path;
            if !command.interactions.is_empty() {
                output.push(command);
            }
        } else {
            collect_commands(analyzer, child, &node.capabilities, &child_path, output);
        }
    }
}

fn conservative_label_match(
    label: &SemanticNode,
    control: &SemanticNode,
    siblings: &[SemanticNode],
) -> bool {
    let label_name = label.name.as_deref().map(normalize);
    let control_name = control.name.as_deref().map(normalize);
    match (label_name, control_name) {
        (Some(label), Some(control)) if label == control => {
            siblings
                .iter()
                .filter(|candidate| {
                    matches!(candidate.role, SemanticRole::Label | SemanticRole::Text)
                        && candidate.name.as_deref().map(normalize).as_deref()
                            == Some(label.as_str())
                })
                .count()
                == 1
        }
        (Some(_), None) => true,
        _ => false,
    }
}

fn normalize(value: &str) -> String {
    value.trim().trim_end_matches(':').to_lowercase()
}

fn semantic_label(node: &SemanticNode) -> Option<String> {
    node.name
        .clone()
        .or_else(|| node.value.clone())
        .or_else(|| {
            let mut labels = Vec::new();
            collect_text_labels(node, &mut labels);
            labels.sort();
            labels.dedup();
            (labels.len() == 1).then(|| labels[0].clone())
        })
}

fn collect_text_labels(node: &SemanticNode, labels: &mut Vec<String>) {
    for child in &node.children {
        if matches!(child.role, SemanticRole::Label | SemanticRole::Text) {
            if let Some(label) = child.name.clone().or_else(|| child.value.clone()) {
                labels.push(label);
            }
        } else if !is_direct_control(child) {
            collect_text_labels(child, labels);
        }
    }
}

fn unique_descendant_text_input(node: &SemanticNode) -> Option<&SemanticNode> {
    fn collect<'a>(node: &'a SemanticNode, inputs: &mut Vec<&'a SemanticNode>) {
        for child in &node.children {
            if child.role == SemanticRole::TextInput {
                inputs.push(child);
            } else if !is_direct_control(child) {
                collect(child, inputs);
            }
        }
    }
    let mut inputs = Vec::new();
    collect(node, &mut inputs);
    if inputs.len() == 1 {
        Some(inputs[0])
    } else {
        None
    }
}

fn is_direct_control(node: &SemanticNode) -> bool {
    matches!(
        node.role,
        SemanticRole::Button
            | SemanticRole::ToggleButton
            | SemanticRole::CheckBox
            | SemanticRole::RadioButton
            | SemanticRole::TextInput
            | SemanticRole::ComboBox
            | SemanticRole::ListItem
            | SemanticRole::MenuItem
    ) || (node.role == SemanticRole::Slider
        && node.capabilities.contains(&SemanticCapability::Value))
}

fn is_command_container(node: &SemanticNode) -> bool {
    node.role == SemanticRole::MenuBar
        || matches!(&node.role, SemanticRole::Unknown(role) if role == "tool bar" || role == "toolbar")
        || (matches!(node.role, SemanticRole::Container)
            && node.children.len() >= 3
            && node.children.iter().all(|child| {
                matches!(child.role, SemanticRole::Button | SemanticRole::MenuItem)
                    && !child.actions.is_empty()
            }))
}

fn is_opaque_candidate(node: &SemanticNode) -> bool {
    let SemanticRole::Unknown(original) = &node.role else {
        return false;
    };
    let graphical_role = matches!(
        original.as_str(),
        "drawing area" | "canvas" | "image" | "video" | "animation" | "3d view"
    );
    graphical_role
        && node.actions.is_empty()
        && node.value.is_none()
        && !node.children.iter().any(has_semantic_signal)
}

fn has_semantic_signal(node: &SemanticNode) -> bool {
    node.name.is_some()
        || node.value.is_some()
        || !node.actions.is_empty()
        || is_direct_control(node)
        || node.children.iter().any(has_semantic_signal)
}

fn has_interactive_descendant(node: &SemanticNode) -> bool {
    is_direct_control(node) || node.children.iter().any(has_interactive_descendant)
}

fn intent_for_capability(capability: InteractionCapability) -> Option<UiIntent> {
    match capability {
        InteractionCapability::None => None,
        InteractionCapability::Activate => Some(UiIntent::Activate),
        InteractionCapability::Toggle => Some(UiIntent::Toggle),
        InteractionCapability::Select => Some(UiIntent::Select),
        InteractionCapability::Choose => Some(UiIntent::BeginChoice),
        InteractionCapability::OpenMenu => Some(UiIntent::OpenMenu),
        InteractionCapability::EditText => Some(UiIntent::BeginEdit),
        InteractionCapability::AdjustValue => Some(UiIntent::IncreaseValue),
        InteractionCapability::BrowseContent => Some(UiIntent::BeginRead),
    }
}

fn count_nodes(node: &SemanticNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

fn count_interactive(
    node: &SemanticNode,
    parent_capabilities: &[crate::semantic::SemanticCapability],
) -> usize {
    let own = usize::from(
        interaction_capability(
            &node.role,
            &node.actions,
            &node.capabilities,
            parent_capabilities,
        ) != InteractionCapability::None,
    );
    own + node
        .children
        .iter()
        .map(|child| count_interactive(child, &node.capabilities))
        .sum::<usize>()
}

fn count_regions(region: &SemanticRegion) -> usize {
    1 + region.children.iter().map(count_regions).sum::<usize>()
}

pub fn format_regions(analysis: &RegionAnalysis) -> String {
    let mut output = String::new();
    format_region(&analysis.root, 0, &mut output);
    output.push_str(&format!(
        "Metrics semantic_nodes={} interactive_nodes={} regions={} direct={} reconstructed={} compressed={} commands={} selections={} opaque={} unsupported={}\n",
        analysis.metrics.semantic_nodes,
        analysis.metrics.interactive_nodes,
        analysis.metrics.regions,
        analysis.metrics.direct_controls,
        analysis.metrics.reconstructed,
        analysis.metrics.compressed,
        analysis.metrics.command_regions,
        analysis.metrics.selection_regions,
        analysis.metrics.opaque_regions,
        analysis.metrics.unsupported_regions,
    ));
    output
}

fn format_region(region: &SemanticRegion, depth: usize, output: &mut String) {
    let indent = "  ".repeat(depth);
    output.push_str(&format!(
        "{indent}Region {} kind={} label={:?} sources={:?} confidence={:?} modality={:?}",
        region.id,
        region.kind,
        region.label,
        region
            .source_nodes
            .iter()
            .map(|id| id.get())
            .collect::<Vec<_>>(),
        region.confidence,
        region.modality,
    ));
    if !region.command_path.is_empty() {
        output.push_str(&format!(" path={:?}", region.command_path));
    }
    if !region.descriptions.is_empty() {
        output.push_str(&format!(" descriptions={:?}", region.descriptions));
    }
    if !region.errors.is_empty() {
        output.push_str(&format!(" errors={:?}", region.errors));
    }
    if !region.logical_group.is_empty() {
        output.push_str(&format!(" logical_group={:?}", region.logical_group));
    }
    output.push('\n');
    for child in &region.children {
        format_region(child, depth + 1, output);
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic::{
        BackendLocator, BackendRelation, DebugInfo, RelationalSemanticGraph, RuntimeNodeId,
        SemanticAction, SemanticCache, SemanticCapability, SemanticRelationKind, SemanticState,
        TextInputKind,
    };

    use super::*;

    fn node(id: u64, role: SemanticRole, name: &str) -> SemanticNode {
        SemanticNode {
            runtime_id: RuntimeNodeId::new(id),
            backend_locator: BackendLocator::new(":1.2", format!("/node/{id}")),
            index_in_parent: None,
            role,
            name: (!name.is_empty()).then(|| name.to_owned()),
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
    fn label_wrapper_preserves_embedded_interactive_control() {
        let mut root = node(0, SemanticRole::Window, "Browser");
        let mut label = node(1, SemanticRole::Label, "");
        label.children = vec![
            node(2, SemanticRole::Label, "Demo items"),
            node(3, SemanticRole::ComboBox, "Demo items"),
        ];
        root.children.push(label);
        let analysis = analyze_regions(&root);
        fn has_combo(region: &SemanticRegion) -> bool {
            region.source_nodes.contains(&RuntimeNodeId::new(3))
                || region.children.iter().any(has_combo)
        }
        assert!(has_combo(&analysis.root));
    }

    fn action(name: &str) -> SemanticAction {
        SemanticAction {
            index: 0,
            name: name.to_owned(),
            description: None,
            keybinding: None,
        }
    }

    #[test]
    fn labeled_fields_and_multiple_fields_form_a_form_region() {
        let mut root = node(0, SemanticRole::Window, "Profile");
        let label_a = node(1, SemanticRole::Label, "Username");
        let mut input_a = node(2, SemanticRole::TextInput, "Username");
        input_a.text_input_kind = Some(TextInputKind::Plain);
        let label_b = node(3, SemanticRole::Label, "Password");
        let mut input_b = node(4, SemanticRole::TextInput, "Password");
        input_b.text_input_kind = Some(TextInputKind::Password);
        root.children = vec![label_a, input_a, label_b, input_b];

        let result = analyze_regions(&root);
        assert_eq!(result.root.kind, SemanticRegionKind::Form);
        assert_eq!(result.root.children[0].kind, SemanticRegionKind::Field);
        assert_eq!(result.root.children[0].source_nodes.len(), 2);
        assert_eq!(result.root.children[1].kind, SemanticRegionKind::Field);
    }

    #[test]
    fn wrapping_html_style_label_reconstructs_a_field_without_losing_input() {
        let mut root = node(0, SemanticRole::Window, "Browser");
        let mut label = node(1, SemanticRole::Label, "Username");
        let mut wrapper = node(2, SemanticRole::Container, "");
        let mut input = node(3, SemanticRole::TextInput, "Username");
        input.text_input_kind = Some(TextInputKind::Plain);
        wrapper.children.push(input);
        label.children.push(wrapper);
        root.children.push(label);

        let result = analyze_regions(&root);
        assert_eq!(result.root.children[0].kind, SemanticRegionKind::Field);
        assert_eq!(
            result.root.children[0].source_nodes,
            vec![RuntimeNodeId::new(1), RuntimeNodeId::new(3)]
        );
    }

    #[test]
    fn consecutive_content_nodes_are_structurally_compressed() {
        let mut root = node(0, SemanticRole::Window, "Summary");
        root.children = vec![
            node(1, SemanticRole::Label, "CPU"),
            node(2, SemanticRole::Label, "23%"),
            node(3, SemanticRole::Label, "Memory"),
        ];
        let result = analyze_regions(&root);
        assert_eq!(result.root.children.len(), 1);
        assert_eq!(result.root.children[0].source_nodes.len(), 3);
        assert_eq!(
            result.root.children[0].label.as_deref(),
            Some("CPU · 23% · Memory")
        );
        assert_eq!(result.metrics.compressed, 2);
    }

    #[test]
    fn ambiguous_duplicate_labels_are_not_merged() {
        let mut root = node(0, SemanticRole::Container, "");
        root.children = vec![
            node(1, SemanticRole::Label, "Item"),
            node(2, SemanticRole::TextInput, "Item"),
            node(3, SemanticRole::Label, "Item"),
        ];
        let result = analyze_regions(&root);
        assert_ne!(result.root.children[0].kind, SemanticRegionKind::Field);
    }

    #[test]
    fn menu_and_toolbar_become_command_sets_with_paths() {
        let mut menu = node(1, SemanticRole::MenuBar, "Commands");
        let mut file = node(2, SemanticRole::MenuItem, "File");
        let mut save = node(3, SemanticRole::MenuItem, "Save");
        save.actions.push(action("Press"));
        file.children.push(save);
        menu.children.push(file);
        let result = analyze_regions(&menu);
        assert_eq!(result.root.kind, SemanticRegionKind::CommandSet);
        assert!(
            result.root.children[0]
                .command_path
                .contains(&"File".to_owned())
        );
    }

    #[test]
    fn list_is_selection_and_parent_capability_preserves_item_interaction() {
        let mut list = node(1, SemanticRole::List, "Items");
        list.capabilities.push(SemanticCapability::SelectChildren);
        let mut beta = node(2, SemanticRole::ListItem, "Beta");
        beta.index_in_parent = Some(1);
        list.children.push(beta);
        let result = analyze_regions(&list);
        assert_eq!(result.root.kind, SemanticRegionKind::Selection);
        assert_eq!(
            result.root.children[0].interactions[0].intent,
            UiIntent::Select
        );
    }

    #[test]
    fn uniquely_contiguous_radio_run_forms_a_labeled_selection_group() {
        let mut root = node(0, SemanticRole::Container, "");
        let mut light = node(2, SemanticRole::RadioButton, "Light");
        light.actions.push(action("Toggle"));
        let mut dark = node(3, SemanticRole::RadioButton, "Dark");
        dark.actions.push(action("Toggle"));
        root.children = vec![
            node(1, SemanticRole::Label, "Theme"),
            light,
            dark,
            node(4, SemanticRole::Button, "Apply"),
        ];

        let result = analyze_regions(&root);
        let group = &result.root.children[0];
        assert_eq!(group.kind, SemanticRegionKind::Selection);
        assert_eq!(group.label.as_deref(), Some("Theme"));
        assert_eq!(group.children.len(), 2);
        assert!(
            group
                .children
                .iter()
                .all(|child| child.interactions[0].intent == UiIntent::Toggle)
        );
    }

    #[test]
    fn sparse_graphics_are_opaque_but_action_rich_unknown_is_not() {
        let mut canvas = node(
            1,
            SemanticRole::Unknown("drawing area".to_owned()),
            "Preview",
        );
        let result = analyze_regions(&canvas);
        assert_eq!(result.root.kind, SemanticRegionKind::OpaqueContent);
        assert_eq!(result.root.modality, ModalityPolicy::FidelityPreferred);

        canvas.actions.push(action("Zoom"));
        let result = analyze_regions(&canvas);
        assert_ne!(result.root.kind, SemanticRegionKind::OpaqueContent);
    }

    #[test]
    fn state_only_unknown_is_not_mistaken_for_opaque() {
        let mut unknown = node(1, SemanticRole::Unknown("custom".to_owned()), "Widget");
        unknown.states.push(SemanticState::Enabled);
        let result = analyze_regions(&unknown);
        assert_eq!(result.root.kind, SemanticRegionKind::Unknown);
    }

    #[test]
    fn explicit_relation_overrides_adjacency_and_carries_description_and_error() {
        let mut root = node(0, SemanticRole::Window, "Form");
        let explicit = node(1, SemanticRole::Label, "Explicit username");
        let adjacent = node(2, SemanticRole::Label, "Adjacent guess");
        let mut input = node(3, SemanticRole::TextInput, "Adjacent guess");
        input.text_input_kind = Some(TextInputKind::Plain);
        let hint = node(4, SemanticRole::Label, "Required account name");
        let error = node(5, SemanticRole::Label, "Invalid account name");
        root.children = vec![explicit, adjacent, input, hint, error];
        let mut cache = SemanticCache::from_snapshot(root).unwrap();
        let find = |cache: &SemanticCache, name: &str| {
            cache
                .nodes()
                .find(|node| node.name.as_deref() == Some(name))
                .unwrap()
                .runtime_id
        };
        let input_id = cache
            .nodes()
            .find(|node| node.role == SemanticRole::TextInput)
            .unwrap()
            .runtime_id;
        let targets = [
            (SemanticRelationKind::LabelledBy, "Explicit username"),
            (SemanticRelationKind::DescribedBy, "Required account name"),
            (SemanticRelationKind::ErrorMessage, "Invalid account name"),
        ]
        .into_iter()
        .map(|(kind, name)| BackendRelation {
            kind,
            targets: vec![
                cache
                    .node(find(&cache, name))
                    .unwrap()
                    .backend_locator
                    .clone(),
            ],
        })
        .collect();
        cache.set_relations(input_id, targets).unwrap();
        let tree = cache.materialize_tree().unwrap();
        let analysis = analyze_regions_with_graph(&tree, &RelationalSemanticGraph::new(&cache));
        let field = analysis
            .root
            .children
            .iter()
            .find(|region| region.kind == SemanticRegionKind::Field)
            .unwrap();
        assert_eq!(field.label.as_deref(), Some("Explicit username"));
        assert_eq!(field.confidence, RegionConfidence::Exact);
        assert_eq!(field.descriptions, vec!["Required account name"]);
        assert_eq!(field.errors, vec!["Invalid account name"]);
    }

    #[test]
    fn combo_disclosure_child_is_not_promoted_to_a_production_interaction() {
        let mut combo = node(1, SemanticRole::ComboBox, "Choice");
        let mut disclosure = node(2, SemanticRole::ToggleButton, "Alpha");
        disclosure.actions.push(action("Click"));
        combo.children.push(disclosure);
        let analysis = analyze_regions(&combo);
        assert!(analysis.root.interactions.is_empty());

        let anonymous = node(3, SemanticRole::ComboBox, "Browser choice");
        let analysis = analyze_regions(&anonymous);
        assert!(analysis.root.interactions.is_empty());
    }
}
