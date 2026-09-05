# GUI2TUI Roadmap to 1.0

This document records the reviewed architecture-level trajectory from the
completed v0.3 release to GUI2TUI 1.0. It is direction, not authorization. A
milestone starts only after explicit user approval, and uncertain architecture
must be qualified through evidence before implementation.

`AGENTS.md` remains the normative development constitution. The
[engineering guide](../project-guide.md) describes the current architecture;
version-specific planning and validation records preserve the detailed
evidence behind completed milestones. This roadmap identifies the product
layers that still need to close. It does not replace those documents or begin
v0.4 work.

## 1. Purpose and status

GUI2TUI must not approach 1.0 as a sequence of isolated widget additions,
application compatibility patches, or AT-SPI interface wrappers. Each
pre-1.0 milestone must answer one missing architecture or product question.
Features are useful only when they supply evidence that the layer works for
generic semantic tasks.

| Version | Architectural layer | Status |
| --- | --- | --- |
| v0.1 | Semantic Reconstruction | **COMPLETED** |
| v0.2 | Spatial Reconstruction | **COMPLETED** |
| v0.3 | Verified Capability Recovery | **COMPLETED / PUBLICLY RELEASED** |
| v0.4 | Semantic Workflow Reconstruction | **DISCOVERY COMPLETED / 0.4A RECOMMENDED / AWAITING USER AUTHORIZATION** |
| v0.5 | Task & Interaction Completeness | **PLANNED / NOT AUTHORIZED** |
| v0.6 | Runtime Continuity & Multi-Surface Robustness | **PLANNED / NOT AUTHORIZED** |
| v0.7 | Deployment & Environment Completeness | **PLANNED / NOT AUTHORIZED** |
| 1.0 | Stabilization, Product Contract & Release Qualification | **PLANNED / NOT AUTHORIZED** |

The default route after v0.7 is a functional feature freeze followed by an
evidence-based 1.0 readiness decision. v0.8 and v0.9 are not assumed. If later
evidence reveals another missing architectural layer, adding a pre-1.0
milestone requires documented justification and explicit user review.

## 2. Completed architectural arc

### v0.1 — Semantic Reconstruction

**Question:** What does this GUI mean?

v0.1 established Accessibility semantics as the product source: AT-SPI
acquisition, semantic identity and roles, safe primitive operations, content
models, runtime ownership, modal scope, and external-modality boundaries. It
made the GUI understandable without treating screenshots, pixels, or private
application APIs as semantic truth.

### v0.2 — Spatial Reconstruction

**Question:** How should those semantics be organized into a usable terminal
interface?

v0.2 combined semantic structure with bounded spatial evidence. The resulting
`SpatialTopology`, `RegionPresentation`, presentation obligations, responsive
`TuiLayoutPlan`, and `RegionNavigator` preserve useful relationships and
reachability without scaling GUI coordinates into terminal cells. Spatial and
flat presentations share the same semantic and operation authority.

### v0.3 — Verified Capability Recovery

**Question:** What can the user safely do?

v0.3 established a stricter capability chain:

```text
public capability
  → generic eligibility
  → bounded invocation
  → current identity and scope checks
  → independent authoritative read-back
  → confirmed user capability
```

This produced verified native single-line text behavior, native bounded Value
interaction, and configurable interaction for complete non-secret complex
plain text. External text handlers edit a private GUI2TUI-owned candidate;
GUI2TUI retains conflict detection, public AT-SPI write-back, and authoritative
verification. Partial, rich, secret, stale, ambiguous, rejected, or unsafe
surfaces continue to degrade honestly. Interface exposure or a successful
method return alone does not establish a trustworthy capability.

The completed and planned layers form one progression:

```text
semantic reconstruction
  → terminal-native spatial organization
  → verified primitive capability
  → transition-aware semantic workflow
  → task completeness
  → runtime continuity
  → deployment contract
  → 1.0 stabilization and qualification
```

## 3. North star for 1.0

GUI2TUI 1.0 should be a generic Accessibility-driven semantic runtime for
ordinary Linux GUI applications. Given reasonably complete public
Accessibility semantics and no GUI2TUI-specific application integration, it
should reconstruct useful semantic structure, present it as a terminal-native
interface, expose only verified interactions, follow dynamic semantic state
transitions, complete common multi-step user tasks, and survive normal
long-running application and session lifecycle changes.

