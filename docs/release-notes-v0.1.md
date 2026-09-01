# GUI2TUI v0.1.0

First public release of GUI2TUI's semantic GUI-to-terminal runtime.

GUI2TUI recompiles Linux AT-SPI accessibility semantics into terminal-native
controls, commands and readable content. It is not framebuffer-to-ASCII remote
desktop software.

## Downloads and quick start

Choose the Linux `x86_64` or `aarch64` archive, then:

```bash
tar -xzf gui2tui-0.1.0-linux-ARCH.tar.gz
cd gui2tui-0.1.0-linux-ARCH
./bin/gui2tui doctor
./bin/gui2tui
```

## Highlights

- Safe buttons, choices, commands and plain single-line editing
- Reader, Outline, Search, semantic tables and partial collections
- Representative GTK, Qt, Chromium, Firefox and LibreOffice workflows
- Headless operation, reference-first modality and explicit static snapshots
- Event-driven runtime, crash recovery and correctness-first fallbacks

## Verification

```bash
sha256sum -c SHA256SUMS
gh attestation verify gui2tui-0.1.0-linux-ARCH.tar.gz --repo Chenhzjs/GUI2TUI
```

`RELEASE-MANIFEST.json` records source commit, architecture, checksum and
measured GLIBC requirement. Attestation proves build provenance, not general
software security.

## Known limitations

Large Chromium trees can require a multi-second correctness walk while the
accessibility Cache is incomplete. Long documents and Electron applications
may expose only partial semantics. Wayland static capture, remote transport,
new-TTY attachment and continuous live graphics are not implemented.

When usable semantics are unavailable, GUI2TUI remains read-only rather than
guessing an action. Full scope and demo links are in the repository README.
