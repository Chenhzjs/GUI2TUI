# GUI2TUI v0.2.0 (release candidate)

GUI2TUI v0.2 adds generic spatial reconstruction on top of the semantic
runtime introduced in v0.1. Accessibility-exposed geometry is presentation
evidence: it helps compose useful terminal regions, but never changes semantic
correctness or operation safety.

## What changed

### Spatial reconstruction

- Spatial evidence and topology preserve meaningful relationships without
  copying GUI pixels or coordinates.
- Region presentation keeps important content, inputs and context reachable as
  terminal width changes.

### Responsive composition

The v0.2 spatial layout is now the default. Regions can split, stack, collapse,
summarize or move into navigation according to available terminal space.
`--layout flat` remains an explicit compatibility fallback.

### Region navigation / terminal UX

The terminal-generated Region Navigator shows at most two useful levels:

```text
F6 / Shift+F6       major region
Ctrl+Tab             sibling pane
Ctrl+Shift+Tab       previous sibling pane
Tab / Shift+Tab      control in the active pane
```

This presentation is distinct from semantic GUI TabList controls. Empty or
low-value surfaces collapse safely while remaining discoverable; unavailable,
read-only and checked states use compact, honest wording.

## Compatibility evidence

The release-candidate smoke covered Mousepad, Chromium, Firefox, EOG, GTK Demo,
Qt Designer, LibreOffice Writer and best-effort VS Code/Electron using existing
AT-SPI workflows. Results and safe limitations are recorded in the
[compatibility matrix](compatibility.md) and [validation report](phase4c-validation.md).

## Installation

Build from source with Rust 1.88 or newer:

```bash
cargo build --release --locked
./target/release/gui2tui --version
```

Official release candidates use native Linux x86_64 and aarch64 archives from
the existing Ubuntu 22.04 pipeline. Verify downloaded archives with
`sha256sum -c SHA256SUMS`.

## Known limitations

- Accessibility quality varies by application and toolkit.
- Large Chromium trees may require a several-second correctness fallback when
  AT-SPI Cache data is incomplete.
- Long documents may be `PartialRealized`; Reader/search never claim unseen
  content.
- Electron and some Firefox/Qt controls remain application-dependent or
  read-only; anonymous actions are refused.
- Multiline/rich-text editing, password editing, Wayland capture, remote
  companion transport, new-TTY attach and live video/game/3D surfaces remain
  out of scope.

See [limitations](limitations.md) for the complete safety boundaries.

## Verification / checksums / attestations

The release workflow produces `SHA256SUMS`, `RELEASE-MANIFEST.json`, measured
ABI metadata and GitHub artifact attestations for exactly x86_64 and aarch64.
The v0.2.0 RC qualification run uses `publish=false`; public-download
verification and the final tag/release are intentionally deferred until an
explicit release authorization.
