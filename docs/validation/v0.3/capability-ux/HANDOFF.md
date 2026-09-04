# GUI2TUI v0.3 Phase 0.3D Capability UX Validation

## Status and scope

Phase 0.3D is validated for compound-interaction qualification and clear,
terminal-native capability UX. Production commit `14e635b` refines only
presentation and handler-availability context. It does not implement
Expand/Collapse, broad Selection, a compound operation framework, new mouse
interaction, spatial redesign, private APIs, or release work.

## Fresh compound evidence

Validation ran on 2026-09-04 in the existing `gui2tui-live` Ubuntu 24.04 arm64
environment with Xvfb/X11, session D-Bus, AT-SPI 2.52, and GTK 4.14.5. GTK Demo
provided a controlled disclosure candidate named `Constraints`:

```text
role=Button
states-before=expandable,expanded,focusable,sensitive,showing,visible
interfaces=Accessible,Action,Component
actions=listitem.toggle-expand,listitem.collapse,listitem.expand
```

The probe selected the exact named `listitem.collapse` action, never an action
index or anonymous action. A fresh complete inspection found the same exact
`BackendLocator`, retained Expandable, removed Expanded, and no longer exposed
the realized `Simple Constraints` entry. Children-changed and expanded state
events were observed. The exact named `listitem.expand` action then restored
Expanded and the realized entry; a further fresh inspection confirmed both.

```text
EXPAND_COLLAPSE_EXPLICIT_ACTION=PASS
EXPAND_COLLAPSE_STATE_READBACK=PASS
EXPAND_COLLAPSE_REALIZATION=PASS
EXPAND_COLLAPSE_RESTORATION=PASS
```

## Expand/Collapse decision

The evidence qualifies the public action and state-observation pieces, but not
a generic product operation. This GTK shape exposes the Action on a Button and
realizes sibling ListItems beneath a surrounding list; no public relation in
the observed data generically establishes that those rows are owned by that
disclosure target. The mutation itself is also one named public Action followed
by normal authoritative refresh, not a genuinely multi-step semantic mutation.

Decision: **DEFERRED — GENERIC TARGET/REALIZATION OWNERSHIP NOT QUALIFIED**.
GUI2TUI did not add Expand/Collapse or `CompoundSemanticOperation`. A shared
compound abstraction would require evidence from at least two distinct generic
workflows with common intermediate verification, deadlines, scope ownership,
cancellation, and realization semantics; that evidence does not exist.

## Normal capability UX

Normal presentation now describes user tasks rather than internal interface
taxonomy:

- a focused verified Value shows `↑/↓ Adjust` in the contextual footer;
- a complete external-text target with configured handler shows `e Edit
  externally` and the document action shows `e: Edit externally`;
- the same qualified target without a configured handler remains readable and
  says `external edit not configured`, with no fake edit key;
- fields and informational Values without safe mutation say `read only`;
- selectors without a safe choice operation say `options unavailable`;
- action-like controls without safe invocation say `action unavailable`;
- structural fallback summaries say `Read only` instead of exposing the
  overloaded product-internal word `Unsupported`.

The Inspector continues to expose roles, state, interfaces, named actions,
semantic capabilities, completeness, and locators for diagnosis. Presentation
does not manufacture or alter any backend capability.

## Existing capability families

Single-line `EditSession` is unchanged: editable inputs remain native and
authoritative read-back still gates success. Value semantics and mutation are
unchanged; only the contextual hint was made explicit. Complete complex text
continues through the configured `program + argv + {file}` path, with the
existing complete-text, conflict, stale, public write, and read-back gates.

Reader remains a viewing task. PartialRealized Writer content showed no edit
affordance and no missing-handler message. Historically quarantined Qt
multiline Text was not probed. PasswordText exclusion was not weakened and no
password content, length, handler eligibility, or representation was exposed.

## Conflict and failure UX

