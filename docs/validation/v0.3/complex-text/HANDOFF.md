# GUI2TUI v0.3 Phase 0.3C Complex Text Validation

## Status and scope

Phase 0.3C is validated for configurable complex plain-text interaction.
Production commit: `2a242b6` (`feat: add configurable complex text
interaction`). The phase did not implement Selection, compound operations,
rich-text editing, private application adapters, or release work.

## Stage A: controlled complete-text contract

Stage A ran first on 2026-09-04 in the existing `gui2tui-live` Ubuntu 24.04
arm64 environment with Xvfb/X11, session D-Bus, AT-SPI 2.52, and GTK 4.14.5.
A controlled `Gtk.TextView` named `External text probe` exposed AT-SPI role
Text, Text and EditableText interfaces, Editable and MultiLine states, no
children, no Document interface, and no password state. The original complete
21-character payload was read from `0..CharacterCount`; its sole attribute run
contained no non-default formatting.

A small line was added through public `EditableText.SetTextContents`. The
method accepted the request, a separate full Text read exactly matched the
candidate, the original text was restored with the same public operation, and
a further independent read exactly matched the original. Thus completeness,
safe public mutation, authoritative read-back, and restoration were established
before the configured-handler path was implemented:

```text
STAGE_A_COMPLEX_TEXT_WRITE=PASS
```

The historically unsafe Qt multiline Text path was not probed.

## Runtime qualification

`SemanticCapability::EditComplexText` is added only after both the existing
content runtime and a fresh backend qualification agree. A candidate must be:

- a currently visible content root with `ContentCompleteness::Complete` and
  `TextCapabilityStatus::Verified`;
- semantic `TextInput` with plain input kind, Editable and MultiLine state,
  Text plus EditableText, and no semantic children;
- raw AT-SPI role Text, not PasswordText, with no Document interface,
  ReadOnly or ManagesDescendants state, and zero children;
- exactly readable from character offset zero through CharacterCount;
- valid UTF-8 within 262,144 bytes and 262,144 characters;
- composed only of bounded, advancing Text attribute runs with no non-default
  attributes.

Qualification probes at most eight sorted visible roots and at most 512
attribute runs per candidate. Declared or Quarantined Text capability is not
retried. PartialRealized, virtualized, rich, oversized, secret, unsupported,
or unverifiable targets remain read-only.

## Configured interaction path

The end-to-end path is:

```text
qualified DocumentSummary + e
→ UiIntent::BeginExternalEdit
→ fresh complete text A + exact runtime identity/scope
→ ExternalTextSession + OwnedArtifactDirectory/OwnedArtifactFile
→ direct configured argv process with private {file}
→ bounded candidate C from the original owned file descriptor
→ fresh GUI text B and generation/locator/scope checks
→ SemanticOperation::ReplaceComplexText
→ BackendOperation::SetComplexTextContents
→ EditableText.SetTextContents(C)
→ independent complete Text read-back
→ full authoritative scene refresh
```

No candidate text is written to the cache or presentation optimistically.
`LocalModalityBroker` and resource modality handlers are not used as mutation
backends; only their already-sound artifact/runtime ownership primitives are
reused.

## Handler configuration and terminal lifecycle

The optional TOML form is:

```toml
[interaction.complex_text]
program = "custom-editor-command"
args = ["--wait", "{file}"]
```

The executable is user configuration, not application data. GUI2TUI calls it
directly with argv: there is no command-string evaluation, `sh -c`, default
editor, `$EDITOR`, `$VISUAL`, application matching, toolkit matching, daemon,
RPC, or plugin system. Exactly one standalone `{file}` argument is required.
Without this section the target remains readable and `e` reports `Edit handler
not configured; target remains read-only`.

Before launch GUI2TUI retires the Crossterm input reader, leaves raw mode and
the alternate screen, and marks the runtime terminal detached. It waits for
the handler synchronously while the handler owns the terminal, then restores
the normal terminal modes, creates one new event reader, invalidates the frame,
marks the runtime attached, and redraws authoritative state. The controlled
handler verified ICANON and ECHO while active.

## Private artifact and security contract

The representation is a 0600 regular file in a 0700 GUI2TUI-owned leased
directory associated with the runtime session and operation ticket. It
contains text only: no `BackendLocator`, runtime ID, backing path, URI,
password, or private backend descriptor is exported. Both input and result are
bounded; truncation is never writable.

After the handler exits GUI2TUI checks the path with `symlink_metadata` and
requires the same device/inode as the original open descriptor, one link,
current-user ownership, private permissions, and a regular file. It reads from
that original descriptor, not by following a replaced path. Success and
unchanged outcomes clean the temporary namespace. A modified candidate is
retained under the existing 30-minute owned-artifact lease after conflict,
stale, rejected, or unverified write-back, and its recovery path is reported.
The phase deliberately does not attempt to sandbox explicitly user-authorized
local code.