It must operate reliably in explicitly documented deployment environments,
preserve graphical or external modality when terminal semantics are
insufficient, and degrade safely whenever semantic fidelity, completeness, or
capability is unavailable.

This definition excludes application-specific production adapters, private
application or toolkit APIs, visual guessing as semantic truth, input
injection, backing-file bypass, and pixel-layout imitation. The original GUI
application remains authoritative throughout acquisition, interaction,
transition, recovery, and verification.

### Progress is measured by generic tasks

1.0 is not defined by the number of AT-SPI roles recognized, applications
tested, tests added, or widgets made interactive. Those counts can increase
while leaving the product unable to complete a common task safely.

The important measure is whether a generic semantic task can be completed
correctly: the target is identified, every operation is explicitly supported,
dynamic state is observed rather than guessed, scope and generation remain
valid, effects are authoritatively verified, and failures degrade safely. Real
applications carry evidence for these contracts; they are not production
inputs and are not the product architecture.

## 4. v0.4 — Semantic Workflow Reconstruction

**Qualified question:** How can one verified semantic operation be observed
through an authoritative semantic transition so the user can continue safely?

The completed [v0.4 Discovery](v0.4-workflow-reconstruction.md) selected
**B — NARROWER CONTINUATION MODEL SUFFICIENT**. Existing refresh, scene rebuild,
modal scope, focus history, and user navigation already solve much of dynamic
continuation. The evidence supports a bounded operation-adjacent semantic
observation contract, not a workflow engine or automatic task runner. The
derived [v0.4 roadmap](v0.4-roadmap.md) recommends 0.4A Exact Authority and
Bounded Transition Observation; implementation remains unauthorized.

### The missing layer

v0.3 primarily qualified bounded or atomic interactions. Many real GUI tasks
continue after an operation changes the accessibility surface:

```text
operation
  → semantic state transition
  → new, removed, or replaced accessibility surface
  → target re-resolution
  → continued verified interaction
  → task completion
```

GUI2TUI already has event processing, cache refresh, runtime generations,
operation tickets, scene rebuilding, and `InteractionScope`. The open question
is whether these pieces support a generic transition-aware task model, or
whether recurring evidence justifies a small additional abstraction. v0.4
should make task progression state-aware rather than treating every step as an
unrelated snapshot operation.

### Discovery candidates

The first v0.4 work must be Discovery, not implementation. It should survey a
small corpus of real generic transitions and determine which contracts recur.
Candidate problem families include:

- Expand or Collapse followed by descendant realization;
- menus, popovers, and context-menu lifecycle;
- dynamic choice surfaces that appear or replace their contents;
- tree expansion followed by descendant discovery and interaction;
- dialog creation and transition into a new active `InteractionScope`;
- dialog confirmation or dismissal and return to the prior surface;
- creation and destruction of temporary windows;
- accessible file-chooser lifecycle;
- operations followed by semantic subtree replacement.

These are evidence candidates, not promised v0.4 features. The common
hypothesis to test is:

```text
explicit semantic invocation
  → observe a bounded semantic transition
  → rebind or re-resolve within current runtime authority
  → verify authoritative intermediate state
  → continue toward an explicit completion condition
```

Discovery must inspect current transition/runtime architecture, real dynamic
task failures, event-driven observation, target rebinding, scope ownership,
timeouts and cancellation, realization evidence, and completion conditions.
It must distinguish a state change such as `Expanded=true` from a stronger
claim such as complete descendant realization.

### No script engine

v0.4 must not turn GUI2TUI into a delay-and-input automation system. A sequence
such as “press a key, sleep, click a coordinate, wait 500 ms, press Enter” has
no semantic target or authoritative transition contract. Keyboard and mouse
injection, coordinate scripting, guessed delays, anonymous action sequences,
and action-index fallback remain forbidden.

A workflow step must name an explicitly compatible public semantic operation,
observe relevant application state, retain generation and scope authority, and
verify its outcome. Time bounds may limit waiting; elapsed time must never be
treated as proof that a transition succeeded.

### No premature workflow abstraction

The roadmap does not pre-approve a Workflow DSL, `CompoundSemanticOperation`,
transaction engine, workflow graph IR, or YAML automation system. A shared
workflow/transition abstraction is justified only if evidence from multiple
genuinely different task families shows recurring ownership, observation,
re-resolution, cancellation, and completion structure that cannot remain
clear in operation-specific code.