The existing normal statuses distinguish unchanged, handler failure, stale,
conflict, application rejection, and unverified write without displaying raw
D-Bus traces. Conflict states explicitly say that the GUI was not overwritten.
When modified work is preserved, the status reports that it is private and
retains the existing recovery path because it is the only direct recovery
handle; the GUI remains authoritative.

Controlled workflows on the current production code passed:

```text
EXTERNAL_TEXT_END_TO_END=PASS
EXTERNAL_TEXT_NO_HANDLER=PASS
EXTERNAL_TEXT_READ_ONLY=PASS
EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS
EXTERNAL_TEXT_HANDLER_FAILURE=PASS
```

## Live application evidence

- Controlled GTK TextView: configured and missing-handler affordances were
  truthful; positive write/read-back, conflict refusal, and handler failure
  passed.
- Mousepad 0.6.1: generic configured interaction passed again in isolated app
  configuration. The visible buffer changed through AT-SPI while the opened
  validation file remained byte-identical
  (`EXTERNAL_TEXT_BACKING_FILE_BYPASS=ABSENT`).
- EOG 45.3: current scene preserved graphical modality and exposed zero
  writable Value scene elements (`CAPABILITY_UX_EOG_VALUE_REFUSAL=PASS`).
- Qt6 controlled fixture: Slider `Probe value` changed authoritatively 4 → 5
  through the normal TUI and restored 5 → 4; ProgressBar `Probe progress`
  remained non-writable.
- Qt Designer was not installed in the existing VM, so no result was
  fabricated. GTK Demo supplied the controlled compound evidence instead.
- LibreOffice Writer 24.2.7: PartialRealized/rich document remained read-only
  with no whole-target edit affordance.

```text
CAPABILITY_UX_VALUE=PASS
CAPABILITY_UX_VALUE_RESTORATION=PASS
CAPABILITY_UX_PROGRESS_READ_ONLY=PASS
```

An ordinary-editor compatibility smoke remained optional and was not run. No
editor-specific behavior, preset, detection, or security relaxation was added.

## Genericity, security, and scope audit

Production checks only semantic role, capability, completeness, current focus,
and whether generic complex-text handler configuration exists. Application,
toolkit, browser, fixture, and editor names occur only in validation code and
documentation. No DOM/CDP, UNO, private GTK/Qt/Electron API, OCR/vision,
keyboard/mouse injection, coordinate action, action-index fallback, backing-file
mutation, or pixel layout copying was introduced.

Selection, mouse UX, spatial topology, resource External Modality, and release
workflow are unchanged. The external handler remains a separate semantic
mutation modality using direct argv and a private GUI2TUI artifact; it was not
merged into the resource broker or extended beyond text.

## Tests and quality

One focused renderer regression test covers truthful configured/missing-handler
affordances and read-only structural wording. Existing focused help tests and
the live v0.3C workflows cover the other modified behavior. The phase did not
add a wording matrix or new testing framework.

Phase-close results:

- macOS: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo
  test --all-targets`, and `cargo clippy --all-targets -- -D warnings` passed;
  278 library tests, 2 inspector CLI tests, and 4 user CLI tests passed;
- Linux arm64: the current binaries built and the controlled GTK/Qt Value,
  external-text positive/missing/conflict/failure, Mousepad, Writer,
  Expand/Collapse evidence, and EOG refusal workflows above passed in isolated
  session D-Bus/Xvfb environments;
- Python fixture/probe compilation, shell syntax checks, and `git diff --check`
  passed.

P0: 0. P1: 0. Remaining P2 questions are generic disclosure-to-realization
ownership and compatibility with editors that replace the artifact inode;
neither weakens a current verified capability.

## Conclusion and next recommendation

v0.3 now has one coherent verified-capability model across native single-line
text, native bounded Value, configured complete complex text, and safe
qualification/degradation for compound candidates. Normal UX communicates
what the user can do; Inspector retains implementation evidence.

Functional development is complete. Recommend **v0.3.0 RC qualification**
next, awaiting explicit user authorization. This recommendation neither starts
RC nor authorizes release work.
