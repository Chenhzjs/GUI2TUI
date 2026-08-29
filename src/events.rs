use std::fmt;

use atspi::{
    EventProperties, EventTypeProperties,
    events::{CacheEvents, Event, ObjectEvents, WindowEvents},
};

use crate::semantic::BackendLocator;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NormalizedEvent {
    NodeStateChanged {
        locator: BackendLocator,
        state: String,
        enabled: bool,
    },
    NodePropertyChanged {
        locator: BackendLocator,
        property: String,
    },
    ChildrenChanged {
        parent: BackendLocator,
        change: String,
        index: i32,
        child: Option<BackendLocator>,
    },
    SelectionChanged {
        container: BackendLocator,
    },
    ActiveDescendantChanged {
        container: BackendLocator,
        descendant: Option<BackendLocator>,
    },
    TextChanged {
        locator: BackendLocator,
        change: String,
        start: i32,
        length: i32,
    },
    WindowCreated {
        locator: BackendLocator,
    },
    WindowDestroyed {
        locator: BackendLocator,
    },
    CacheAdded {
        locator: BackendLocator,
    },
    CacheRemoved {
        locator: BackendLocator,
    },
    Unknown {
        locator: BackendLocator,
        interface: String,
        member: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DirtyScope {
    Node(BackendLocator),
    Subtree(BackendLocator),
    Application,
}

impl DirtyScope {
    pub fn from_event(event: &NormalizedEvent) -> Self {
        match event {
            NormalizedEvent::NodeStateChanged { locator, .. }
            | NormalizedEvent::NodePropertyChanged { locator, .. }
            | NormalizedEvent::TextChanged { locator, .. } => Self::Node(locator.clone()),
            NormalizedEvent::ChildrenChanged { parent, .. }
            | NormalizedEvent::SelectionChanged { container: parent }
            | NormalizedEvent::ActiveDescendantChanged {
                container: parent, ..
            } => Self::Subtree(parent.clone()),
            NormalizedEvent::WindowCreated { .. }
            | NormalizedEvent::WindowDestroyed { .. }
            | NormalizedEvent::CacheAdded { .. }
            | NormalizedEvent::CacheRemoved { .. }
            | NormalizedEvent::Unknown { .. } => Self::Application,
        }
    }
}

/// Coalesce one short event burst into the smallest correctness-preserving refresh set.
pub fn coalesce_dirty_scopes(events: &[NormalizedEvent]) -> Vec<DirtyScope> {
    let has_structural_event = events
        .iter()
        .any(|event| matches!(event, NormalizedEvent::ChildrenChanged { .. }));
    let structurally_added_or_removed: std::collections::HashSet<_> = events
        .iter()
        .filter_map(|event| match event {
            NormalizedEvent::ChildrenChanged { child, .. } => child.clone(),
            _ => None,
        })
        .collect();
    let mut scopes = Vec::new();
    for event in events {
        if has_structural_event
            && matches!(
                event,
                NormalizedEvent::CacheAdded { .. } | NormalizedEvent::CacheRemoved { .. }
            )
        {
            continue;
        }
        let scope = DirtyScope::from_event(event);
        if matches!(&scope, DirtyScope::Node(locator) if structurally_added_or_removed.contains(locator))
        {
            // Refreshing the parent subtree materializes (or removes) the
            // transient child. A state/property echo sourced by that child can
            // otherwise race ahead and look like an impossible unknown node.
            continue;
        }
        if scope == DirtyScope::Application {
            return vec![DirtyScope::Application];
        }
        match &scope {
            DirtyScope::Subtree(locator) => {
                scopes.retain(
                    |existing| !matches!(existing, DirtyScope::Node(node) if node == locator),
                );
                if !scopes.contains(&scope) {
                    scopes.push(scope);
                }
            }
            DirtyScope::Node(locator) => {
                if !scopes.iter().any(
                    |existing| matches!(existing, DirtyScope::Subtree(root) if root == locator),
                ) && !scopes.contains(&scope)
                {
                    scopes.push(scope);
                }
            }
            DirtyScope::Application => unreachable!(),
        }
    }
    scopes
}

impl NormalizedEvent {
    pub fn from_atspi(event: &Event) -> Self {
        let source = locator_from_event(event);
        match event {
            Event::Object(ObjectEvents::StateChanged(event)) => Self::NodeStateChanged {
                locator: source,
                state: format!("{:?}", event.state).to_lowercase(),
                enabled: event.enabled,
            },
            Event::Object(ObjectEvents::PropertyChange(event)) => Self::NodePropertyChanged {
                locator: source,
                property: event.property.clone(),
            },
            Event::Object(ObjectEvents::ChildrenChanged(event)) => Self::ChildrenChanged {
                parent: source,
                change: format!("{:?}", event.operation).to_lowercase(),
                index: event.index_in_parent,
                child: locator_from_object_ref(&event.child),
            },
            Event::Object(ObjectEvents::SelectionChanged(_)) => {
                Self::SelectionChanged { container: source }
            }
            Event::Object(ObjectEvents::ActiveDescendantChanged(event)) => {
                Self::ActiveDescendantChanged {
                    container: source,
                    descendant: locator_from_object_ref(&event.descendant),
                }
            }
            Event::Object(ObjectEvents::TextChanged(event)) => Self::TextChanged {
                locator: source,
                change: format!("{:?}", event.operation).to_lowercase(),
                start: event.start_pos,
                length: event.length,
            },
            Event::Window(WindowEvents::Create(_)) => Self::WindowCreated { locator: source },
            Event::Window(WindowEvents::Destroy(_) | WindowEvents::Close(_)) => {
                Self::WindowDestroyed { locator: source }
            }
            Event::Cache(CacheEvents::Add(event)) => Self::CacheAdded {
                locator: locator_from_object_ref(&event.node_added.object).unwrap_or(source),
            },
            Event::Cache(CacheEvents::LegacyAdd(event)) => Self::CacheAdded {
                locator: locator_from_object_ref(&event.node_added.object).unwrap_or(source),
            },
            Event::Cache(CacheEvents::Remove(event)) => Self::CacheRemoved {
                locator: locator_from_object_ref(&event.node_removed).unwrap_or(source),
            },
            _ => Self::Unknown {
                locator: source,
                interface: event.interface().to_owned(),
                member: event.member().to_owned(),
            },
        }
    }

    pub fn source(&self) -> &BackendLocator {
        match self {
            Self::NodeStateChanged { locator, .. }
            | Self::NodePropertyChanged { locator, .. }
            | Self::TextChanged { locator, .. }
            | Self::WindowCreated { locator }
            | Self::WindowDestroyed { locator }
            | Self::CacheAdded { locator }
            | Self::CacheRemoved { locator }
            | Self::Unknown { locator, .. } => locator,
            Self::ChildrenChanged { parent, .. } => parent,
            Self::SelectionChanged { container }
            | Self::ActiveDescendantChanged { container, .. } => container,
        }
    }
}

impl fmt::Display for NormalizedEvent {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeStateChanged {
                locator,
                state,
                enabled,
            } => write!(
                output,
                "kind=state-changed\nsource={locator}\ndetail={state}\nenabled={enabled}"
            ),
            Self::NodePropertyChanged { locator, property } => write!(
                output,
                "kind=property-change\nsource={locator}\nproperty={property}"
            ),
            Self::ChildrenChanged {
                parent,
                change,
                index,
                child,
            } => write!(
                output,
                "kind=children-changed\nsource={parent}\nchange={change}\nindex={index}\nchild={}",
                child
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<null>".to_owned())
            ),
            Self::SelectionChanged { container } => {
                write!(output, "kind=selection-changed\nsource={container}")
            }
            Self::ActiveDescendantChanged {
                container,
                descendant,
            } => write!(
                output,
                "kind=active-descendant-changed\nsource={container}\ndescendant={}",
                descendant
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<null>".to_owned())
            ),
            Self::TextChanged {
                locator,
                change,
                start,
                length,
            } => write!(
                output,
                "kind=text-changed\nsource={locator}\nchange={change}\nstart={start}\nlength={length}"
            ),
            Self::WindowCreated { locator } => {
                write!(output, "kind=window-created\nsource={locator}")
            }
            Self::WindowDestroyed { locator } => {
                write!(output, "kind=window-destroyed\nsource={locator}")
            }
            Self::CacheAdded { locator } => {
                write!(output, "kind=cache-added\nsource={locator}")
            }
            Self::CacheRemoved { locator } => {
                write!(output, "kind=cache-removed\nsource={locator}")
            }
            Self::Unknown {
                locator,
                interface,
                member,
            } => write!(
                output,
                "kind=unknown\nsource={locator}\ninterface={interface}\nmember={member}"
            ),
        }
    }
}

