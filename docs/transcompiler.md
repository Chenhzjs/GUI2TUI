# Semantic transcompiler validation

Validated on 2026-08-29 in Ubuntu 24.04 arm64, Xvfb/X11, AT-SPI 2.52. The
same production analyzer was used for every row; it has no toolkit or
application-name branch.

## Pipeline

    SemanticCache
      → materialized SemanticNode view
      → generic SemanticRegion analysis
      → PresentationStrategy
      → TuiScene
      → Ratatui renderer

`BackendLocator` addresses the live AT-SPI object, `RuntimeNodeId` preserves
semantic identity within the cache session, and `SceneElementId` identifies a
terminal presentation element. Scene bindings provide the reverse path from a
command, field, or hit region to the existing semantic operation resolver.

## Live coverage

| Application | Semantic | Interactive | Regions | Direct | Reconstructed | Compressed | Commands | Selection | Opaque | Unsupported | Scene | Action result |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| GTK fixture | 26 | 8 | 18 | 6 | 1 | 0 | 0 | 1 | 0 | 0 | 13 | edit, button, list PASS |
| gtk4-demo | 92 | 22 | 57 | 6 | 0 | 4 | 1 | 1 | 0 | 7 | tree only |
| Qt fixture | 22 | 11 | 20 | 5 | 3 | 0 | 1 | 1 | 0 | 0 | 15 | palette, edit, list PASS |
| Qt Assistant | 170 | 77 | 127 | 28 | 3 | 3 | 5 | 1 | 0 | 24 | 108 | tree only |
| LibreOffice Writer | 1,954 | 447 | 478 | 2 | 0 | 28 | 13 | 0 | 0 | 8 | 449 | About dialog PASS |
| Swing fixture | 29 | 8 | 26 | 6 | 3 | 2 | 1 | 1 | 0 | 3 | 18 | palette command PASS |
| Electron fixture | 31 | 0 | 16 | 2 | 3 | 4 | 1 | 0 | 0 | 0 | 8 | no named safe action |
| Chrome fixture | 286 | 0 | 142 | 26 | 4 | 19 | 2 | 0 | 0 | 12 | 51 | regression/read-only |
| Chrome large (700 rows) | 5,158 | 0 | 3,621 | 1,417 | 0 | 20 | 2 | 700 | 0 | 711 | 3,535 | scale probe |
| Firefox fixture | 236 | 2 | 140 | 27 | 4 | 37 | 4 | 2 | 0 | 45 | 90 | regression/read-back guard |
| GTK opaque fixture | 9 | 1 | 8 | 1 | 0 | 0 | 0 | 0 | 1 | 0 | 4 | surrounding button PASS |

Interactive nodes count safe semantic operations in the source tree. Scene
interactive count additionally includes focusable read-only fields, so the two
columns intentionally need not match. Anonymous Chromium/Electron actions do
not become commands or semantic activation.

## Implemented generic rules

- Adjacent uniquely compatible Label/Text + TextInput becomes a labeled field.
- A label wrapper with exactly one descendant TextInput becomes a labeled
  field; ambiguity leaves nodes separate.
- A group with multiple fields/controls becomes a form.
- Menu bars, toolbars, and action-heavy containers become command sets. Only
  operations accepted by the role-aware resolver enter the palette.
- Lists retain the existing node-action or parent-Selection operation behind a
  common selection presentation.
- Unnamed one-child layout groups are flattened and consecutive content rows
  are combined into summaries.
- Sparse drawing-area/canvas/image/video/animation/3d-view roles with no value,
  action, or semantic descendants become `OpaqueContent` with
  `FidelityPreferred` modality.

AT-SPI labelled-by relations are **not implemented** in the current Semantic
IR. Geometry is used only to report dimensions for an already-classified
opaque region; it never maps GUI pixels to terminal coordinates.

## Timing samples

| Application | Bootstrap | Region analysis | Scene compile | First frame marker |
| --- | ---: | ---: | ---: | ---: |
| GTK fixture | 7.1 ms cache | 0.036 ms | 0.048 ms | 79.7 ms |
| Qt fixture | 40.6 ms walk | 0.037 ms | 0.068 ms | 113.6 ms |
| Qt Assistant | 193.2 ms walk | 0.107 ms | 0.291 ms | 261.6 ms |
| LibreOffice Writer | 2,464.3 ms walk | 1.333 ms | 3.345 ms | 2,653.7 ms |
| Swing fixture | 52.8 ms walk | 0.040 ms | 0.090 ms | 189.5 ms |
| Electron fixture | 6.1 ms cache | 0.034 ms | 0.055 ms | 102.3 ms |
| Chrome fixture | 29.1 ms cache | 0.203 ms | 0.412 ms | 103.2 ms |
| Chrome large (5,158 nodes) | 199.7 ms cache | 3.126 ms | 8.110 ms | 387.7 ms |
| Firefox fixture | 357.0 ms walk | 0.140 ms | 0.392 ms | 392.1 ms |
| Opaque fixture | 1.6 ms cache | 0.012 ms | 0.020 ms | 107.5 ms |

These are development observations, not a benchmark. Bootstrap and first-frame
samples were single runs in the shared live session. The analyzer/compiler
times were emitted by `--dump-scene`; the first-frame marker was measured in a
real PTY at 120×40.

## Known limits

- LibreOffice exposes hundreds of valid menu actions; flattening makes them
  searchable but the palette still needs grouping/ranking.
- Firefox exposes many menu objects that remain unsupported summaries.
- Relation ingestion, ComboBox popup reconstruction, virtualized collection
  semantics, and an opaque surface provider are not implemented.
- No raster, framebuffer, Chafa, Kitty, Sixel, compositor, UNO, DevTools,
  Electron, or Java application API is used.
