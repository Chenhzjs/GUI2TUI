# GUI2TUI Engineering Guide

`AGENTS.md` contains normative AI development rules. This guide explains the
architecture, history, and boundaries for new AI sessions and contributors.
When the two appear inconsistent, `AGENTS.md` governs agent behavior; report
the inconsistency and correct the documentation rather than silently inventing
a resolution.

## 1. What GUI2TUI Is

GUI2TUI reads public Linux Accessibility / AT-SPI semantics and recompiles them
into terminal-native workflows. A button remains an explicit action, a choice
remains a safe selection task, and document-like content becomes a bounded
Reader with navigation and search. Useful exposed geometry supplies spatial
evidence for arranging those tasks.

## 2. What GUI2TUI Is Not

It is not a framebuffer-to-ANSI converter, screenshot renderer, remote desktop,
pixel-perfect layout emulator, OCR system, or application automation layer.
It does not promise every GUI control, Electron surface, or long document is
fully available. Missing or unreliable semantics degrade safely.

## 3. Design Philosophy

Semantics over pixels, function over layout, and tasks over widgets. Preserve
spatial topology rather than pixel coordinates. Utility is preferred over
literal GUI structure. Geometry influences presentation only; it never decides
semantic role, capability, identity, scope, content identity, or operation
safety. The original GUI is the authority for state and effects.

## 4. Evolution: v0.1 → v0.2

v0.1 established the semantic foundation: AT-SPI inspection, semantic IR,
explicit capabilities and safe actions, authoritative EditableText readback,
relations and interaction scopes, commands and choices, Reader/Outline/Search,
progressive content, external modality, runtime recovery, and hardened
installation/release workflows.

v0.2 added three presentation steps:

- **0.2A Spatial Reconstruction:** bounded trusted geometry becomes generic
  `SpatialEvidence` and `SpatialTopology` relationships.
- **0.2B Responsive Region Composition and Semantic Surface Preservation:**
  `RegionPresentation`, `PresentationObligation`, `LayoutDemand`, and
  `VisibilityGuarantee` keep important surfaces reachable while regions split,
  stack, collapse, or move into navigation.
- **0.2C Terminal UX Refinement:** `RegionNavigator` provides a bounded,
  terminal-generated hierarchy with major-region and sibling-subregion
  navigation, compact contextual surfaces, and contextual help.

The v0.2 spatial/responsive layout is the default; `--layout flat` remains a
compatibility fallback. Original GUI `TabList` semantics are distinct from the
terminal Region Navigator.

## 5. Current Architecture

The stable pipeline is:

```text
AT-SPI backend
  → SemanticCache / relational semantic graph
  → SemanticRegion and SemanticContentModel
  → SpatialEvidenceIndex / SpatialTopology
  → RegionPresentation / responsive TuiLayoutPlan
  → TuiScene and SceneBinding
  → UiIntent / SemanticOperation
  → BackendOperation / AT-SPI
  → bounded transition observation where needed
  → authoritative GUI state/readback; events only wake observation
```

The semantic and runtime contracts are shared by flat and spatial
presentations. Presentation can be recomputed for terminal size without
re-querying geometry or changing the semantic snapshot.

## 6. End-to-End Data Flow

An AT-SPI object is cached as a semantic node with a `RuntimeNodeId`,
`BackendLocator`, role, state, relations, actions, and capabilities. Region and
scene compilation creates a focusable `SceneElement` whose `SceneBinding`
retains that runtime identity and a safe default `UiIntent`. Input resolves the
intent to a `SemanticOperation`; the operation resolver checks the current
scene/cache and advertised action or selection capability, producing a
`BackendOperation`. The AT-SPI backend performs it. Dynamic actions with an
explicit supported postcondition capture exact session, generation, locator,
and scope authority, then observe that condition within a deadline. Events
only wake/coalesce a new backend read; an event, invocation return, or elapsed
time never establishes semantic success. The ordinary cache, scopes, scene,
and focus are rebuilt from the authoritative read, and local optimistic state
is never the authority.

## 7. Repository / Module Map

- `src/backend/`: public AT-SPI transport, bootstrap/cache acquisition,
  protocol compatibility, and explicitly requested static visual capture. Do
  not add app/toolkit adapters here.
- `src/semantic/`: `SemanticCache`, nodes, roles, relations, capabilities,
  locators, and semantic graph construction. Do not infer capabilities from
  pixels or names of known applications.
- `src/transcompile/`: region analysis, scopes, choices, commands, content
  compression, scene compilation, and spatial/presentation planning. Keep this
  toolkit-independent and presentation-only where spatial logic is involved.
- `src/content/`: `SemanticContentModel`, `ContentBlockId`, bounded caches,
  progressive materialization, Reader/search, tables, and virtual collections.
  Do not treat Reader content as the ordinary control scene.
- `src/runtime/`: `RuntimeSession`, `RuntimeSessionId`,
  `ApplicationGenerationId`, operation tickets, lifecycle and cancellation.
  Do not bypass generation checks.
- `src/modality/`: references, artifacts, static snapshots, broker/transport,
  and handler boundaries. Do not make modality a semantic backend.
