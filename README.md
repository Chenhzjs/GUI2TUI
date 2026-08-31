# GUI2TUI

GUI2TUI explores **GUI semantics → terminal-native semantics**. It is not a framebuffer-to-ASCII-art converter.

The long-term goal is to make GTK, Qt, Chromium/Electron, and similar applications operable from a terminal or SSH session by translating accessibility objects such as buttons, text inputs, menus, lists, trees, and tables into native terminal controls.

This repository contains the validated inspector, cross-toolkit semantic TUI, container operations,
event-driven cache, bulk semantic bootstrap, atomic plain-text editing, and the application-agnostic
semantic transcompiler:

```text
GUI application
      ↓
    AT-SPI
      ↓
Accessibility tree
      ↓
AT-SPI Cache bulk bootstrap (recursive-walk fallback)
      ↓
Arena-backed live semantic cache
      ↓
Targeted relation enrichment + semantic graph
      ↓
Region analysis + interaction scopes ───────┐
Semantic content analysis + bounded cache ──┤
      ↓
Hierarchical commands + presentation/Reader planning
      ↓
Terminal-native TuiScene
      ↓
Ratatui keyboard/mouse interaction
      ↓
AT-SPI action → original GUI
```

It does not reconstruct GUI pixels or map GUI screen coordinates into terminal coordinates.
The semantic renderer does not use screenshots. Phase 3H adds a separate, explicit single-frame
Image acquisition provider; it is not a Wayland compositor or SSH session framework.

When a semantic task genuinely needs an original image, document, video, or
portable model, Phase 3G uses a reference-first, user-authorized local handoff.
See [External modality handoff](docs/modality-handoff.md). It is not a remote
desktop, file-sync service, or continuous media stream.

For an Image with no resource reference, an explicit request can produce an honestly labelled
**RenderedSnapshot**, subject to strict coordinate checks. A viewer is optional: headless users
can materialize a bounded, hashed, expiring file on the GUI2TUI host. See
[Static acquisition and deployment topology](docs/static-acquisition.md) and
[live GUI → TUI examples](docs/gui-to-tui-examples.md).

## Interactive prototype

Run the semantic TUI against one accessible application, or omit `--app` to use the application selector:

```bash
gui2tui --app gui2tui-live-fixture
gui2tui
```

The first prototype provides:

- terminal-native representations for labels, text, buttons, toggle buttons, checkboxes, plain/password text inputs, lists, and list items;
- visible keyboard focus with Tab and Shift-Tab wrapping;
- Enter/Space dispatch through semantic Activate, Toggle, Select, and OpenMenu operations;
- terminal-rectangle mouse hit testing for buttons and checkboxes, plus focusable text inputs/list items;
- arrow, PageUp/PageDown, and mouse-wheel scrolling with focus auto-scroll;
- event-driven incremental node/subtree refresh, with manual `r` full-snapshot fallback;
- fast bulk bootstrap through AT-SPI Cache when its inventory is complete, with automatic walk fallback;
- non-fatal stale-object/application-gone status messages;
- password redaction before data reaches the TUI renderer;
- role-aware action resolution with no implicit first-action fallback; and
- list selection through either a node action or its parent's AT-SPI Selection interface;
- terminal-native Menu/MenuItem presentation with distinct OpenMenu and leaf Activate intents;
- explicit `(read-only)` presentation for focusable controls without a compatible advertised action; and
- local single-line plain-text edit sessions committed atomically through AT-SPI EditableText,
  with authoritative GUI read-back and conflict/replacement protection; and
- generic reconstruction of labeled fields, forms, command sets, selections, status/content
  summaries, and semantically sparse graphical regions without application-name branches.
- targeted AT-SPI relations, modal/popup interaction scopes, and a true hierarchical command
  browser/search model with scope filtering before explainable ranking.
- a progressive semantic Reader, heading Outline, and indexed/loaded content Search for Web,
  Writer, and generic read-only multiline Text objects; and
- bounded partial-collection models that never equate realized children with a logical total.

The transcompiled presentation is the default. `--presentation legacy` retains the former direct
widget projection as a diagnostic comparison. Press `:` outside text-edit mode to open the command
palette generated from safe semantic commands.

Password, multiline/rich-text editing, remote caret/selection, IME, and clipboard editing are
deliberately not implemented. Read-only rich content is supported by the Reader. The raw normalized
stream can be inspected with
`gui2tui-inspect --watch-events --app NAME`.

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

Runtime/application lifecycle, detach/resume, bounded event recovery and crash-owned artifacts are
documented in [docs/runtime-recovery.md](docs/runtime-recovery.md). These ownership layers do not
alter the frozen semantic/content IR.

