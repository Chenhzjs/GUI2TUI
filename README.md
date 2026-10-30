<p align="center">
  <img src="docs/assets/readme/gui2tui-mark.svg" width="560" alt="GUI2TUI - GUI semantics to terminal tasks">
</p>

# GUI2TUI

**Operate Linux GUI applications from a terminal when they expose sufficient
public Accessibility semantics.**

GUI2TUI recompiles GUI application semantics, capabilities and useful spatial
relationships into a terminal-native interface. It does not stream pixels or
map screen coordinates to terminal cells.

[![Release](https://img.shields.io/github/v/release/Chenhzjs/GUI2TUI?display_name=tag&sort=semver)](https://github.com/Chenhzjs/GUI2TUI/releases/latest)
[![CI](https://github.com/Chenhzjs/GUI2TUI/actions/workflows/ci.yml/badge.svg)](https://github.com/Chenhzjs/GUI2TUI/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

## Why GUI2TUI?

You may have access to a Linux terminal, container terminal or SSH session
while the application you need to use is graphical. GUI2TUI keeps the original
GUI authoritative and presents the application's exposed meaning as a TUI:

```text
GUI application
      ↓
public AT-SPI Accessibility semantics
      ↓
GUI2TUI semantic runtime
      ↓
terminal-native views and operations
      ↓
authoritative GUI state/readback
```

The Headless-first path does not require the user to operate GNOME, KDE, VNC
or a physical display. The GUI application may still need a background Xvfb,
session D-Bus or qualified Wayland compositor on the same host.

## What you can do

For earlier source status and capability boundaries, see [Current source status (2026-10-20)](docs/current-status-20261020.md).
The development JSONL Agent interface now also offers an explicit shared Human
TUI projection. These source capabilities do not imply a new binary release.

For the current shared document, Human TUI projection, exact batch operations,
and AI-assisted development results, see the [project report (Chinese, 2026-10-28)](docs/project-report-20261028.zh-CN.md)
and [validation summary](docs/validation/agent-tui-20261028/summary.json).
The report distinguishes Rust source features from the separately maintained
Jev client. The optional [Jev client setup](docs/jev-client.md) explains the pinned
`integrations/jev-anything` submodule, model configuration and shared-TUI runner.
Ordinary Rust builds and Human TUI usage do not require that submodule.
These changes do not create a new binary release.

The v1.0 baseline remains supported. The v1.1 line adds a richer semantic
Surface and keeps user-visible operations tied to fresh authoritative GUI
state. The current v1.1 release candidate adds:

- discover and explicitly select accessible applications;
- navigate responsive terminal-native scenes, forms and controls;
- operate buttons, toggles, values, single selection, table rows, tabs and
  qualified hierarchies;
- use menus, modal dialogs, Open File and Choose Folder where the application
  exposes verifiable public semantics;
- read document/content regions through Reader, outline and bounded search;
- edit qualified single-line text and complete, bounded, non-secret plain text
  through an optional local handler;
- follow dynamic UI changes and continue manually from the fresh scene;
- reject stale targets and report unsupported semantics instead of guessing.
- preserve application-shell regions alongside document/content regions;
- keep Surface context separate from the explicit Reader projection;
- verify text edits by authoritative Accessibility readback before accepting
  them;
- expose `Send Enter to current control` as an explicit, low-level raw-input
  capability. It is never silently used as Semantic Submit.

The current development branch also exposes **F3 public operations**: choose
an exact advertised action, add/remove selection members, and adjust values
without restricting them to a small widget-role list. Whole-text editing has
a configurable byte budget. See the [operation guide](docs/public-operations.md)
and [new recorded demonstrations](docs/demo/public-operations/README.md).
These additions are source-branch capabilities, not a new published release.

## What it is not

GUI2TUI is not a remote desktop, framebuffer-to-ASCII converter, OCR system,
coordinate-clicking tool, arbitrary automation or application-specific
scripting framework. It does not use DOM/CDP, UNO, private toolkit APIs or
backing-file mutation to manufacture capability. A narrowly bounded native
keyboard backend exists for an explicit Raw Enter intent on a freshly verified
X11 target; key delivery is not semantic authority or proof of success.

## Quick start

New to the terminal interface? Start with the [five-key walkthrough](docs/first-steps.md)
or [中文入门](docs/first-steps.zh-CN.md). No model or API key is needed for Human TUI use.

GUI2TUI runs live on Linux. macOS is useful for building and testing the
project, but it cannot provide the Linux AT-SPI runtime needed to operate a
GUI application. Before installing, make sure the host has a UTF-8 terminal,
an accessible GUI application, and either an existing Linux desktop session or
the Managed headless dependencies (`xvfb`, `dbus-x11`, `at-spi2-core`,
`x11-utils`, `libglib2.0-bin`, and `python3`).

The stable public binary is v1.0.0. Download the archive matching
`uname -m`:

```bash
ARCH="$(uname -m)"       # x86_64 or aarch64
curl -LO "https://github.com/Chenhzjs/GUI2TUI/releases/download/v1.0.0/gui2tui-1.0.0-linux-${ARCH}.tar.gz"
tar -xzf "gui2tui-1.0.0-linux-${ARCH}.tar.gz"
cd "gui2tui-1.0.0-linux-${ARCH}"
./install-user.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
gui2tui --version
```

Verify a downloaded archive with the release `SHA256SUMS` before installing.
The installer is unprivileged and deliberately refuses to overwrite existing
targets. The v1.1 source tree and its `v1.1.0-rc.1` build are qualification
artifacts rather than a separate public download; build from source only when
developing or validating the project. For replacement or custom prefixes, see
[Getting started](docs/getting-started.md).

### Recommended first run: Managed headless mode

This is the most repeatable path on a Linux host. It creates a private Xvfb,
D-Bus and AT-SPI session. The example uses Mousepad; replace it with an
installed GUI application that exposes public Accessibility semantics.

```bash
gui2tui setup persistent
gui2tui --session managed doctor
gui2tui --session managed app add mousepad
gui2tui --session managed launch mousepad
gui2tui --session managed
```

`doctor` must finish without a `FAIL` result before opening the TUI. If the
application is already running in the same session, omit `app add` and select
it from the application list. GUI2TUI cannot create an Accessibility tree for
an application that exposes none. Use `gui2tui setup status`, `restart` and
`stop` to manage the session.

On a normal Linux desktop, use the inherited session instead:

```bash
gui2tui --session desktop doctor
gui2tui --session desktop
```

If startup fails, run `gui2tui --session managed doctor` again and follow its
first `FAIL` instruction. Missing Managed dependencies are reported with the
corresponding Ubuntu/Debian `apt install` command. Docker, SSH and Wayland
qualification paths are documented in [Deployment](docs/deployment.md).

### Use the TUI

Once an application is already running in the selected session, its name can
be passed directly:

```bash
gui2tui Mousepad
gui2tui Firefox
```

The name may be an exact name or an unambiguous substring. If more than one
application matches, GUI2TUI reports the ambiguity and the selector remains
available through plain `gui2tui`. `--app NAME` remains available for scripts
and explicit command-line use.

The selector accepts arrows, Enter and `/` for filtering. In a scene, use
Tab/Shift-Tab and the arrow keys to move through available semantic controls;
Enter invokes the currently advertised operation. `F1` or `?` opens contextual
help, `F3` searches public actions, `r` refreshes the main view (`F5` retries a disconnected application), `:` opens scoped commands,
and `q` or Ctrl-C exits while restoring the terminal.

For the complete first-use path, external text configuration, Docker and SSH
examples, read [Getting started](docs/getting-started.md) and
[Deployment](docs/deployment.md).

## Real demonstrations

These repository assets are real accessibility-backed GUI/TUI captures, not
mockups:

### Semantic action with authoritative result

The original GTK fixture and the terminal show the same public semantic action
and resulting state:

For the current F3 interface and real Firefox/Mousepad tasks, watch the
[public operations videos](docs/demo/public-operations/README.md).

![Real GUI and TUI action confirmation](docs/assets/readme/action-confirmed.png)

### Reader and content search (read-only)

An exposed document is reorganized as a terminal Reader; search operates on
the available semantic content. This is intentionally a read-only frame: the
checkbox on the left remains `idle` because searching does not mutate the GUI.
The corresponding GUI-to-TUI state-changing operation is shown above.

![Real GUI2TUI Reader and semantic search; the source GUI remains unchanged](docs/assets/readme/reader-search.png)

### End-to-end semantic scene

This capture shows the original GUI beside the terminal-native scene, including
an exposed action and status readback:

![Real GUI2TUI semantic scene](docs/assets/readme/hero.png)

For the reproducible GTK fixture walkthrough, recording method and result
files, see [demo assets](docs/demo/README.md). The older Value and external-text
recordings remain useful demonstrations of the same v1.0 safety contracts;
their historical provenance is retained under [docs/demo/v0.3](docs/demo/v0.3/README.md):
[short walkthrough](docs/demo/v0.3/hero-v0.3.mp4) and
[full safety/refusal walkthrough](docs/demo/v0.3/demo-v0.3.mp4).

## Supported environments

GUI2TUI v1.1 is Headless-first. The qualified support contract includes:

- Managed Xvfb sessions;
- the recorded unprivileged Docker/OCI interactive-PTY topology;
- same-host SSH into a Managed session;
- the recorded Weston headless Native Wayland/XWayland topology, with explicit
  limitations;
- native Linux aarch64 package and live evidence;
- semantic operations on the recorded headless Wayland/XWayland topology;
- Raw Enter only where the qualified X11 NativeInputBackend can activate and
  verify the exact target window and Accessibility focus.

Linux x86_64 packages are available and ABI/install/runtime-smoke qualified
through amd64 emulation on an arm64 host. Native x86_64 hardware and a full
native environment matrix are not qualified. Linux native VT, ordinary local
desktop sessions, Wayland over SSH, static capture and cross-host Remote
Companion are not v1.1 support claims.

See the [v1.1 support matrix](docs/validation/v1.1/final-support-matrix.md)
for exact topology, architecture and capability boundaries. The v1.0 matrix
remains historical release evidence.

## Safety and correctness

GUI2TUI acts only on currently exposed public Accessibility semantics. A
successful backend method or event does not by itself prove success; writes
require fresh authoritative application readback. Stale targets, replaced
applications and invalid scopes are rejected. PasswordText is never exported,
external text handlers receive only private GUI2TUI-owned candidates, and
unsupported or incomplete semantics degrade to read-only or an explicit refusal.

## Limitations

Accessibility quality is application-defined. Not every control exposes a
safe operation, and some Qt transient choice shapes do not expose reliable
selected/current truth. Rich-text fidelity, arbitrary drag/drop, exhaustive
virtualization, pointer-only interactions and private application semantics
are outside the v1.1 task contract. Native Raw Enter is currently X11/XTEST-only;
native text fallback is limited to its documented character set; mouse
injection, coordinate clicking, arbitrary key scripting and uinput remain
unsupported. Long documents and virtualized content may remain partial. See
[current limitations](docs/limitations.md) for safe recovery guidance.

## Documentation

- [Current source status, changes and capabilities (2026-10-20)](docs/current-status-20261020.md)
- [Development Agent protocol](docs/agent-protocol.md)
- [Getting started](docs/getting-started.md)
- [Deployment and supported topologies](docs/deployment.md)
- [Limitations and support boundaries](docs/limitations.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Architecture](docs/architecture.md)
- [Engineering guide](docs/project-guide.md)
- [v1.1 release notes](docs/release-notes-v1.1.0.md)
- [v1.1 support matrix](docs/validation/v1.1/final-support-matrix.md)
- [v1.0 release notes](docs/release-notes-v1.0.0.md)
- [Release verification](docs/validation/v1.0/final-release/HANDOFF.md)
- [Development and validation history](docs/history.md)

## Development

Builds and tests require Rust 1.88 or newer. Live AT-SPI operation requires
Linux; macOS can build and run non-live checks.

```bash
cargo build --locked
cargo test --all-targets --locked
```

GUI2TUI reached v1.0 through internal architecture and validation milestones.
Historical evidence remains under `docs/validation/` and the planning
documents; it is provenance, not additional user setup.

## License

GUI2TUI is available under either:

- [Apache License 2.0](LICENSE-APACHE), or
- [MIT License](LICENSE-MIT),

at your option.
