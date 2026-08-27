# GUI2TUI

GUI2TUI explores **GUI semantics → terminal-native semantics**. It is not a framebuffer-to-ASCII-art converter.

The long-term goal is to make GTK, Qt, Chromium/Electron, and similar applications operable from a terminal or SSH session by translating accessibility objects such as buttons, text inputs, menus, lists, trees, and tables into native terminal controls.

This repository currently contains only **Phase 0**: `gui2tui-inspect`, a command-line probe for validating this path:

```text
GUI application
      ↓
    AT-SPI
      ↓
Accessibility tree
      ↓
Semantic UI model
      ↓
CLI tree output / Action invocation
```

It does not contain a Ratatui frontend, raster capture, a Wayland compositor, or SSH session plumbing.

## Phase 0 capabilities

- Enumerate applications exposed by the current AT-SPI desktop.
- Select an application by one-based list index, exact name, or an unambiguous case-insensitive substring.
- Recursively convert its accessibility tree into a backend-independent semantic tree.
- Print semantic roles, names, salient states, text/numeric values, and actions.
- Print AT-SPI role, full states, bus name, object path, interfaces, and screen geometry in verbose mode.
- List and invoke AT-SPI actions by node ID, index, or exposed action name.
- Bound every remote operation with a configurable timeout and visibly mark incomplete trees.
- Report missing desktop/session bus, stale objects, unsupported actions, bad action indices, and D-Bus failures without panicking.

## Architecture

```text
src/backend/atspi.rs       AT-SPI connection, traversal, and action calls
           │
           ▼
src/semantic/node.rs       SemanticNode / Role / State / Action / NodeId
           │
           ▼
src/inspect.rs             Human-readable tree formatter
           │
           ▼
src/main.rs                clap CLI and user-facing diagnostics
```

The formatter does not hold AT-SPI proxies. Backend-specific identity and debug data are confined to `NodeId` and `DebugInfo`, leaving the core semantic fields suitable for a future TUI renderer.

## Build

Install stable Rust 1.87 or newer, then run:

```bash
cargo build
cargo test
cargo install --path .
```

The AT-SPI implementation is pure Rust and uses D-Bus through `zbus`; it does not link to `libatspi`.

## Usage

With no selector, the command behaves like `--list`:

```bash
gui2tui-inspect
gui2tui-inspect --list
```

Example output:

```text
1  gnome-text-editor
2  Firefox
```

Inspect an application:

```bash
gui2tui-inspect --app firefox
gui2tui-inspect --app-id 1
gui2tui-inspect --app firefox --verbose
```

Ordinary tree output prints a copyable node ID on nodes that expose actions:

```text
Application "gedit"
└── Window "Untitled Document"
    ├── Text
    └── Button "Save" actions=[click] id=atspi1_...
```

Inspect and invoke an action:

```bash
gui2tui-inspect --actions 'atspi1_...'
gui2tui-inspect --activate 'atspi1_...'
gui2tui-inspect --action 'atspi1_...' --index 0
gui2tui-inspect --action-name 'atspi1_...' click
```

`--action-name` first uses an exact action-name match, then an ASCII case-insensitive match. Duplicate matches are rejected. `--activate` prefers actions named `press`, `click`, `activate`, or `open`, in that order, and otherwise invokes the first advertised action.

`--activate` is a convenience heuristic. For deterministic automation use `--action` or `--action-name`; both operate directly on AT-SPI's advertised actions and do not infer behavior from the semantic role.

Traversal can be bounded for very large applications:

```bash
gui2tui-inspect --app firefox --max-depth 20 --max-nodes 5000
gui2tui-inspect --app firefox --timeout-ms 5000
```

When a depth/node limit or a per-object timeout prevents complete traversal, the output includes a visible pseudo-node such as:

```text
└── … [tree truncated: max nodes=5000]
```

Set `RUST_LOG=debug` for backend diagnostics. Warnings caused by objects disappearing during traversal are sent to stderr while other live nodes continue to print.