fn locator_from_event(event: &Event) -> BackendLocator {
    BackendLocator::new(event.sender().as_str(), event.path().as_str())
}

fn locator_from_object_ref(object: &atspi::ObjectRefOwned) -> Option<BackendLocator> {
    Some(BackendLocator::new(
        object.name_as_str()?,
        object.path_as_str(),
    ))
}

#[cfg(test)]
mod tests {
    use atspi::{
        ObjectRefOwned, Operation, State,
        events::{
            object::{
                ActiveDescendantChangedEvent, ChildrenChangedEvent, SelectionChangedEvent,
                StateChangedEvent,
            },
            window::CreateEvent,
        },
    };

    use super::*;

    fn object(path: &'static str) -> ObjectRefOwned {
        ObjectRefOwned::from_static_str_unchecked(":1.2", path)
    }

    #[test]
    fn normalizes_state_children_selection_descendant_and_window_events() {
        let state = Event::from(StateChangedEvent {
            item: object("/check"),
            state: State::Checked,
            enabled: true,
        });
        assert!(matches!(
            NormalizedEvent::from_atspi(&state),
            NormalizedEvent::NodeStateChanged { state, enabled: true, .. } if state == "checked"
        ));

        let children = Event::from(ChildrenChangedEvent {
            item: object("/list"),
            operation: Operation::Insert,
            index_in_parent: 7,
            child: object("/item"),
        });
        assert!(matches!(
            NormalizedEvent::from_atspi(&children),
            NormalizedEvent::ChildrenChanged {
                index: 7,
                child: Some(_),
                ..
            }
        ));

        assert!(matches!(
            NormalizedEvent::from_atspi(&Event::from(SelectionChangedEvent {
                item: object("/list")
            })),
            NormalizedEvent::SelectionChanged { .. }
        ));
        assert!(matches!(
            NormalizedEvent::from_atspi(&Event::from(ActiveDescendantChangedEvent {
                item: object("/virtual-list"),
                descendant: object("/visible-item")
            })),
            NormalizedEvent::ActiveDescendantChanged {
                descendant: Some(_),
                ..
            }
        ));
        assert!(matches!(
            NormalizedEvent::from_atspi(&Event::from(CreateEvent {
                item: object("/window")
            })),
            NormalizedEvent::WindowCreated { .. }
        ));
    }