If one candidate is simply one named public action followed by existing cache
refresh, implement no compound framework. The v0.3 Expand/Collapse evidence is
an important warning: explicit state mutation alone did not establish generic
ownership between the action target and realized descendants.

### Success direction

A successful v0.4 should demonstrate multiple generic dynamic tasks chosen by
future evidence. Illustrative shapes include:

- expand a semantic item, observe qualified realization, then interact with a
  newly available descendant;
- open a semantic popup or menu, discover its current choices, choose one, and
  verify the resulting state;
- open a dialog, enter its active scope, perform verified interaction, confirm
  it, and observe task completion or return to the prior scope.

Success means stateful semantic task progression with deterministic refusal on
stale, ambiguous, timed-out, or unverifiable transitions. It is not the number
of newly interactive controls.

### Recommended next task

The immediate recommended technical task is **0.4A Exact Authority and Bounded
Transition Observation**. It is **awaiting explicit user authorization**.
Discovery completion does not authorize implementation.

## 5. v0.5 — Task & Interaction Completeness

**Core question:** Can the common interaction families of ordinary desktop
applications actually be used to complete tasks?

v0.5 should build on semantic reconstruction, spatial composition, verified
primitive capabilities, and transition evidence produced by v0.4. Its purpose
is coverage at the task level. It should identify gaps that prevent ordinary
work, qualify the relevant public contracts, and add only the bounded
interaction needed to complete those generic tasks.

### Candidate interaction families

Likely evidence areas include menus, context menus, popup choices, tabs,
tree/disclosure navigation, lists, single selection, deselection,
multi-selection, dialogs, common forms, checkbox/toggle/radio, text, Value,
Choice, file choosers, directory/file selection, table navigation,
semantically reliable row/column/table selection, hyperlinks, actions, and
command surfaces.

This list is not a promise that every family will support every mutation. An
interface may remain read-only when selection semantics are incomplete,
realization is ambiguous, the operation is anonymous, or authoritative
verification is unavailable.

### Task matrix principle

Coverage should be evaluated with generic end-to-end task families:

| Generic task | Required architectural evidence |
| --- | --- |
| Open a menu and invoke a command | explicit open/transition, current items, named action, completion state |
| Open settings, change a field or value, Apply | dialog scope, verified primitive write, confirmation and resulting state |
| Expand a tree and choose a descendant | expansion ownership, bounded realization, descendant identity and selection |
| Open a file chooser, navigate, choose, confirm | surface/scope lifecycle, path semantics, selection and confirmation without bypass |
| Complete and submit a structured form | field capability, validation state, scoped submit action and outcome |
| Navigate and select list/table content | bounded collection identity, current selection, reliable navigation and read-back |

Applications used in validation demonstrate these generic contracts; no
application name, process identity, title, browser/editor brand, or toolkit may
become a production condition.

### File chooser decision

Open, Save As, Choose File, and Choose Folder are common desktop workflows.
Before 1.0, the project must make an explicit evidence-based decision about
generic accessible file-chooser task support. Future discovery must determine
how navigation, selection, location entry, scope changes, overwrite prompts,
and confirmation can be represented through public semantics.

Directly changing application backing state, mutating files to simulate a GUI
choice, injecting input, or adding application-specific dialog adapters remain
forbidden. A safe limitation is acceptable if the accessibility contract
cannot support the full task.

### Non-goals

v0.5 is not “support every AT-SPI role.” It does not require every widget,
toolkit, or desktop application; private rich-text editor integration; or
application-specific adapters. It must preserve password exclusion,
`PartialRealized` honesty, GUI authority, and correct degradation. Useful
generic task completeness—not breadth statistics—is the exit direction.

## 6. v0.6 — Runtime Continuity & Multi-Surface Robustness

**Core question:** Can GUI2TUI remain correct and usable through long-running,
dynamic application lifecycle changes?

The repository already contains substantial recovery foundations:
`RuntimeSession`, `RuntimeSessionId`, `ApplicationGenerationId`, operation
tickets, cache invalidation, endpoint lifecycle, and `InteractionScope`. v0.6
should qualify runtime continuity as observable product behavior, rather than
assuming that individual architectural mechanisms compose correctly over a
long session.

### Runtime problem areas

Expected evidence areas include:

- application restart and generation replacement;
- endpoint disappearance and reappearance;
- AT-SPI reconnect and recoverable session-bus interruption;
- window creation and destruction;
- multiple windows and multiple applications;
- modal/dialog and popup churn;
- event overflow, backpressure, and cache invalidation;
- stale target rejection and late operation completion;
- long-running external-handler interaction while GUI state changes;
- terminal detach/reattach where supported;
- artifact and session cleanup;
- bounded memory and file-descriptor growth;
- cancellation and controlled recovery after partial runtime failure.

This roadmap does not predict a new runtime type or recovery framework.
Discovery should first identify which current mechanisms fail under realistic
continuity workloads and which behavior belongs in the 1.0 support contract.

### Multi-surface user model

Before 1.0, GUI2TUI needs a clear generic answer to four user-facing
questions: which application is active, which window or surface is active,
which modal scope owns interaction, and how a user moves between eligible
applications or windows. When a surface disappears, its bindings and pending
operations must lose authority without making another similar-looking target
an implicit replacement.

This must extend the existing identity model rather than create a second one.
`RuntimeSession`, `ApplicationGenerationId`, exact locators, operation tickets,
and `InteractionScope` remain foundational. Labels, geometry, role similarity,
or text similarity must not become fuzzy target reconciliation.

### Success direction

A representative continuity story might move from application A to application
B, enter a dialog, close it, return to the prior surface, observe an
application restart, reject all operations owned by the old generation,
acquire the new generation, and remain usable. Long-running validation should
also show bounded event queues, memory, file descriptors, artifact lifetime,
and recovery attempts.

The exact sequence must follow future evidence. Success is continued correct
authority and usable recovery, not hiding interruptions or retrying forever.

## 7. v0.7 — Deployment & Environment Completeness

**Core question:** Can users reliably run GUI2TUI in every environment that
the 1.0 product contract claims to support?

Earlier releases have qualified package architectures and selected Linux
validation environments. v0.7 must turn deployment assumptions into an
explicit, reproducible support boundary. Unsupported configurations should be
diagnosed honestly rather than inferred from a nearby success.

### Environment questions

At minimum, v0.7 should review:

- a local Linux graphical session;
- X11 and the exact meaning of any Wayland support;
- headless/Xvfb-style operation;
- Accessibility/session D-Bus discovery and permissions;
- installation, first run, and `doctor` diagnostics;
- required terminal capabilities;
- terminal suspend/resume for configured external text handlers;
- native x86_64 and aarch64 packages;
- archive extraction or other supported installation flow;
- configuration portability and XDG paths;
- session permissions and artifact ownership;
- SSH and remote-use expectations.

Each claimed environment needs evidence covering discovery, session ownership,
AT-SPI reachability, terminal lifecycle, interaction, degradation, and package
behavior. “AT-SPI works in principle” is not a deployment qualification.

### Wayland decision gate

The 1.0 contract must say exactly what Wayland support means. It must not infer
full Wayland support merely because AT-SPI is available. Evidence must cover
the actual graphical session, accessibility bus discovery, application
visibility, interaction authority, and relevant external-modality limitations.
If Wayland is outside the 1.0 baseline, that exclusion must be explicit and
user-facing.

### Remote companion decision gate

Current architecture has considered same-host graphical operation,
headless/terminal-oriented use, and a remote companion design, but production
remote transport is not established. v0.7 must choose one reviewed outcome:

1. remote companion is required by the GUI2TUI 1.0 product contract, so its
   implementation and qualification become a prerequisite; or
2. remote companion is not required for 1.0 and is explicitly documented as
   post-1.0 or future work.

The decision must not remain ambiguous at the 1.0 boundary. It must be driven
by the intended deployment contract and evidence, not by the existence of an
architectural sketch.

### Platform boundary

v0.7 is deployment completion, not automatic platform expansion. GUI2TUI is
currently centered on Linux Accessibility and AT-SPI. Windows and macOS
backends are not implied requirements for 1.0 unless the user explicitly
changes project scope.

## 8. From v0.7 to 1.0

After v0.7, the default action is a functional feature freeze and a formal 1.0
readiness review. That review should compare accumulated evidence against the
product contract and qualification dimensions below. It should not create
v0.8 or v0.9 merely to continue development.

If a fundamental layer remains missing, the HANDOFF must name the gap, show
evidence that it blocks the 1.0 contract, propose one bounded milestone, and
request user approval. Cosmetic work, another application row, another widget,
or a safe P2 limitation is not by itself justification for a new architectural
version.