```text
src/backend/atspi.rs       AT-SPI traversal, bulk enrichment, events, and operations
src/backend/bootstrap.rs   Cache/walk strategy + bulk tree reconstruction
src/backend/protocol_compat.rs  modern/legacy wire normalization
           │
           ▼
src/events.rs              normalized events + dirty-scope coalescing
           │
           ▼
src/semantic/cache.rs      canonical arena/hash maps + identity reconciliation
           │
           ▼
src/semantic/              SemanticNode + BackendLocator + RuntimeNodeId
           ├──────────────────────────────────────────────┐
           ▼                                              ▼
src/inspect.rs                         src/transcompile/
Inspector formatter                    SemanticRegion analysis
                                                   │
                                                   ▼
                                        PresentationStrategy
                                                   │
                                                   ▼
                                               TuiScene
                                                   │
                                                   ▼
                                        Ratatui renderer + input
                                       │
                                       ▼
                         UiIntent → SemanticOperation
                                       │
                         SelectionStrategy / action resolver
                                       ▼
                             BackendOperation dispatch
                                       └──→ GUI event → incremental cache update
```

Neither frontend holds AT-SPI proxies. `BackendLocator` retains the ephemeral D-Bus identity needed
to reach the original GUI object; compact `RuntimeNodeId` values remain stable for the current
`SemanticCache` session and are used for focus, widget identity, and terminal hit testing.

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

Open a named application directly or select one interactively:

```bash
gui2tui
gui2tui --app gui2tui-live-fixture
gui2tui --app gtk4-demo --max-depth 20 --max-nodes 2000
gui2tui --app firefox --bootstrap auto
gui2tui --app assistant --presentation transcompiled
gui2tui --app assistant --presentation legacy
```

Keys:

```text
Tab / Shift-Tab  focus next / previous
Enter            activate/select/open, or begin/commit plain-text editing
Space            activate, toggle, or select a non-text control
↑ / ↓            scroll one line
PageUp/PageDown  scroll one page
r                 refresh the semantic snapshot
:                 open semantic command palette
F2                in command palette, toggle current-scope/all-command search
q / Esc           quit
Mouse wheel       scroll
Left click        focus; buttons/toggles/checkboxes also activate
```

In an editable plain single-line TextInput, Enter starts a local session. Character keys,
Left/Right, Home/End, Backspace, and Delete edit the local buffer; Enter commits and Esc cancels.
Tab is blocked until the edit is committed or cancelled. Password and unsupported inputs remain
read-only.

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
gui2tui-inspect --watch-events --app firefox
gui2tui-inspect --app firefox --probe-cache
gui2tui-inspect --app firefox --bootstrap cache
gui2tui-inspect --app firefox --bootstrap walk
gui2tui-inspect --app firefox --probe-collection
gui2tui-inspect --app firefox --dump-regions
gui2tui-inspect --app firefox --dump-scene
gui2tui-inspect --app firefox --dump-relations
gui2tui-inspect --app firefox --relations NODE_ID
gui2tui-inspect --app firefox --dump-scopes
gui2tui-inspect --app firefox --dump-commands
gui2tui-inspect --app firefox --command-query about
gui2tui-inspect --app firefox --audit-scene-reachability
gui2tui-inspect --app firefox --dump-content
gui2tui-inspect --app firefox --dump-outline
gui2tui-inspect --app firefox --probe-document
gui2tui-inspect --app firefox --dump-virtual-collections
gui2tui-inspect --app firefox --audit-content-reachability
```

`--dump-regions` exposes generic rewrite decisions, their confidence, source runtime IDs, and
coverage metrics. `--dump-scene` shows the renderer-facing presentation primitives and reports
region-analysis/scene-compilation timing on stderr.

The cross-family live measurements and rule limits are recorded in
[docs/transcompiler.md](docs/transcompiler.md); design and licensing boundaries are in
[docs/design-principles.md](docs/design-principles.md), [docs/architecture.md](docs/architecture.md),
and [docs/term-everything-study.md](docs/term-everything-study.md). Relation, scope, and command
planning observations are in [docs/relations.md](docs/relations.md). Content architecture and live
measurements are in [docs/content-navigation.md](docs/content-navigation.md); representative actual
GUI-to-TUI frames are in [docs/gui-to-tui-examples.md](docs/gui-to-tui-examples.md). Cancellable
full search, runtime Text quarantine, virtual collection navigation, tables, and content-scope
policy are documented in [docs/progressive-content.md](docs/progressive-content.md).

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
gui2tui-inspect --select-child 'PARENT_NODE_ID' --child-index 1
```

`--action-name` first uses an exact action-name match, then an ASCII case-insensitive match. Duplicate matches are rejected. `--activate` accepts only advertised actions named `click`, `press`, or `activate`, in that order. It reports an error and the available actions when none is compatible; it never invokes an arbitrary first action.

`--activate` is a convenience heuristic. For deterministic automation use `--action` or `--action-name`; both operate directly on AT-SPI's advertised actions and do not infer behavior from the semantic role.

`--select-child` is an explicit container-level diagnostic API. It invokes the parent's AT-SPI
Selection interface for exactly one zero-based direct-child index. Normal TUI list selection uses
the same backend operation only after semantic relationship/capability resolution.

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
- `RuntimeNodeId`: a compact `u64` owned by one live semantic-cache session; it is used by focus,
  renderer state, and mouse hit testing.

