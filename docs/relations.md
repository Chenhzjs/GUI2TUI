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

The large-tree cap is 256 candidates. Smaller caches enrich relation-sensitive roles plus scene
sources; they are not subjected to a full-tree relation scan.

| Application | Semantic nodes | Candidates / RPCs | Relations found | Relation latency |
| --- | ---: | ---: | ---: | ---: |
| GTK4 fixture | 58 | 31 / 31 | 5 | 3.687 ms |
| Qt6 fixture | 31 | 25 / 25 | 2 | 1.845 ms |
| LibreOffice Writer | 1,954 | 256 / 256 | 0 | 8.068 ms |
| Java Swing fixture | 29 | 22 / 22 | 2 | 2.447 ms |
| Electron fixture | 31 | 21 / 21 | 2 | 1.383 ms |
| Chrome large fixture | 5,158 | 256 / 256 | 2 | 7.987 ms |
| Firefox first-run fixture | 50 | 40 / 40 | 9 targets across 5 bearing nodes | 1.807 ms |

The final Chrome 5,158-node warm scene runs completed in 0.456 / 0.408 / 0.409 seconds wall time
(median 0.409 s). Their region analysis was 8.809–9.238 ms and scene compilation 8.678–8.930 ms;
targeted relations remained 7.374–9.900 ms. This stays below the Phase 3D 500 ms warm-first-frame
goal without issuing 5,158 relation RPCs.

Observed types were:

- GTK4: five `LabelledBy` edges (Username and four buttons).
- Qt6: one `LabelFor` and reciprocal `LabelledBy` for Username.
- Swing: two `ControllerFor` slider-to-container edges.
- Electron: `Embeds` / `EmbeddedBy` between the window and web document.
- Chrome large: `EmbeddedBy` on the document and `MemberOf` on the status text.
- Firefox first-run UI: `NodeChildOf`, `LabelledBy`, `DescribedBy`, and reciprocal `LabelFor`.
- LibreOffice: none among the 256 targeted candidates in the baseline window. Opening About
  produced 14 relation targets in a later 1,984-node snapshot.

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

Qt QComboBox exposed `ShowMenu`/`Press`; opening created a realized List popup with Alpha/Beta/Gamma.
Selecting Beta invoked the item `Toggle`, refreshed two nodes, performed zero full snapshots, and
the independent inspector reported Beta selected. No advertised action had a verified close
meaning, so GUI2TUI intentionally left the popup open instead of guessing that an opening action
was reversible.

GTK `ComboBoxText` exposed a unique descendant ToggleButton `Click`, so safe open and Popup scope
creation worked. The realized popup subtree contained only unnamed panels and no semantic options
or safe option actions. Selection and close are therefore **BLOCKED by current AT-SPI exposure**;
GUI2TUI does not guess a child index.

## Hierarchical commands

`CommandGroup`/`CommandEntry` form the canonical command tree. Search is derived by flattening that
tree, never by treating a flat vector plus path strings as canonical storage. Duplicate labels
retain source runtime identity. The filter order is active scope first, then generic ranking:
current scope +50, enabled +10, visible +5, exact query +20, and recent in-session use +3.
`--dump-commands` prints each reason.
Palette search defaults to the current interaction scope; `F2` explicitly expands or narrows
the derived index to all application commands.

LibreOffice exposed 478 safe semantic leaves. The reachability audit reported 478 reachable, one
structural reveal omitted, 749 unsafe/unresolved operations, and zero unreachable safe leaves.
The normal scene now renders command summaries rather than 478 command rows. `:` displayed top
groups, entering Container displayed its real children, and searching `about` produced
`soffice › Container › MenuBar › Help › About LibreOffice`; the real action opened the modal About
dialog.

## Collection completeness and modality preservation

Collections carrying `manages-descendants` are `PartialRealized`, never advertised as complete.
`ActiveDescendantChanged` remains normalized, but no new live occurrence was observed in this
Phase 3D run. Opaque/fidelity-preferred regions remain semantic placeholders: **NO RASTER** and
**OpaqueSurfaceProvider NOT IMPLEMENTED**.
