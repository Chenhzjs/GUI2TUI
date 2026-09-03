# v0.2 spatial reconstruction and responsive composition

GUI2TUI v0.2A adds a presentation-only reconstruction path while preserving
the v0.1 semantic/runtime contracts:

```text
SemanticRegion + bounded SpatialEvidenceIndex
    -> SpatialTopology + SpatialRegion
    -> RegionPresentation
    -> PresentationObligation + LayoutDemand + VisibilityGuarantee
    -> semantic surface selection/coalescing
    -> responsive composition
    -> terminal-independent TuiLayoutPlan
    -> responsive renderer (the default; `gui2tui --layout spatial` is explicit)
```

The v0.2 spatial renderer is the default user experience. The compatibility
linear renderer remains available with `--layout flat` for workflows that need
the pre-v0.2 presentation. Both modes share the same semantic/runtime
contracts; spatial composition is presentation-only.

## Evidence and trust

`SpatialEvidence` is keyed by `RuntimeNodeId` and
`ApplicationGenerationId`. It records `SpatialBounds`, `CoordinateSpace`,
visible/showing state, optional layer, provenance and `GeometryTrust`.
Coordinate spaces are `Screen`, `Window`, `Parent` and `Unknown`; rectangles
are compared only in the same known space. Invalid, zero-sized, sentinel or
otherwise inconsistent extents are rejected.

`SpatialEvidenceIndex::from_backend` selects at most 128 likely layout anchors
by default and performs eight concurrent bounded public AT-SPI
`GetExtents(Screen)` probes. Cached snapshot geometry is reused. Metrics expose
node/candidate/request/success/failure/rejection counts and duration. Terminal
resize reuses the generation-scoped plan and does not re-query GUI geometry.

`SpatialTopology` normalizes comparable rectangles to fixed-point inference
features and records only useful relationships such as containment, relative
position, adjacency, alignment, bands, peripheral placement and area
dominance. Geometry is evidence for those relationships. It never defines
roles, capabilities, action safety, identity, scope, content identity or
modality. Normalized GUI dimensions are never proportionally mapped to
terminal cells.

## Presentation and composition

`RegionPresentationKind` currently supports `InlineContent`, `InputSurface`,
`Navigation`, `Form`, `ChoiceList`, `Table`, `CommandBar`,
`GraphicalPlaceholder`, `Status`, `ControlGroup`, `WorkspacePane`,
`CollapsedSummary`, `Structural`, `DiagnosticOnly` and `Empty`.

A semantic Content region is only dominant when it has useful presentation
payload. Meaningful text/content blocks become `InlineContent`; mixed documents
remain inline even when they contain images. Pure fidelity-required content can
become a large honest `GraphicalPlaceholder`. Empty/Reader-only content is not
dominant. `PrimaryContent = None` is valid.

Generic `CompositionKind` values are `ContentDominant`, `NavigationDetail`,
`MultiPaneWorkspace`, `DialogForm`, `ControlSurface` and `FallbackStack`.
Compatible siblings/descendants may be coalesced, but all source region/node
bindings are retained and `audit_layout_reachability` must report no lost
actionable regions. Only semantic dialogs/overlays may become terminal overlays;
rectangle intersection alone cannot invent modality.

Structural-only objects remain in diagnostics but receive no standalone row.
Unsupported presentation is separated into meaningful grouped presentation,
structural-only, and diagnostic-only policy rather than printed uniformly.

## Surface policy and responsive realization

`PresentationObligation::{Persistent, Contextual, Discoverable, Structural,
DiagnosticOnly}` decides whether a semantic surface must remain represented.
`LayoutDemand::{Expand, Supporting, Compact, Minimal, Hidden}` independently
describes how much terminal space makes its payload useful.
`VisibilityGuarantee::{Pinned, PreferDirect, Collapsible, DiscoverableOnly}`
independently controls how strongly it must remain directly visible. Pinned
compact surfaces collapse only when their generic minimum viable size cannot
fit; a rich content surface expands and an empty pane remains minimal.

Generic policy preserves top-level single-line inputs outside the dominant
content subtree, current tab context, coherent compact control bands, useful
status and the primary task. Input purpose is inferred only from accessible
role/name/description evidence; uncertainty is rendered as `Input` rather than
inventing a purpose. Commands remain discoverable through the existing palette.

`realize_responsive_layout` uses minimum viable terminal pane widths, height,
surface count, visibility guarantee, obligation, demand, payload richness and
active region. Wide layouts may retain multiple columns; medium and narrow
layouts reserve viable Pinned compact rows before adding PreferDirect peers and
the collapsed selector. Resize reuses the same semantic/spatial plan, source
bindings and GUI topology; it performs no geometry query.

`PresentationCoverageAudit` verifies primary task, persistent inputs, compact
controls, current tab context, meaningful status and command discoverability.
Semantic coverage reports direct-or-collapsed representation; direct coverage
separately reports Pinned surfaces that are direct, legitimately forced to
collapse, or improperly collapsed. `audit_layout_reachability` remains the
independent operation-binding check.

F6 and Shift-F6 cycle semantic regions inside the current scope. Tab and
Shift-Tab move among controls in the active region. A modal scope's filtered
scene excludes blocked background sources from the region order.

## Renderer

The renderer consumes `TuiLayoutPlan` and realizes bordered
panes, stacks, horizontal/vertical splits, compact support bars, overlays and a
scrollable content viewport. `LayoutImportance::{Dominant, Supporting,
Compact, Structural}` controls terminal-native allocation independently of GUI
pixel ratios.

Inline content reuses `ContentRuntime`, `SemanticContentModel` and bounded
materialization. The normal scene shows only the rows fitting the actual pane
and keeps Reader/Outline/Search as a separate full-content task. Graphical
content is never fabricated; fidelity-required regions expose a placeholder and
existing modality operations.

## Diagnostics

```text
gui2tui-inspect --app NAME --dump-spatial-evidence
gui2tui-inspect --app NAME --dump-spatial-regions
gui2tui-inspect --app NAME --dump-layout-plan
gui2tui-inspect --app NAME --dump-presentation-coverage
```

The plan dump includes inference/coalescing/suppression reasons, source region
and runtime-node bindings, phase timings and layout reachability. Real 0.2A
evidence is under `docs/validation/v0.2/spatial/`; responsive 0.2B evidence is
under `docs/validation/v0.2/responsive/`.

No production spatial path refers to application, executable, window-title or
toolkit identity. The reference applications are validation data only.