The backend locator is a URL-safe Base64 encoding of:

```text
AT-SPI unique D-Bus bus name NUL D-Bus object path
```

The locator is stable across separate processes while the original object remains alive, but does
**not** survive application restarts. Exact locators retain their runtime ID during incremental
updates. A changed locator is reconciled only by an unambiguous sibling-local fingerprint; an
ambiguous replacement receives a new ID. Application restart always starts a new identity session.
The locator encoding is reversible, not secret or cryptographic.

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

The fixture intentionally contains a normal entry, a password sentinel that must never appear in
output, a checkbox, a safe activation button, and a selectable list. Actions update the original
GUI; emitted events incrementally update the terminal view.

A corresponding Qt6 fixture is provided for cross-toolkit validation (Ubuntu package: `python3-pyqt6`):

```bash
QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 python3 tests/fixtures/qt6_live_fixture.py
gui2tui-inspect --app gui2tui-qt-fixture --verbose
gui2tui --app gui2tui-qt-fixture
```

The validated headless session set `org.a11y.Status.IsEnabled` and `ScreenReaderEnabled` before restarting Qt. See [the compatibility matrix](docs/compatibility.md) and [semantic contract](docs/semantic-contract.md) for the exact observed GTK4/Qt6 data.

The repeatable Linux smoke harness builds the project, starts a private D-Bus session, Xvfb,
AT-SPI, and both fixtures, then checks discovery, password redaction, and safe button actions:

```bash
./scripts/live-test-linux.sh
CACHE_BOOTSTRAP_TEST=1 ./scripts/live-test-linux.sh
```

It requires the GTK4/PyGObject and PyQt6 fixture dependencies and is intentionally not part of
the default CI. Browser probing is also optional because it installs a large external package;
the exact Chrome 152 observations are recorded in [browser-probe.md](docs/browser-probe.md).
The raw-to-normalized event contract and measured incremental results are in
[events.md](docs/events.md). Bulk/walk behavior and measured bootstrap speed are in
[bootstrap.md](docs/bootstrap.md).

## Current limitations

- Initial loading uses Cache.GetItems when the returned inventory is usable. A missing, empty,
  malformed, or detectably incomplete cache falls back to the slower recursive walk.
- Backend locators remain valid only for their accessible object's lifetime. Conservative runtime
  reconciliation intentionally drops identity when sibling fingerprints are ambiguous.
- Role mapping is intentionally conservative; known but unmapped AT-SPI roles become `Unknown(original-role)`.
- Text values are read only for ordinary entry-like roles, capped at 256 characters. Password text is intentionally not read.
- Numeric values are read for sliders, progress/level bars, and spin buttons.
- `--activate` is a conservative heuristic convenience; use explicit `--action-name` or `--action --index` when correctness matters.
- GTK4 and Qt6 fixture tree/action/TUI paths have been exercised on Linux/Xvfb. Chrome 152 tree,
  password, explicit action, scaling, and locator churn were probed. Firefox 154 discovery, tree,
  cache/event behavior, password safety, and EditableText rejection/read-back were also probed;
  Electron and broad browser interaction remain unverified.
- Plain single-line TextInput supports explicit local edit sessions and atomic AT-SPI
  `EditableText.SetTextContents`. Remote caret/selection synchronization, IME, clipboard,
  multiline, rich-text, and password editing are not implemented.
- Partial virtual collections are modeled from realized AT-SPI children, but arbitrary logical
  paging and reliable ActiveDescendant traversal remain limited. There is no raster fallback,
  compositor, or SSH integration yet.
- List selection currently supports a compatible item action or direct-child selection through a
  parent Selection interface. Multi-selection/deselection is not implemented.
- Menu OpenMenu/leaf Activate are separated, but hierarchy navigation, Escape/back, and focus
  trapping are not implemented.
- Some Chrome 152 web controls advertise only anonymous AT-SPI actions. Explicit inspector index
  invocation works on the test fixture; the semantic TUI intentionally refuses to guess index 0.

## Roadmap

```text
Phase 0  AT-SPI inspector                         ✓ validated
Phase 1  Interactive semantic TUI prototype       ✓ validated
Phase 2A GTK + Qt semantic contract               ✓ validated
Phase 2B Container selection/menu + browser probe ✓ validated
Phase 2C Event stream + incremental semantic cache ✓ validated
Phase 3A Fast bulk bootstrap                    ✓ validated
Phase 3B Atomic EditableText                    ✓ validated
Phase 3C Semantic UI transcompiler              ✓ validated
Phase 3D Relational/contextual task planning    ✓ validated
Phase 3E Semantic content navigation            ✓ validated
Phase 3F Progressive content operations         ✓ validated
Phase 4  Chromium / Electron
Phase 5  Raster fallback
Phase 6  Wayland compositor / SSH integration
```

The renderer remains semantic-first and does not consume AT-SPI geometry for layout. Full snapshot
refresh remains a correctness escape hatch when an event cannot be resolved or a cache invariant
fails.

## License

GUI2TUI is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