- `src/tui/`: scene rendering, focus/input, operation dispatch, overlays,
  help, content view, palette, and `RegionNavigator`. Do not introduce a
  second semantic or focus authority.
- `src/product/`: CLI configuration, launcher, doctor, paths, and headless
  session management.
- `tests/`, `scripts/`, and `docs/`: regression/workflow validation,
  reproducible release/demo tooling, and public/engineering documentation.

## 8. Identity and Runtime Model

`RuntimeNodeId` is a compact identifier regenerated for a semantic snapshot;
it is not an AT-SPI locator. `BackendLocator` identifies the AT-SPI bus/object
pair. `ApplicationGenerationId` changes whenever `RuntimeSession` opens a new
application cache. Operation tickets carry session and generation ownership;
stale, late, or cancelled results must be rejected rather than applied to a
new process or snapshot. Cache reconciliation may retain a `RuntimeNodeId`
across an exact, unique structural replacement to reduce presentation churn,
but that continuity is never operation authority: authority captured for
locator L1 cannot act on or confirm locator L2. A newly current binding may
capture fresh authority for L2 normally.

## 9. Semantic Operations

`UiIntent` describes user intent (`Activate`, `Toggle`, `Select`, editing,
navigation, reading, and so on). `SemanticOperation` is the explicit operation
over a runtime node. Action resolution is role-aware and matches advertised
AT-SPI action names; an empty or incompatible action list is unsupported.
Choices use explicit node-action or parent-selection strategies. Anonymous
actions, guessed indexes, and keyboard sequences are not semantic operations.
For qualified dynamic actions, the short-lived transition observer is adjacent
to the operation rather than a workflow engine: it checks a small internal
semantic condition, returns to the normal `TuiScene`, and leaves the next
operation to the user.

## 10. Content Architecture

`SemanticContentModel` stores `ContentBlockId` blocks, metadata, reading order,
navigation indexes, and `ContentCompleteness`. `ContentRuntime` materializes
visible blocks and bounded lookahead into a cache; Reader, Outline, Search,
tables, and virtual collections use that model. `PartialRealized` means only a
verified portion is available and must be shown honestly. Reader is a separate
full-content task, not a promise that the main application scene contains every
paragraph or cell.

## 11. Spatial / Presentation Architecture

`SpatialEvidenceIndex` accepts bounded, trusted AT-SPI bounds and records
provenance and coordinate space. `SpatialTopology` normalizes comparable
rectangles into relationships such as containment, adjacency, alignment,
bands, peripheral placement, and dominance. It never maps GUI dimensions to
terminal cells.

`RegionPresentation` classifies useful surfaces. `PresentationObligation`
(`Persistent`, `Contextual`, `Discoverable`, `Structural`, `DiagnosticOnly`)
describes whether a surface must remain represented. `LayoutDemand` describes
space needed for useful payload; `VisibilityGuarantee` describes how directly
it should remain visible. `TuiLayoutPlan` and responsive composition use these
policies, preserve source bindings, and audit reachability. Empty and
structural-only surfaces can collapse without deleting semantic operations.

## 12. Region Navigation / Terminal UX

`RegionNavigator` derives a presentation-only, maximum two-level hierarchy from
the existing `TuiLayoutPlan`, meaningful layout branches, and region labels.
It does not create application categories or alter semantic GUI tabs. F6 and
Shift-F6 cycle major groups; Ctrl-Tab and Ctrl-Shift-Tab cycle sibling
subregions; Tab and Shift-Tab remain control focus navigation. Modal and popup
scopes filter the scene so blocked background regions are not navigable. A
single meaningful region produces no unnecessary navigator, and missing
hierarchy falls back to one level.

## 13. External Modality

Some content is not safely reproducible as terminal text. `ExternalModalityId`
tracks a modality owner; resolution may yield a `ReferencedResource`, an
`OriginalArtifact`, a `RenderedSnapshot`, live visual state, or unavailable
status. An original resource is distinct from a one-frame rendered snapshot.
Reference-first handling transfers no payload; explicitly requested local
materialization or a same-host broker may handle a permitted artifact. The
provider never becomes the semantic tree source, and controls around opaque
content remain AT-SPI controls.

## 14. Error / Degradation Model

Runtime errors include stale identity, target gone, backend unavailable,
timeout, unsupported/capability unavailable, cancellation, endpoint loss,
integrity/protocol failure, and internal failure. Content can be partial or
unavailable; modality can be unresolved; text capability probes can be
quarantined after failure. The response is to preserve safety and explain the
limitation—not to guess, retry indefinitely, or claim success.

## 15. Genericity and Framework Independence

The production contract is independent of application/process names, window
titles, browser/editor brands, and GTK/Qt/Electron identities. Generic rules
use roles, capabilities, state, relations, scopes, trusted topology, payload,
and terminal dimensions. Mousepad, Chromium, Firefox, EOG, GTK Demo, Qt
Designer, Writer, and Electron are validation corpus examples only.

## Why Forbidden Approaches Are Forbidden