    #[test]
    fn coalesces_nodes_and_subtrees_with_application_subsumption() {
        let locator = BackendLocator::new(":1.2", "/node");
        let node_event = NormalizedEvent::NodeStateChanged {
            locator: locator.clone(),
            state: "checked".to_owned(),
            enabled: true,
        };
        assert_eq!(
            coalesce_dirty_scopes(&[node_event.clone(), node_event]),
            vec![DirtyScope::Node(locator.clone())]
        );

        let subtree = NormalizedEvent::ChildrenChanged {
            parent: locator.clone(),
            change: "insert".to_owned(),
            index: 0,
            child: None,
        };
        assert_eq!(
            coalesce_dirty_scopes(&[
                NormalizedEvent::NodePropertyChanged {
                    locator: locator.clone(),
                    property: "accessible-name".to_owned(),
                },
                subtree,
            ]),
            vec![DirtyScope::Subtree(locator.clone())]
        );

        let window = NormalizedEvent::WindowCreated {
            locator: locator.clone(),
        };
        assert_eq!(
            coalesce_dirty_scopes(&[window]),
            vec![DirtyScope::Application]
        );

        let child = BackendLocator::new(":1.2", "/new-child");
        assert_eq!(
            coalesce_dirty_scopes(&[
                NormalizedEvent::ChildrenChanged {
                    parent: locator.clone(),
                    change: "insert".to_owned(),
                    index: 2,
                    child: Some(child.clone()),
                },
                NormalizedEvent::NodeStateChanged {
                    locator: child,
                    state: "showing".to_owned(),
                    enabled: true,
                },
            ]),
            vec![DirtyScope::Subtree(locator)]
        );
    }

    #[test]
    fn coalesced_structural_scope_can_be_processed_before_transient_node_echoes() {
        let parent = BackendLocator::new(":1.2", "/combo");
        let unrelated_echo = BackendLocator::new(":1.2", "/old-popup/item");
        let scopes = coalesce_dirty_scopes(&[
            NormalizedEvent::NodeStateChanged {
                locator: unrelated_echo.clone(),
                state: "selected".to_owned(),
                enabled: true,
            },
            NormalizedEvent::ChildrenChanged {
                parent: parent.clone(),
                change: "remove".to_owned(),
                index: 0,
                child: None,
            },
        ]);

        assert_eq!(
            scopes,
            vec![
                DirtyScope::Node(unrelated_echo),
                DirtyScope::Subtree(parent)
            ]
        );
        // The cache owner deliberately sorts this result so the structural
        // refresh becomes the baseline before it interprets the stale echo.
    }
}
