# GUI2TUI v0.1.0

The first public release of GUI2TUI's semantic GUI-to-terminal runtime.

GUI2TUI turns accessibility semantics exposed by Linux GUI applications into
terminal-native controls, commands and readable content. It is not a
framebuffer-to-ASCII converter or remote desktop.

## Highlights

- Semantic GUI → TUI transcompiler with event-driven live state
- Application selector, keyboard and terminal-mouse operation
- Buttons, checkboxes, choices, menus and scoped commands
- Safe plain single-line text editing with authoritative GUI read-back
- Reader, Outline, Search, tables and explicitly partial collections
- GTK and Qt representative control workflows
- Chromium, Firefox and LibreOffice content workflows
- Headless operation and optional same-host modality handoff
- Reference-first resources and explicit one-frame static visual acquisition
- Crash/lifecycle hardening and bounded correctness fallbacks
- Native Linux x86_64 and aarch64 builds with GitHub provenance attestations

## Validated applications

Representative workflows were run against Mousepad, Qt Designer, Google Chrome,
Mozilla Firefox, LibreOffice Writer and Visual Studio Code, plus controlled GTK
and Qt fixtures. Coverage depends on the accessibility information each
application exposes; this is not a claim of complete support for every control.

## Installation

Download the archive matching `uname -m` from the
[v0.1.0 release](https://github.com/Chenhzjs/GUI2TUI/releases/tag/v0.1.0):

```bash
tar -xzf gui2tui-0.1.0-linux-x86_64.tar.gz
cd gui2tui-0.1.0-linux-x86_64
./bin/gui2tui doctor
./bin/gui2tui
```

The GUI application must already be running in a Linux session whose AT-SPI bus
is reachable. No config file or companion viewer is required.

## Correctness and security principles

- The original GUI remains the source of truth after every operation.
- Anonymous or incompatible actions are refused rather than guessed.
- Password contents are neither read nor written.
- Multiline/rich text is Reader content, not atomically overwritten.
- Partial documents and collections never claim complete source coverage.
- Stale backend identities are rejected across application generations.
- External modality requires explicit user intent and bounded resources.

## Known limitations

- Large fresh Chromium trees may take several seconds while an incomplete
  accessibility Cache requires a recursive correctness walk.
- Long documents may expose only a realized subset through accessibility.
- Electron workflows are partial and application-dependent.
- Complex Qt Designer controls can remain read-only summaries.
- Multiline/rich, password, IME, clipboard and remote-caret editing are absent.
- Wayland static capture, remote companion transport and new-TTY attachment are
  not implemented.
- Live video, game and 3D surfaces are not streamed.

## Checksums and provenance

The release contains `SHA256SUMS` and `RELEASE-MANIFEST.json`:

```bash
sha256sum -c SHA256SUMS
gh attestation verify gui2tui-0.1.0-linux-x86_64.tar.gz \
  --repo Chenhzjs/GUI2TUI
```

Both native packages are smoke-tested after extraction on Ubuntu 22.04 runners.
Attestation proves build provenance, not general software security. See
[release verification](release-pipeline.md).

## Demo and documentation

- [60-second real split-screen demo](https://github.com/Chenhzjs/GUI2TUI/releases/download/v0.1.0/gui2tui-v0.1-demo.mp4)
- [Getting started](getting-started.md)
- [Compatibility](compatibility.md)
- [Limitations](limitations.md)
- [Architecture](architecture.md)

## Acknowledgements

GUI2TUI builds on Linux AT-SPI, Rust, zbus, Ratatui and Crossterm. Thanks to the
GTK, Qt, Chromium, Firefox and LibreOffice accessibility communities whose
public semantic interfaces make this work possible.