Application-specific adapters destroy the generic accessibility boundary and
make behavior depend on one vendor's private surface. DOM/CDP, UNO, Electron
private APIs, and toolkit internals replace the public semantic contract with a
special-case integration. OCR and vision make semantic truth probabilistic.
Keyboard or mouse injection bypasses the operation that the GUI actually
advertises and cannot safely verify its target. Pixel-coordinate scaling
recreates the wrong abstraction: topology is useful, coordinates are not
portable terminal layout. Action guessing can invoke the wrong command.
Direct backing-file edits bypass the GUI's authority, events, permissions, and
conflict handling. These shortcuts can make a screenshot look successful while
violating correctness and trust.

## 16. Testing Philosophy

Semantic/runtime contracts benefit from targeted automated regression tests:
identity, password exclusion, action resolution, modal scope, authoritative
readback, and content completeness. Spatial and presentation heuristics are
better judged with real accessibility scenes, resize behavior, and actual
keyboard interaction than with large synthetic matrices. Full suites and
multi-platform qualification belong at phase/release boundaries. Test count
does not define product scope or quality.

## 17. Release Model

The supported release pipeline builds native Linux x86_64 and aarch64 packages,
runs extracted-package smoke, checks ABI, checksums, manifest, and
attestations, then optionally publishes. Published tags are immutable. The
release source commit is distinct from later evidence/documentation commits;
never assume newest HEAD is the binary source. `v0.1.0`, `v0.2.0`, and
`v0.3.0` must not be moved.

## 18. Known Boundaries / Non-goals

Accessibility quality varies by application. Long documents may remain partial;
multiline/rich-text editing and password content are intentionally limited.
Electron/Monaco, Wayland capture, remote companion transport, new-TTY attach,
and live video/game/3D surfaces are not complete product capabilities. There
is no DOM/CDP, UNO, OCR/vision, private toolkit API, GUI injection, or pixel
layout reconstruction.

## 19. Current Project State

- Public release: **v0.3.0**
- Project state: **v0.4 DEVELOPMENT**
- v0.1.0, v0.2.0 and v0.3.0 source tags: immutable and already published
- v0.3 functional development: **COMPLETE**
- v0.3.0 release-candidate qualification: **QUALIFIED**
- v0.3.0 public release: **COMPLETE** from frozen source
  `efc704adf8a3ded3463ed8bb81670eddd08296c3`
- v0.4 Semantic Workflow Reconstruction Discovery: **COMPLETE**; conclusion
  **B — NARROWER CONTINUATION MODEL SUFFICIENT**
- v0.4A Exact Authority and Bounded Transition Observation: **VALIDATED**
- Recommended next work: **0.4B Dynamic Surface and Scope Continuation**,
  awaiting explicit user authorization

Release and validation details live in the [v0.3.0 release notes](release-notes-v0.3.0.md),
[production release verification](validation/v0.3/release/HANDOFF.md), and
[v0.3 roadmap](planning/v0.3-roadmap.md). The architecture-level trajectory
from the completed v0.3 release to 1.0 is recorded separately in the
[roadmap to 1.0](planning/roadmap-to-1.0.md); listing a future milestone there
does not authorize it.

The evidence and bounded phase plan for v0.4 are recorded in the
[v0.4 Discovery](planning/v0.4-workflow-reconstruction.md) and
[v0.4 roadmap](planning/v0.4-roadmap.md). The completed 0.4A evidence is in the
[transition-observation handoff](validation/v0.4/transition-observation/HANDOFF.md).
Completion of 0.4A does not authorize 0.4B or later implementation.

## 20. v0.3 Capability Recovery

The completed functional line adds verified native Value interaction and a
configured complex plain-text interaction modality while preserving public
Accessibility mutation, stale/conflict checks, and authoritative read-back.
Compound interaction evidence did not justify speculative orchestration.
Consult the [v0.3 roadmap](planning/v0.3-roadmap.md) and latest validation
handoff before any further work. v0.3.0 is released and immutable. Future
source fixes require v0.3.1 or later; do not begin v0.4 without explicit
authorization.

## 21. Glossary

- **AT-SPI:** Linux Accessibility service used as the public semantic source.
- **BackendLocator:** validated AT-SPI bus name/object path locator.
- **RuntimeNodeId:** snapshot-local semantic identity.
- **ApplicationGenerationId:** generation boundary for an application cache.
- **InteractionScope:** application/window/dialog/popup boundary that confines
  operations and navigation.
- **SemanticOperation:** explicit safe operation resolved from a `UiIntent`.
- **Transition observation:** short-lived, operation-adjacent bounded checking
  of an explicit postcondition through fresh authoritative semantics.
- **SemanticContentModel / ContentBlockId:** bounded document/content model and
  its block identity.
- **SpatialEvidence / SpatialTopology:** trusted geometry evidence and the
  generic relationships inferred from it.
- **RegionPresentation / TuiLayoutPlan:** presentation policy and terminal-
  independent responsive arrangement.
- **RegionNavigator:** terminal-generated two-level region navigation, distinct
  from GUI semantic tabs.
- **OriginalArtifact / RenderedSnapshot:** original referenced resource versus
  one explicitly acquired static visual artifact.
