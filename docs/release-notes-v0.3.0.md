# GUI2TUI v0.3.0 (release candidate)

GUI2TUI v0.3 adds **Verified Capability Recovery** to the semantic runtime and
responsive spatial presentation established in v0.1 and v0.2. The question is
not merely whether an Accessibility interface exists, but whether the exposed
operation can be invoked safely against current identity and confirmed through
fresh authoritative GUI state.

## Highlights

- Qualified plain single-line fields retain native atomic editing with
  conflict detection and authoritative read-back.
- Enabled Slider/SpinButton-style controls with finite public bounds and a
  positive increment gain compact terminal-native Value adjustment.
- Complete, bounded, non-secret multiline plain-text targets may use an
  optional user-configured local interaction handler.
- Normal capability UX distinguishes writable actions, read-only content,
  incomplete targets, unavailable options, and missing handler configuration
  without exposing raw AT-SPI details.

## Configurable complex text

The handler is configured as direct argv, not a shell command:

```toml
[interaction.complex_text]
program = "custom-editor-command"
args = ["--wait", "{file}"]
```

`{file}` is a private GUI2TUI-owned candidate representation, never the GUI
application's backing file. After the configured process exits, GUI2TUI checks
generation, exact target, active scope, and current complete GUI text. Only an
unchanged starting state permits public AT-SPI write-back, and a further full
read must confirm the result. Conflicts are refused without merge and modified
candidate work is retained privately for recovery. No editor is required or
selected by default; the real v0.3 demo validated Vim through this generic
configuration without a production special case.

## Safety model

- Backend/process return values are invocation evidence, not semantic success.
- Requested text or Value is never installed optimistically as truth.
- Stale generations, changed targets, modal scope changes, conflicts,
  rejection, unchanged results, and unverified reads remain failures.
- PasswordText is excluded before reads, artifacts, handler eligibility,
  diagnostics, and mutation.
- Handler execution is shell-free; private artifact ownership, permissions,
  size bounds, inode identity, cleanup, and failure preservation are checked.
- No backing-file mutation, DOM/CDP, UNO, private toolkit API, OCR/vision,
  keyboard/mouse injection, or anonymous action guessing is used.

## Compatibility and intentional limits

- Rich, formatted, virtualized, and `PartialRealized` documents are not
  flattened into writable whole-text targets. Writer long/rich content remains
  Reader-only for whole-document mutation.
- The historically unsafe Qt multiline Text path remains generation-scoped
  quarantine and is not re-probed automatically.
- ProgressBar/LevelBar Values remain informational, and ScrollBars do not
  become generic writable controls.
- Editor compatibility is bounded by the private-artifact contract; handlers
  that replace the artifact inode are safely refused rather than special-cased.
- Broad Selection mutation and generic Expand/Collapse recovery are not part of
  v0.3. Anonymous actions remain unavailable.

## Inherited v0.2 behavior

Responsive spatial composition remains the default, with F6 major-region and
Ctrl-Tab pane navigation. `--layout flat` remains the compatibility fallback.
Reader, Outline, Search, Choice, commands, modal confinement, external resource
modality, runtime recovery, and honest `PartialRealized` presentation remain
available.

## Demonstration and verification

The [real v0.3 demonstration](https://github.com/Chenhzjs/GUI2TUI/blob/v0.3.0/docs/demo/v0.3/README.md)
shows Value `4 → 5 → 4`, a configured external edit confirmed in Mousepad while
its backing file remains unchanged, conflict refusal with candidate
preservation, and safe Writer degradation. The public release was rebuilt from
and attested against the exact source commit qualified by the v0.3.0 RC
workflow: `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
