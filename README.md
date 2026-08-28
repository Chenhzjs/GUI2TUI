# GUI2TUI

GUI2TUI explores **GUI semantics → terminal-native semantics**. It is not a framebuffer-to-ASCII-art converter.

The long-term goal is to make GTK, Qt, Chromium/Electron, and similar applications operable from a terminal or SSH session by translating accessibility objects such as buttons, text inputs, menus, lists, trees, and tables into native terminal controls.

This repository contains the validated Phase 0 inspector and a **Phase 1 interactive semantic TUI prototype**:

```text
GUI application
      ↓
    AT-SPI
      ↓
Accessibility tree
      ↓
Semantic snapshot
      ↓
Terminal-native view model
      ↓
Ratatui keyboard/mouse interaction
      ↓
AT-SPI action → original GUI
```

It does not reconstruct GUI pixels or map GUI screen coordinates into terminal coordinates. It also does not contain raster capture, a Wayland compositor, or SSH session plumbing.

## Phase 1 prototype

Run the semantic TUI against one accessible application:

```bash
gui2tui --app gui2tui-live-fixture
```

The first prototype provides:

- terminal-native representations for labels, text, buttons, toggle buttons, checkboxes, read-only text inputs, lists, and list items;
- visible keyboard focus with Tab and Shift-Tab wrapping;
- Enter/Space action dispatch through the node's advertised semantic actions;
- terminal-rectangle mouse hit testing for buttons and checkboxes, plus focusable text inputs/list items;
- arrow, PageUp/PageDown, and mouse-wheel scrolling with focus auto-scroll;
- manual `r` refresh and automatic snapshot refresh after successful actions;
- non-fatal stale-object/application-gone status messages; and
- password redaction before data reaches the TUI renderer.

Text input editing and AT-SPI event subscriptions are deliberately not implemented.

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
src/semantic/              SemanticNode + BackendLocator + RuntimeNodeId
           ├───────────────────────────┐
           ▼                           ▼
src/inspect.rs             src/tui/view_model.rs
Inspector formatter                    │
                                       ▼
                             focus / action / hit test
                                       │
                                       ▼
                             Ratatui renderer + input
```

Neither frontend holds AT-SPI proxies. `BackendLocator` retains the ephemeral D-Bus identity needed to reach the original GUI object; compact `RuntimeNodeId` values are regenerated for each snapshot and are used only for focus, widget identity, and terminal hit testing.

## Build

Install stable Rust 1.88 or newer, then run:

```bash
cargo build
cargo test
cargo install --path .
```

The AT-SPI implementation is pure Rust and uses D-Bus through `zbus`; it does not link to `libatspi`.

## Usage

### Interactive TUI

The Phase 1 prototype currently requires `--app`:

```bash
gui2tui --app gui2tui-live-fixture
gui2tui --app gtk4-demo --max-depth 20 --max-nodes 2000
```

Keys:

```text
Tab / Shift-Tab  focus next / previous
Enter / Space    activate or toggle the focused control
↑ / ↓            scroll one line
PageUp/PageDown  scroll one page
r                 refresh the semantic snapshot
q / Esc           quit
Mouse wheel       scroll
Left click        focus; buttons/toggles/checkboxes also activate
```

Text inputs are display/focus-only. Editing is not implemented.

### Inspector

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

The semantic IR uses two identities:

- `BackendLocator`: the versioned locator encoded as `atspi1_...`; it contains the AT-SPI unique bus name and object path and can relocate a live GUI object.
- `RuntimeNodeId`: a compact `u64` allocated while building one snapshot; it is used by focus, renderer state, and mouse hit testing.

The backend locator is a URL-safe Base64 encoding of:

```text
AT-SPI unique D-Bus bus name NUL D-Bus object path
```

The locator is stable across separate processes while the original object remains alive, but does **not** survive application restarts. `RuntimeNodeId` is intentionally snapshot-local and may be reassigned after every refresh. The locator encoding is reversible, not secret or cryptographic.

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
gui2tui --app gui2tui-live-fixture
```

The fixture intentionally contains a normal entry with value `alice`, a password entry with a sentinel secret that must never appear in inspector or TUI output, a checkbox, and a button that changes both a status label and checkbox state. Tab to `Activate safely` and press Enter, or click its terminal hit region; the refreshed TUI should show a checked checkbox and `Status: activated`.

## Current limitations

- This is a synchronous snapshot traversal, not an event-driven cache. The UI can change while it is being read.
- Backend locators remain valid only for the lifetime of their backing accessible object; runtime IDs last only for one snapshot.
- Role mapping is intentionally conservative; known but unmapped AT-SPI roles become `Unknown(original-role)`.
- Text values are read only for ordinary entry-like roles, capped at 256 characters. Password text is intentionally not read.
- Numeric values are read for sliders, progress/level bars, and spin buttons.
- `--activate` is a heuristic convenience; use an explicit action index when correctness matters.
- The basic GTK4 tree/action path and the bundled fixture have been exercised manually on Linux. Broad GTK3, Qt, Firefox, Chromium, and Electron compatibility is still unverified.
- TextInput editing, selection, cursor synchronization, IME, and clipboard integration are not implemented.
- There is no AT-SPI event cache, runtime identity reconciliation, raster fallback, compositor, or SSH integration yet.
- The first version requires `gui2tui --app NAME`; an in-TUI application selector is not implemented.

## Roadmap

```text
Phase 0  AT-SPI inspector                         ✓ validated
Phase 1  Interactive semantic TUI prototype       ← current
Phase 2  Event cache + richer semantic controls
Phase 3  GTK + Qt compatibility
Phase 4  Chromium / Electron
Phase 5  Raster fallback
Phase 6  Wayland compositor / SSH integration
```

Qt and browser compatibility remain separate coverage gates for Phases 3 and 4. The Phase 1 renderer remains semantic-first and does not consume AT-SPI geometry for layout.

## License

GUI2TUI is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