## 9. GUI2TUI 1.0 product contract

At 1.0, for an ordinary Linux GUI application with reasonably complete public
Accessibility semantics and no GUI2TUI-specific integration, GUI2TUI should be
able to:

1. discover the application and its relevant surfaces;
2. reconstruct useful semantic structure;
3. present a usable terminal-native interface;
4. preserve useful spatial and topological organization;
5. expose only capabilities that can be invoked safely and verified
   authoritatively;
6. follow dynamic semantic state transitions;
7. complete common multi-step task families;
8. preserve modal and `InteractionScope` correctness;
9. survive ordinary runtime, window, and application lifecycle changes;
10. preserve external or graphical modality when terminal semantics are
    insufficient;
11. degrade safely when capability, fidelity, or completeness is unavailable;
12. install and operate reliably in explicitly supported deployment
    environments.

This is a generic product contract, not universal compatibility. Applications
whose public semantics are incomplete, misleading, unsafe, or unverifiable may
remain partially usable, read-only, externally represented, quarantined, or
unsupported. Such outcomes are correct when they preserve truth.

## 10. Cross-cutting invariants

The following remain non-negotiable at 1.0 and throughout every intervening
milestone:

- Semantics over pixels; function over layout; tasks over widgets.
- Correct degradation is a feature.
- GUI geometry may influence presentation, never semantic correctness.
- The GUI application remains authoritative.
- Backend acceptance alone is not success, and optimistic mutation is not
  semantic truth.
- Production behavior does not branch on application, process, executable,
  title, browser/editor brand, or toolkit identity.
- No private application/toolkit semantic backend replaces public
  Accessibility.
- No input injection, coordinates, or guessed delays become semantic fallback.
- No anonymous action or action-index guessing.
- No backing-file/database bypass to simulate GUI interaction.
- No password or secret export.
- `PartialRealized` never becomes false completeness.
- Runtime identity, generation, exact target, and active scope constrain every
  operation and transition.
- Presentation can explain or omit unavailable capability, but cannot create
  it.

## 11. 1.0 qualification dimensions

Future 1.0 qualification should evaluate at least these dimensions:

### A. Semantic correctness

Roles, state, relations, identity, content completeness, and operation targets
remain faithful to public Accessibility evidence.

### B. Terminal presentation usability

Semantic tasks remain reachable and understandable across supported terminal
sizes without pixel imitation or presentation-driven capability changes.

### C. Verified primitive interaction

Single-step mutation families retain explicit eligibility, bounded invocation,
stale/scope safety, and authoritative read-back.

### D. Dynamic workflow completion

The runtime observes transitions, re-resolves targets safely, and reaches
defined completion or honest refusal across representative workflows.

### E. Common task completeness

A reviewed set of ordinary task families can be completed generically; gaps
are classified by semantic cause rather than hidden by widget counts.

### F. Runtime continuity

Applications, windows, buses, generations, event streams, handlers, and
terminal sessions can change without stale mutation or unbounded degradation.

### G. Deployment and support contract

Every claimed environment, architecture, installation path, and session model
has reproducible evidence and truthful diagnostics.

### H. Security and privacy boundaries

Secrets, artifact ownership, configured processes, scope, conflict handling,
backing-file separation, logs, and forbidden fallbacks retain their guarantees.

### I. Performance and resource bounds

Acquisition, caches, event processing, content, workflow waits, artifacts,
memory, and file descriptors remain bounded for the stated product scale.

### J. Packaging, installation, and doctor

Released packages install or extract cleanly, report correct identity, start
without optional handlers, and diagnose missing environment prerequisites.

### K. Documentation

User and engineering documentation describe real capabilities, safe
limitations, configuration, supported environments, and authority boundaries
without overclaiming.

### L. Release reproducibility

Immutable source, multi-architecture artifacts, version, manifest, checksums,
ABI, provenance, public bytes, and release evidence remain reproducible and
distinguishable from later documentation commits.

Quality is not defined by test count. Automated tests, controlled fixtures,
real application tasks, long-running exercises, package smokes, and audits
should each provide the evidence appropriate to the contract.

## 12. Task-based 1.0 validation

The eventual compatibility corpus should be organized around tasks such as:

