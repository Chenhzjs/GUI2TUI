# GUI2TUI

## Mission

GUI2TUI is not a GUI renderer. It recompiles accessibility-exposed GUI
semantics, capabilities, state, and useful spatial relationships into a
terminal-native interaction model. It transforms interaction, not screenshots.

## Core Principles

- Semantics over pixels; function over layout; tasks over widgets.
- Preserve spatial topology, not pixel coordinates.
- Geometry may influence presentation, never semantic correctness.
- The original GUI remains authoritative.
- Correct degradation is a feature. Never imitate the GUI merely for visual
  similarity or claim capability that Accessibility does not expose.

## Architecture at a Glance

AT-SPI backend → semantic cache/graph and content models → spatial evidence and
`SpatialTopology` → `RegionPresentation` and responsive layout → `TuiScene` /
`SceneBinding` → `UiIntent` / `SemanticOperation` → backend operation → GUI
state/readback. See [`docs/project-guide.md`](docs/project-guide.md) for the
current module map and detailed flow.

## Correctness Invariants

- `RuntimeNodeId` is snapshot-scoped; `BackendLocator` is the AT-SPI locator.
- `ApplicationGenerationId` and `RuntimeSession` prevent stale nodes or late
  results from targeting a restarted application.
- Semantic operations resolve only to explicitly compatible advertised actions
  or capabilities. Never guess an action or fall back to action index zero.
- The GUI/backend is authoritative. Writes require backend acceptance and
  authoritative state/readback; optimistic local state is not truth.
- Password content stays unavailable/redacted. Modal `InteractionScope`
  confines visible and invokable operations.
- Presentation never manufactures backend capability, and
  `PartialRealized` content is never presented as complete.

## Genericity Contract

Production behavior MUST be generic. Do not branch on application, process,
executable, window title, browser/editor brand, or toolkit identity. Rules may
use roles, capabilities, state, relations, semantic ancestry,
`InteractionScope`, trusted geometry/topology, payload, presentation policy,
and terminal dimensions. Validation applications are evidence, not production
inputs.

## Forbidden Approaches

- No application/toolkit adapters such as UNO, DOM/CDP, Electron private APIs,
  GTK/Qt private APIs, VS Code APIs, or Flutter hooks.
- No OCR, vision, screenshot understanding, framebuffer-to-ANSI rendering, or
  GUI screenshot recreation as the semantic source.
- No keyboard/mouse injection, coordinate clicking, XTest/uinput fallback, or
  scripted guesses such as “press Down three times then Enter”.
- No anonymous action guessing (including `action[0]`).
- No proportional GUI-coordinate-to-terminal scaling. Geometry is evidence for
  topology, not semantic truth or a pixel layout.
- Do not modify an application's backing file/database to simulate a GUI
  operation. Do not invent unsupported capabilities.

## Presentation vs Semantics

Layout, grouping, compactness, visibility, wording, symbols, and terminal
navigation may change without changing semantic truth. `SpatialTopology`,
`PresentationObligation`, `LayoutDemand`, and `VisibilityGuarantee` are
presentation policy; they must not alter roles, identity, capabilities,
operation safety, scope, content identity, or modality semantics.

## State and Operation Authority

`SceneBinding` connects a focusable scene element to a runtime node, locator,
role, advertised actions, capability, and intent. `SemanticOperation` is
resolved against the current cache/scene and then executed by the AT-SPI
backend. Reject stale identity, missing nodes, unsupported capabilities,
timeouts, and backend failures honestly.

## Degradation Policy

Read-only is better than false write success; unsupported is better than a
guessed action; `PartialRealized` is better than pretending full document
coverage; preserving modality is better than fake graphical semantics. The
message may be compact, but its truth must remain intact.

## External Modality / External Interaction Principles

Semantics remote, modality local. `ExternalModalityId`, `ReferencedResource`,
`OriginalArtifact`, and `RenderedSnapshot` preserve references or explicitly
requested materialization without making a modality provider the semantic
tree source. A future external interaction handler, if authorized, is a user
configured modality: the GUI remains authoritative, public capabilities and
readback verify writes, and secrets are never exported. This is a design
principle, not a v0.3 API.

## Scope Discipline

Do not self-expand a task, add adjacent “while here” work, introduce a new
abstraction without need, or begin the next phase without explicit
authorization. Record unrelated non-blocking issues instead of fixing them.
When acceptance criteria are satisfied, STOP.

## Testing Discipline

During normal development, prefer targeted build/check commands and directly
affected existing tests. Add only a few targeted regression tests for concrete
bugs or stable invariants. Validate presentation/heuristic changes with real
application scenes and interaction where practical. Do not create large
synthetic matrices, new test frameworks, or treat test count as progress.
Run full suites, multi-platform checks, and release qualification only at phase
close, RC/release, or explicit user request; do not repeat them after every
small change.

## Git and Release Discipline

Never rewrite history or move a published tag. `v0.1.0` and `v0.2.0` are
immutable; post-release fixes belong to later versions. Distinguish the
release source commit from later evidence/documentation HEADs. Avoid
destructive Git operations unless explicitly authorized.

## Before Changing Code

1. Identify the architectural layer and whether an existing abstraction already
   solves the request.
2. Decide whether it is semantic or presentation behavior.
3. Check that the rule is generic and does not create Accessibility-unexposed
   capability.
4. Check the forbidden-approach list and choose the smallest implementation.
5. Define only the targeted validation necessary for the requested change.

## Where to Read More

- [`docs/project-guide.md`](docs/project-guide.md) — architecture, history,
  module map, safety model, and current state.
- [`docs/architecture.md`](docs/architecture.md) — semantic/runtime pipeline.
- [`docs/spatial-layout.md`](docs/spatial-layout.md) — v0.2 topology and
  responsive presentation.
- [`docs/compatibility.md`](docs/compatibility.md) and
  [`docs/limitations.md`](docs/limitations.md) — supported workflows and
  honest boundaries.
- [`docs/release-notes-v0.2.0.md`](docs/release-notes-v0.2.0.md) — public
  release scope.