## Node identity

A node ID is a versioned, URL-safe Base64 encoding of:

```text
AT-SPI unique D-Bus bus name NUL D-Bus object path
```

The current prefix is `atspi1_`. This identity is stable across separate `gui2tui-inspect` processes while the original application and accessibility object remain alive. It does **not** survive application restarts, and non-root accessibility objects may be destroyed and recreated whenever the GUI changes. In that case the CLI reports the object as stale and the tree must be inspected again. The encoding is reversible, not secret or cryptographic.

## Environment requirements

The runtime target is a Linux graphical desktop session with:

- a reachable session D-Bus (`DBUS_SESSION_BUS_ADDRESS`),
- an active AT-SPI bus (`org.a11y.Bus`), and
- applications whose toolkit accessibility bridge is enabled.

Useful diagnostics are:

```bash
echo "$XDG_SESSION_TYPE"
echo "$DBUS_SESSION_BUS_ADDRESS"
echo "$DISPLAY"
echo "$WAYLAND_DISPLAY"
```

`DISPLAY` or `WAYLAND_DISPLAY` alone is not sufficient: the command must reach the same user's session D-Bus and AT-SPI registry. A plain SSH login commonly lacks those variables and will produce:

```text
No accessible AT-SPI desktop session found.
```

Run the inspector inside the graphical user's session. Forwarding or importing a desktop session environment has security implications and is outside Phase 0.

## Manual integration test

On a Linux desktop, start one accessible sample application, for example:

```bash
gtk4-demo
```

Then, from a terminal in the same desktop session:

```bash
cargo run --bin gui2tui-inspect -- --list
cargo run --bin gui2tui-inspect -- --app gtk4-demo
cargo run --bin gui2tui-inspect -- --actions 'NODE_ID_FROM_TREE'
cargo run --bin gui2tui-inspect -- --action-name 'NODE_ID_FROM_TREE' click
```

Confirm visually that activating a button or menu item changes the original GUI. Do not use a destructive control for this test.

For repeatable input/password/action checks, a small non-destructive GTK4 fixture is also provided. It requires PyGObject and the GTK4 introspection data (on Ubuntu: `python3-gi gir1.2-gtk-4.0`):

```bash
python3 tests/fixtures/gtk4_live_fixture.py
gui2tui-inspect --app gui2tui-live-fixture --verbose
```

The fixture intentionally contains a normal entry with value `alice`, a password entry with a sentinel secret that must never appear in inspector output, a checkbox, and a button that changes both a status label and checkbox state.

## Current limitations

- This is a synchronous snapshot traversal, not an event-driven cache. The UI can change while it is being read.
- Node IDs remain valid only for the lifetime of their backing accessible object.
- Role mapping is intentionally conservative; known but unmapped AT-SPI roles become `Unknown(original-role)`.
- Text values are read only for ordinary entry-like roles, capped at 256 characters. Password text is intentionally not read.
- Numeric values are read for sliders, progress/level bars, and spin buttons.
- `--activate` is a heuristic convenience; use an explicit action index when correctness matters.
- The basic GTK4 tree/action path and the bundled fixture have been exercised manually on Linux. Broad GTK3, Qt, Firefox, Chromium, and Electron compatibility is still unverified.
- There is no keyboard input, editable-text mutation, event subscription, TUI renderer, raster fallback, compositor, or SSH integration yet.

## Roadmap

```text
Phase 0  AT-SPI inspector                         ← current
Phase 1  Semantic UI IR + Ratatui renderer
Phase 2  Keyboard / mouse → semantic actions
Phase 3  GTK + Qt compatibility
Phase 4  Chromium / Electron
Phase 5  Raster fallback
Phase 6  Wayland compositor / SSH integration
```

Phase 1 can proceed after the GTK Phase 0 path has been validated end to end. Qt and browser compatibility remain separate coverage gates for Phases 3 and 4 and should continue to be tested without blocking the initial semantic TUI prototype.

## License

GUI2TUI is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
