# Relational and contextual semantic transcompiler

This document records the Phase 3D implementation and the Ubuntu 24.04 arm64/Xvfb live probes
performed on 2026-08-29. Counts are observed data, not claims about a toolkit specification.

## Pipeline

```text
SemanticCache arena
        ↓ targeted GetRelationSet
RelationState + RelationalSemanticGraph
        ↓
relation-aware regions + InteractionScopes
        ↓
CommandHierarchy
        ├── contextual hierarchy browser
        └── derived ranked search index
        ↓
TuiScene
```

`SemanticRelationTarget` retains both the ephemeral `BackendLocator` and an optional resolved
`RuntimeNodeId`. Unknown target objects never panic. Relations use `Unknown`, `Known`, and
`Unavailable` states, and node/subtree refresh invalidates them for lazy re-enrichment. Exact
locator identity and unique conservative subtree reconciliation preserve runtime targets; an
ambiguous replacement is not guessed.

Explicit `LabelledBy`, `DescribedBy`, `ErrorMessage`, and `MemberOf` edges take precedence over
structural reconstruction. The older conservative adjacency/wrapper heuristics remain the
fallback when a toolkit exposes no edge. `ControllerFor`, window, popup, embedding, flow, details,
tooltip, and unknown relations are retained and queryable even where the current presentation
does not consume them.

## Targeted relation cost

The large-tree cap is 256 candidates. The cap is now priority-driven rather than the first 256
objects in traversal order: focused node, active scope, visible scene sources, relation-sensitive
roles, current window, then background nodes. Equal-priority candidates use RuntimeNodeId only as
a deterministic tie breaker. Focus navigation can request up to eight still-unknown relation sets
on demand and recompiles only the derived scene when new edges are found.

| Application | Semantic nodes | Candidates / RPCs | Relations found | Relation latency |
| --- | ---: | ---: | ---: | ---: |
| GTK4 fixture | 58 | 31 / 31 | 5 | 3.687 ms |
| Qt6 fixture | 31 | 25 / 25 | 2 | 1.845 ms |
| LibreOffice Writer baseline | 1,981 | 256 / 256 | 0 | 10.119 ms |
| LibreOffice About active scope | 2,011 | 256 / 256 | 14 | 12.033 ms |
| Java Swing fixture | 29 | 22 / 22 | 2 | 2.447 ms |
| Electron fixture | 31 | 21 / 21 | 2 | 1.383 ms |
| Chrome large fixture | 5,158 | 256 / 256 | 2 | 8.860–9.244 ms |
| Firefox first-run fixture | 50 | 40 / 40 | 9 targets across 5 bearing nodes | 1.807 ms |

The completion run rebuilt the Chrome 5,158-node backend in 215.946 / 209.696 / 198.892 ms.
Targeted relations took 8.860–9.244 ms, region analysis 8.516–8.835 ms, and scene compilation
8.351–8.775 ms. Three TUI runs reported internal startup at 496 / 496 / 491 ms (median 496 ms),
staying below the 500 ms warm-first-frame goal while issuing exactly the configured 256 relation
RPCs rather than 5,158.

Observed types were:

- GTK4: five `LabelledBy` edges (Username and four buttons).
- Qt6: one `LabelFor` and reciprocal `LabelledBy` for Username.
- Swing: two `ControllerFor` slider-to-container edges.
- Electron: `Embeds` / `EmbeddedBy` between the window and web document.
- Chrome large: `EmbeddedBy` on the document and `MemberOf` on the status text.
- Firefox first-run UI: `NodeChildOf`, `LabelledBy`, `DescribedBy`, and reciprocal `LabelFor`.
- LibreOffice: none among the 256 targeted candidates in the 1,981-node baseline window. Opening
  About reprioritized the active dialog and produced 14 relation targets in a 2,011-node snapshot.

`ErrorMessage`, `PopupFor`, `FlowsTo`, and `FlowsFrom` were **NOT EXPOSED** by these controlled
fixtures. GTK/Qt radio controls did not expose `MemberOf`; a unique contiguous radio run preceded
by a label is therefore reconstructed conservatively as a selection group.

## Context and modality

`InteractionScopes` owns Application, Window, Dialog, ModalDialog, Popup, and MenuPopup boundaries,
maps every runtime node to its nearest scope, and selects the deepest modal/popup/focused scope.
When a modal or popup scope is active, background nodes are excluded before focus traversal,
command filtering, ranking, and mouse dispatch. Scope identity uses `RuntimeNodeId`; each boundary
also retains its `BackendLocator` for diagnostics.

GTK and Qt modal fixtures both created an active ModalDialog and removed it after the real close
button action. LibreOffice About produced:

```text
Application soffice
└── Window "Untitled 1 — LibreOffice Writer"
    └── ModalDialog "About LibreOffice" [ACTIVE]
```

Only `0815`, Close, Copy, Credits, Release Notes, and Website were returned in that scope. Closing
the real dialog returned the active scope to the Writer window.

Choice semantics are deliberately independent from GUI popup lifecycle. Qt QComboBox already
exposed Alpha/Beta/Gamma ListItems, so the terminal overlay selected Beta through the child
`Toggle` with zero `ShowMenu` calls. GTK `ComboBoxText` exposed no usable named options and is
read-only; production issued zero popup calls. Chrome exposed named options but only a hidden
parent Menu Selection interface, whose direct selection was rejected, so it is read-only. Firefox
exposed a visible Menu Selection interface and selected Beta successfully. The opening action is
never treated as a close/toggle action; TUI Escape only closes the terminal overlay.

The same catalog expresses Qt Light/Dark radio siblings and GTK/Qt Lists. Backend mechanics remain
different—child Toggle versus parent Selection—but the TUI task and focus lifecycle are shared.

## Exact focus restoration

Live Qt modal data preserved `SceneElementId 14 / RuntimeNodeId 27` for Open modal dialog, moved to
`SceneElementId 21 / RuntimeNodeId 35` for Close inside the modal, then restored exactly to 14/27.
LibreOffice preserved `SceneElementId 0 / RuntimeNodeId 5`, moved through About scope focus
24/1987 and Close 32/2011, then restored exactly to 0/5. Unit coverage separately verifies that an
invalid old SceneElement remaps through a stable RuntimeNodeId and that a vanished runtime target
falls back safely.

## Hierarchical commands

`CommandGroup`/`CommandEntry` form the canonical command tree. Search is derived by flattening that
tree, never by treating a flat vector plus path strings as canonical storage. Duplicate labels
retain source runtime identity. The filter order is active scope first, then generic ranking:
current scope +50, enabled +10, visible +5, exact query +20, and recent in-session use +3.
`--dump-commands` prints each reason.
Palette search defaults to the current interaction scope; `F2` explicitly expands or narrows
the derived index to all application commands.

LibreOffice exposed 478 safe semantic leaves. The reachability audit reported 478 reachable, one
structural reveal omitted, 756 unsafe/unresolved operations, and zero unreachable safe leaves.
The normal scene now renders command summaries rather than 478 command rows. `:` displayed top
groups, entering Container displayed its real children, and searching `about` produced
`soffice › Container › MenuBar › Help › About LibreOffice`; the real action opened the modal About
dialog.

## Collection completeness and modality preservation

Collections carrying `manages-descendants` are `PartialRealized`, never advertised as complete.
`ActiveDescendantChanged` remains normalized, but no new live occurrence was observed in this
Phase 3D run. Opaque/fidelity-preferred regions remain semantic placeholders: **NO RASTER** and
**OpaqueSurfaceProvider NOT IMPLEMENTED**.
