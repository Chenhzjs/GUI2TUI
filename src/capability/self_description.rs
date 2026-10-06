//! Bounded acquisition of GUI claims. Text is evidence, never authority.
use super::*;

pub const MAX_DESCRIPTION_CLAIMS: usize = 256;
pub const MAX_DESCRIPTION_CHARS: usize = 2048;
pub const MAX_SHORTCUTS_PER_TEXT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionOrigin {
    RuntimeDeclared,
    Menu,
    InterfaceDescription,
    ShortcutSurface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionStatus {
    Described,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShortcutMention {
    pub text: String,
    /// None preserves unsupported F-keys as textual evidence, not guessed input.
    pub primitive: Option<KeyboardPrimitive>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptionClaim {
    pub source: BackendLocator,
    pub origin: DescriptionOrigin,
    pub status: DescriptionStatus,
    pub description: String,
    pub shortcut: ShortcutMention,
    pub enabled: bool,
    pub risk: RiskClass,
    pub verification: VerificationPlan,
    pub confidence_basis: &'static str,
    /// A description's source is NOT necessarily the execution target.
    pub target_grounded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CapabilityEnumerationSurface {
    pub locator: String,
    pub kind: String,
    pub evidence: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelfDescriptionCatalog {
    pub claims: Vec<DescriptionClaim>,
    pub surfaces: Vec<CapabilityEnumerationSurface>,
    pub inspected_objects: usize,
    pub description_objects: usize,
    pub truncated: bool,
}

fn modifier(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "ctrl" | "control" | "alt" | "shift" | "meta" | "super" | "cmd" | "command"
    )
}

/// Conservative lexical grammar, independent of app, language or task oracle.
pub fn extract_shortcuts(text: &str) -> Vec<ShortcutMention> {
    let normalized = text
        .chars()
        .take(MAX_DESCRIPTION_CHARS)
        .collect::<String>()
        .replace('⌘', "Meta+")
        .replace('⇧', "Shift+")
        .replace('⌥', "Alt+")
        .replace('⌃', "Ctrl+");
    let words: Vec<_> = normalized
        .split(|c: char| !c.is_alphanumeric() && c != '+' && c != '-')
        .filter(|word| !word.is_empty())
        .collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < words.len() && result.len() < MAX_SHORTCUTS_PER_TEXT {
        let start = index;
        let first = words[index].split(['+', '-']).next().unwrap_or("");
        index += 1;
        // Bare prose "command Ctrl+K" is not a Meta modifier. Symbols and
        // explicit Command+K/Cmd+K remain supported.
        if matches!(
            words[start].to_ascii_lowercase().as_str(),
            "command" | "cmd"
        ) {
            continue;
        }
        if !modifier(first) {
            continue;
        }
        let mut chord = words[start].replace('-', "+");
        while index < words.len() && (chord.ends_with('+') || chord.split('+').all(modifier)) {
            if !chord.ends_with('+') {
                chord.push('+');
            }
            chord.push_str(words[index]);
            index += 1;
        }
        let parts: Vec<_> = chord.split('+').collect();
        let Some((key, modifiers)) = parts.split_last() else {
            continue;
        };
        if modifiers.is_empty() || !modifiers.iter().all(|m| modifier(m)) {
            continue;
        }
        let function_key = key
            .strip_prefix('F')
            .or_else(|| key.strip_prefix('f'))
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n));
        let canonical = parts
            .iter()
            .map(|part| match part.to_ascii_lowercase().as_str() {
                "cmd" | "command" => "Meta",
                _ => *part,
            })
            .collect::<Vec<_>>()
            .join("+");
        let primitive = parse_advertised_keybinding(&canonical);
        if primitive.is_some() || function_key {
            let mention = ShortcutMention {
                text: chord,
                primitive,
            };
            if !result.contains(&mention) {
                result.push(mention);
            }
        }
    }
    result
}

pub fn is_publicly_visible(object: &RuntimeObject) -> bool {
    object
        .states
        .iter()
        .any(|state| matches!(state, SemanticState::Other(s) if s.eq_ignore_ascii_case("showing") || s.eq_ignore_ascii_case("visible")))
}

fn menu_role(role: &SemanticRole) -> bool {
    matches!(
        role,
        SemanticRole::MenuBar | SemanticRole::Menu | SemanticRole::MenuItem
    )
}

impl SelfDescriptionCatalog {
    pub fn mine(objects: &RuntimeObjectModel) -> Self {
        let mut catalog = Self::default();
        for object in objects.objects() {
            if object
                .text
                .as_ref()
                .is_some_and(|text| text.kind == TextInputKind::Password)
                || !is_publicly_visible(object)
            {
                continue;
            }
            catalog.inspected_objects += 1;
            if object.description.as_ref().is_some_and(|d| !d.is_empty()) {
                catalog.description_objects += 1;
            }
            let parent = object.parent.and_then(|id| objects.object(id));
            let in_menu = menu_role(&object.role) || parent.is_some_and(|p| menu_role(&p.role));
            let action_children = object
                .children
                .iter()
                .filter_map(|id| objects.object(*id))
                .filter(|child| !child.advertised_actions.is_empty())
                .count();
            let shortcut_children = object
                .children
                .iter()
                .filter_map(|id| objects.object(*id))
                .filter(|child| {
                    child
                        .name
                        .as_deref()
                        .is_some_and(|s| !extract_shortcuts(s).is_empty())
                })
                .count();
            if (menu_role(&object.role) || action_children >= 3 || shortcut_children >= 2)
                && catalog.surfaces.len() < 64
            {
                catalog.surfaces.push(CapabilityEnumerationSurface {
                    locator: object.backend_locator.encode(),
                    kind: if in_menu { "menu" } else { "action_or_shortcut_list" }.to_owned(),
                    evidence: format!("role={} action_children={action_children} shortcut_children={shortcut_children}", object.role),
                });
            }
            let mut sources = Vec::new();
            if let Some(name) = &object.name {
                sources.push((
                    if in_menu {
                        DescriptionOrigin::Menu
                    } else {
                        DescriptionOrigin::ShortcutSurface
                    },
                    name.as_str(),
                ));
            }
            if let Some(description) = &object.description {
                sources.push((
                    if in_menu {
                        DescriptionOrigin::Menu
                    } else {
                        DescriptionOrigin::InterfaceDescription
                    },
                    description.as_str(),
                ));
            }
            for action in &object.advertised_actions {
                if let Some(binding) = &action.keybinding {
                    sources.push((DescriptionOrigin::RuntimeDeclared, binding.as_str()));
                }
            }
            for (origin, text) in sources {
                for shortcut in extract_shortcuts(text) {
                    if catalog.claims.len() == MAX_DESCRIPTION_CLAIMS {
                        catalog.truncated = true;
                        return catalog;
                    }
                    // Language hints only make risk MORE restrictive, never safe.
                    let lower = text.to_lowercase();
                    let external = [
                        "export", "save", "send", "print", "download", "保存", "导出", "发送",
                        "打印",
                    ]
                    .iter()
                    .any(|word| lower.contains(word));
                    let enabled = object.states.contains(&SemanticState::Enabled);
                    catalog.claims.push(DescriptionClaim {
                        source: object.backend_locator.clone(),
                        origin,
                        status: if enabled {
                            DescriptionStatus::Described
                        } else {
                            DescriptionStatus::Rejected
                        },
                        description: text.chars().take(MAX_DESCRIPTION_CHARS).collect(),
                        shortcut,
                        enabled,
                        risk: if external {
                            RiskClass::ExternalEffect
                        } else {
                            RiskClass::Unknown
                        },
                        verification: VerificationPlan::Unavailable,
                        confidence_basis: match origin {
                            DescriptionOrigin::RuntimeDeclared => {
                                "public binding assertion; execution unverified"
                            }
                            DescriptionOrigin::Menu => {
                                "menu hierarchy plus shortcut grammar; target unresolved"
                            }
                            _ => "text grammar only; may be example or context-inapplicable",
                        },
                        target_grounded: false,
                    });
                }
            }
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_grammar_preserves_unsupported_and_rejects_sequences() {
        for text in [
            "Ctrl+K",
            "Ctrl K",
            "Ctrl-K",
            "Alt+Enter",
            "Ctrl+Shift+P",
            "⌘K",
            "⇧⌘P",
            "⌥K",
        ] {
            assert!(
                extract_shortcuts(text).first().unwrap().primitive.is_some(),
                "{text}"
            );
        }
        assert_eq!(extract_shortcuts("Shift+F10")[0].primitive, None);
        assert!(extract_shortcuts("Ctrl+K+X; normal documentation").is_empty());
        assert_eq!(extract_shortcuts("Press Ctrl K to preview").len(), 1);
        assert!(extract_shortcuts("a plain K is not a binding").is_empty());
        assert_eq!(
            extract_shortcuts("Unavailable command Ctrl+Shift+D")[0].text,
            "Ctrl+Shift+D"
        );
    }
}