PasswordText is rejected before Text content access or artifact creation.
No secret payload was read, logged, exported, or tested.

## Conflict, stale, and write verification

The session captures `ApplicationGenerationId`, exact `RuntimeNodeId`, exact
`BackendLocator`, `InteractionScopeId`, and complete starting text A. After the
handler returns, GUI2TUI settles and applies pending AT-SPI events, then requires
the same generation, node-to-locator binding, semantic capability, and active
scope. It freshly reads current GUI text B and refuses write-back when `B != A`.
There is no fuzzy target reconciliation and no merge.

The backend repeats the complete read-and-compare immediately before the
public setter. Setter acceptance is invocation evidence only. A second
independent complete read must equal C before success is reported. Rejection,
read failure, or differing authoritative content never reports success and
preserves modified user work privately.

## Controlled workflows

The validation-only handler is supplied through the public TOML path and
appends `handler candidate C\n` to its private file. It also verifies that the
terminal is canonical with echo enabled.

Positive workflow:

```text
A = "alpha line\nbeta line\n"
C = A + "handler candidate C\n"
fresh pre-write B = A
public EditableText write accepted
fresh complete authoritative read = C
TUI status = External text update confirmed
EXTERNAL_TEXT_END_TO_END=PASS
```

Conflict workflow:

```text
A = "alpha line\nbeta line\n"
C = A + "handler candidate C\n"
independent GUI action changes B to "GUI concurrent B\n"
decision = conflict; no write of C
GUI final = B
C retained as one private regular 0600 artifact in a private directory
EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS
```

Failure workflow:

```text
configured handler exits 7
GUI remains A; no AT-SPI write
terminal reattaches and renders concise failure status
EXTERNAL_TEXT_HANDLER_FAILURE=PASS
```

Additional safe-degradation evidence:

```text
EXTERNAL_TEXT_READ_ONLY=PASS
EXTERNAL_TEXT_NO_HANDLER=PASS
```

## Real application evidence

- Mousepad 0.6.1: **EXTERNAL EDIT QUALIFIED** through the same generic role,
  interfaces, completeness, bound, conflict, and read-back path. A copied
  validation document changed in the live GUI buffer; byte comparison proved
  the file Mousepad had opened remained unchanged
  (`EXTERNAL_TEXT_BACKING_FILE_BYPASS=ABSENT`).
- Controlled GTK TextView: **EXTERNAL EDIT QUALIFIED** and used for the Stage A,
  positive, conflict, failure, unavailable-handler, and read-only controls.
- LibreOffice Writer 24.2.7: **READ-ONLY — INCOMPLETE/RICH**. Its document did
  not show the external-edit affordance; no UNO or flattening was attempted.
- Qt multiline: **QUARANTINED** from prior unsafe Text evidence and not
  re-probed.

These application names are validation evidence only. Production contains no
application, process, window, browser, editor, or toolkit branch.

## Regression and forbidden-path audit

- Native single-line `EditSession` was not rerouted or rewritten.
- Verified Value eligibility, arrows, bounds, read-back, progress refusal, and
  scrollbar suppression were not refactored; existing Value tests pass.
- Selection and Expand/Collapse were not implemented.
- Reader remains a viewing task; its PartialRealized or bounded representation
  is never used for whole-target mutation.
- External Modality resource acquisition remains separate from semantic state
  mutation.
- No application backing-file write, DOM/CDP, UNO, private toolkit API,
  OCR/vision, clipboard/keyboard/mouse injection, coordinate action, anonymous
  action guess, remote handler, or non-text handler was introduced.

## Tests and phase-close quality

One focused unit test covers shell-free configuration and the exactly-one
standalone `{file}` rule. The committed Linux harness covers controlled
positive, conflict, handler failure, unavailable handler, read-only refusal,
private artifact permissions, Mousepad backing-file separation, and Writer
refusal. It is validation evidence rather than a new fake AT-SPI framework.

Phase-close results:

- macOS: `cargo fmt --all -- --check`, `cargo check --all-targets`,
  `cargo test --all-targets`, and `cargo clippy --all-targets -- -D warnings`
  passed; 277 library tests, 2 inspector CLI tests, and 4 user CLI tests passed.
- Linux: the final arm64 build passed and all live workflows above passed in
  independent session D-Bus/Xvfb environments.
- Python fixture/probe compilation, shell syntax validation, and
  `git diff --check` passed.

P0: 0. P1: 0.

## Conclusion and next recommendation

The configured local interaction model preserves GUI2TUI's authority boundary:
the handler edits only a private bounded candidate, while GUI2TUI retains
semantic identity, conflict authority, public Accessibility mutation, and
fresh read-back confirmation. Correct refusal remains part of the feature.

Recommend **Phase 0.3D — Compound Interaction & Capability UX Qualification**
next, awaiting explicit user authorization. Selection recovery and release
work remain not authorized, and no later phase is automatically authorized.
