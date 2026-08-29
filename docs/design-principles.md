# Semantic presentation principles

GUI2TUI compiles accessibility semantics into terminal interaction. It does not
scale GUI coordinates into terminal cells and it does not infer application
domains from process names or labels.

    SemanticCache
      → generic region analysis
      → presentation planning
      → TuiScene
      → Ratatui renderer

Rules consume only roles, names, descriptions, values, states, actions,
interfaces/capabilities, and parent/child relationships. AT-SPI labelled-by
relations are not yet present in the Semantic IR, and geometry remains debug
metadata rather than a rewrite input. A rewrite is accepted only when it
preserves every interactive source node and is unambiguous. Otherwise the
source remains a direct control or an explicit unsupported summary.

Current generic rewrites:

- adjacent, uniquely matching Label/Text + TextInput, or a label wrapper with
  exactly one descendant TextInput, become one labeled field;
- containers with at least two reconstructed fields become forms;
- menu bars, toolbars, and action-heavy containers become command sets;
- List and ListItem semantics become selection regions while retaining the
  already-resolved node-action or parent-Selection backend strategy;
- unnamed, non-interactive one-child layout groups are flattened;
- semantically sparse graphical roles (image, canvas, drawing area, video,
  animation, 3d view) become fidelity-preferred opaque regions.

An unknown node with an action, value, name-bearing semantic descendants, or
known controls is not considered opaque. GUI state remains authoritative:
presentation rewrites never synthesize checked, selected, or text values.

Three identities intentionally coexist:

- BackendLocator relocates an AT-SPI object and follows its object lifetime.
- RuntimeNodeId identifies a semantic node within one live cache session.
- SceneElementId identifies a presentation element for focus, layout, and
  terminal hit testing.

The renderer consumes TuiScene only. A scene binding maps user intent back to
the semantic/runtime identity and then to an explicit backend operation.
