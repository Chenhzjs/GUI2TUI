use std::fmt::Write as _;

use crate::semantic::{SemanticNode, SemanticState};

#[derive(Clone, Copy, Debug, Default)]
pub struct FormatOptions {
    pub verbose: bool,
}

pub fn format_tree(root: &SemanticNode, options: FormatOptions) -> String {
    let mut output = String::new();
    format_node(root, "", None, options, &mut output);
    output
}

fn format_node(
    node: &SemanticNode,
    prefix: &str,
    branch: Option<bool>,
    options: FormatOptions,
    output: &mut String,
) {
    output.push_str(prefix);
    if let Some(is_last) = branch {
        output.push_str(if is_last { "└── " } else { "├── " });
    }
    write_node_summary(node, options, output);
    output.push('\n');

    let child_prefix = if branch.is_none() {
        String::new()
    } else {
        format!(
            "{prefix}{}",
            if branch == Some(true) {
                "    "
            } else {
                "│   "
            }
        )
    };
    let item_count = node.children.len() + node.truncations.len();
    for (index, child) in node.children.iter().enumerate() {
        format_node(
            child,
            &child_prefix,
            Some(index + 1 == item_count),
            options,
            output,
        );
    }
    for (index, truncation) in node.truncations.iter().enumerate() {
        output.push_str(&child_prefix);
        output.push_str(if node.children.len() + index + 1 == item_count {
            "└── "
        } else {
            "├── "
        });
        let _ = writeln!(output, "… [{truncation}]");
    }
}

fn write_node_summary(node: &SemanticNode, options: FormatOptions, output: &mut String) {
    let _ = write!(output, "{}", node.role);
    if let Some(name) = &node.name {
        let _ = write!(output, " \"{}\"", escape(name));
    }
    if let Some(value) = &node.value {
        let _ = write!(output, " value=\"{}\"", escape(value));
    }

    let states: Vec<_> = node
        .states
        .iter()
        .filter(|state| options.verbose || is_salient_state(state))
        .map(ToString::to_string)
        .collect();
    if !states.is_empty() {
        let _ = write!(output, " [{}]", states.join(","));
    }

    if !node.actions.is_empty() {
        let action_names = node
            .actions
            .iter()
            .map(|action| action.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(output, " actions=[{action_names}]");
    }

    if !node.actions.is_empty() || options.verbose {
        let _ = write!(output, " id={}", node.id);
    }

    if options.verbose {
        if let Some(description) = &node.description {
            let _ = write!(output, " description=\"{}\"", escape(description));
        }
        let _ = write!(
            output,
            " {{atspi-role=\"{}\", bus=\"{}\", path=\"{}\", interfaces=[{}]",
            escape(&node.debug.atspi_role),
            escape(&node.debug.bus_name),
            escape(&node.debug.object_path),
            node.debug.interfaces.join(",")
        );
        if let Some(geometry) = &node.debug.geometry {
            let _ = write!(
                output,
                ", geometry=({},{} {}x{})",
                geometry.x, geometry.y, geometry.width, geometry.height
            );
        }
        output.push('}');
    }
}

fn is_salient_state(state: &SemanticState) -> bool {
    match state {
        SemanticState::Enabled => false,
        SemanticState::Other(value) => !matches!(
            value.as_str(),
            "sensitive" | "showing" | "visible" | "focusable" | "selectable"
        ),
        _ => true,
    }
}

fn escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| character.escape_default())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::semantic::{DebugInfo, NodeId, SemanticAction, SemanticRole};

    use super::*;

    fn node(role: SemanticRole, name: Option<&str>) -> SemanticNode {
        SemanticNode {
            id: NodeId::new(":1.2", "/node"),
            role,
            name: name.map(str::to_owned),
            description: None,
            value: None,
            states: Vec::new(),
            actions: Vec::new(),
            children: Vec::new(),
            truncations: Vec::new(),
            debug: DebugInfo {
                atspi_role: "panel".to_owned(),
                bus_name: ":1.2".to_owned(),
                object_path: "/node".to_owned(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn formats_tree_connectors_and_semantics() {
        let mut root = node(SemanticRole::Window, Some("Settings"));
        let mut checkbox = node(SemanticRole::CheckBox, Some("Enable proxy"));
        checkbox.states = vec![SemanticState::Enabled, SemanticState::Checked];
        let mut button = node(SemanticRole::Button, Some("Apply"));
        button.actions = vec![SemanticAction {
            index: 0,
            name: "press".to_owned(),
            description: None,
            keybinding: None,
        }];
        root.children = vec![checkbox, button];

        let output = format_tree(&root, FormatOptions::default());
        assert!(output.starts_with("Window \"Settings\"\n"));
        assert!(output.contains("├── CheckBox \"Enable proxy\" [checked]\n"));
        assert!(output.contains("└── Button \"Apply\" actions=[press] id=atspi1_"));
        assert!(!output.contains("enabled"));
    }

    #[test]
    fn verbose_format_includes_backend_debug_data() {
        let root = node(SemanticRole::Container, None);
        let output = format_tree(&root, FormatOptions { verbose: true });
        assert!(output.contains("id=atspi1_"));
        assert!(output.contains("atspi-role=\"panel\""));
        assert!(output.contains("bus=\":1.2\""));
        assert!(output.contains("path=\"/node\""));
    }

    #[test]
    fn escapes_untrusted_accessible_text() {
        let root = node(SemanticRole::Label, Some("a\"b\n"));
        assert!(format_tree(&root, FormatOptions::default()).contains("\"a\\\"b\\n\""));
    }

    #[test]
    fn renders_depth_and_node_limit_truncation_markers() {
        let mut root = node(SemanticRole::Window, Some("Large tree"));
        root.children = vec![node(SemanticRole::Button, Some("Visible"))];
        root.truncations = vec![
            crate::semantic::TreeTruncation::MaxDepth { limit: 3 },
            crate::semantic::TreeTruncation::MaxNodes { limit: 5000 },
        ];

        let output = format_tree(&root, FormatOptions::default());
        assert!(output.contains("├── Button \"Visible\""));
        assert!(output.contains("├── … [tree truncated: max depth=3]"));
        assert!(output.contains("└── … [tree truncated: max nodes=5000]"));
    }
}