- navigate to and invoke a command;
- edit a simple field and verify the GUI result;
- adjust a bounded Value;
- choose an option;
- complete and submit a form;
- open, interact with, and close a modal dialog;
- navigate a dynamic menu or popup;
- expand hierarchical content and use a realized descendant;
- choose a file or folder where the public contract qualifies it;
- navigate, read, search, and select useful list/table content;
- edit qualified complete complex plain text;
- preserve graphical or external modality where terminal semantics are
  insufficient;
- survive an application restart without reusing stale authority;
- switch among multiple active surfaces while respecting modal scope.

The exact matrix should be defined from future Discovery evidence. Application
rows are useful for variation and regression evidence, but production behavior
must remain generic.

## 13. Explicit non-requirements for 1.0

GUI2TUI 1.0 does not automatically require:

- compatibility with every desktop application or every AT-SPI role;
- making every exposed widget writable;
- lossless rich-text editing;
- private Monaco or VS Code editing;
- DOM/CDP browser integration;
- LibreOffice UNO;
- private GTK, Qt, Electron, or application adapters;
- OCR, vision, or screenshot semantic inference;
- pixel-perfect GUI rendering;
- arbitrary drag-and-drop emulation;
- universal external-editor compatibility;
- keyboard or mouse injection;
- application-specific production rules;
- Windows or macOS backends.

Correct unsupported, partial, read-only, quarantined, or external-modality
results are acceptable when the public semantic contract cannot support a
faithful and verifiable terminal task.

## 14. Pre-1.0 change control

### Milestone method

Every future milestone follows the same discipline:

1. perform Discovery and collect evidence where the architecture is uncertain;
2. define bounded implementation phases from that evidence;
3. validate generic tasks in controlled fixtures and a small real-app corpus;
4. close the milestone with explicit P0/P1/open questions and a HANDOFF;
5. recommend one next step and wait for user review.

Do not self-expand, begin the next milestone automatically, introduce a
speculative generic framework, optimize for test or application count, or
solve P2 compatibility limitations with application-specific hacks.

### Version scope discipline

Each version should solve one architectural or product-layer question. “Add
trees, menus, and fifteen controls” is a poor milestone definition. “Enable
verified semantic state transitions so dynamic tasks can continue after GUI
structure changes” identifies an architectural gap and a meaningful exit
direction. Individual features are subordinate evidence for the question.

### Roadmap change process

This roadmap is strong direction, not immutable architecture. Future evidence
may justify changing milestone scope or order, merging milestones, or deferring
a proposed capability. Such a change must not happen silently because an
implementation path became convenient. A meaningful change must be documented,
supported by evidence, presented in a HANDOFF, and approved by the user.

The status vocabulary is `COMPLETED`, `CURRENT`, `PLANNED`, `RECOMMENDED`,
`AWAITING USER AUTHORIZATION`, `DEFERRED`, and `NOT REQUIRED FOR 1.0`.
`CURRENT` may be applied only after explicit authorization. Completion of one
milestone never changes the next milestone to `CURRENT` automatically.

## 15. Immediate next decision

The v0.4 Discovery is complete. After user architectural review, the
recommended next technical task is **0.4A Exact Authority and Bounded
Transition Observation**, as bounded in the [v0.4 roadmap](v0.4-roadmap.md).
It is not authorized by this roadmap update. v0.5, v0.6, v0.7, and 1.0 likewise
remain direction only until separately authorized.

## 16. References

- [`AGENTS.md`](../../AGENTS.md) — normative repository rules.
- [Engineering guide](../project-guide.md) — current architecture, history,
  module map, and safety model.
- [Architecture](../architecture.md) — semantic/runtime pipeline.
- [v0.3 roadmap](v0.3-roadmap.md) — completed historical capability-recovery
  milestone.
- [v0.3 discovery record](v0.3-capability-recovery.md) — evidence that shaped
  verified capability recovery.
- [v0.3 capability UX validation](../validation/v0.3/capability-ux/HANDOFF.md)
  — final functional-phase evidence.
- [v0.3 production release verification](../validation/v0.3/release/HANDOFF.md)
  — immutable public release identity and current post-release evidence.
- [v0.4 workflow reconstruction Discovery](v0.4-workflow-reconstruction.md) —
  evidence and conclusion B.
- [v0.4 bounded continuation roadmap](v0.4-roadmap.md) — derived phases; no
  implementation authorization.
