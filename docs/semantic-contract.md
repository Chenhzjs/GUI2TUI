# GUI2TUI cross-toolkit semantic contract

This document records behavior observed in the Ubuntu 24.04 arm64 Xvfb test session on 2026-08-28. It is a compatibility record, not a list of assumed toolkit behavior.

## Identity and snapshots

- `BackendLocator` is the encoded AT-SPI unique bus name plus object path. It can relocate an object only while that object and application bus name remain alive.
- `RuntimeNodeId(u64)` is unique inside one semantic snapshot and is regenerated on refresh.
- Focus recovery after refresh matches the previous `BackendLocator`; cross-refresh runtime identity reconciliation is not implemented.

## Input secrecy

- `TextInputKind::Plain` permits the backend to retain a bounded plain value.
- `TextInputKind::Password` prevents text/value retrieval in the AT-SPI backend. The view model emits only `[password]`.
- AT-SPI state `sensitive` means that a control responds to user input. It remains a normal semantic state and never implies `TextInputKind::Password`.

Both GTK4 `password text` and Qt6 `password text` were observed with the `Text` and `EditableText` interfaces. Their sentinel values were absent from normal inspector, verbose inspector, and TUI output.

## Role and action observations

| Semantic role | GTK4 observation | Qt6 observation | TUI contract |
| --- | --- | --- | --- |
| Button | role `button`, action `Click` | role `button`, actions `Press`, `SetFocus` | `[ label ]`; Activate accepts `click`, `press`, `activate` |
| TextInput (plain) | role `text`, interfaces `Text`, `EditableText`, action `Activate` | role `text`, interfaces `Text`, `EditableText`, action `SetFocus` | focusable read-only value; no activation capability |
| TextInput (password) | role `password text`, value suppressed | role `password text`, value suppressed | focusable, `[password]`, no activation capability |
| CheckBox | role `check box`, no actions in the fixture | role `check box`, actions `Toggle`, `SetFocus` | checked state comes from snapshot; Toggle only when `toggle`, `click`, or `press` is advertised |
| Label | role `label`; GTK exposes text-oriented actions on some labels | role `label`; no advertised actions in fixture | non-focusable text; label actions are ignored by the TUI |
| List | GTK4 demo: role `list`, `Selection` interface, list-level select actions | Qt6: role `list`, `Table` interface, no Action interface | read-only group heading |
| ListItem | GTK4 demo: role `list item`, action `listitem.scroll-to` | Qt6: role `list item`, action `Toggle`; Toggle set `selected` | focusable; accepts `select`, `toggle`, `activate`, `click`; other actions remain unavailable |
| MenuBar | not tested in bundled GTK fixture | role `menu bar`, Action interface, no advertised action | read-only unsupported summary |
| Menu | not tested in bundled GTK fixture | popup role `popup menu` mapped to Menu | read-only unsupported summary |
| MenuItem | not tested in bundled GTK fixture | top item action `ShowMenu`; leaf `Press` changed the fixture label to `Status: menu activated` | currently read-only; full menu navigation is not implemented |

Action-name comparisons in the TUI are ASCII case-insensitive because GTK and Qt expose different capitalization. The selected action must still be in the role-specific compatibility list.

## Interaction capability

`InteractionCapability` is derived from both the semantic role and the actions advertised in the current snapshot:

- `None`: focus may still be allowed, but Enter/Space and mouse activation report `No compatible semantic action for "…"`.
- `Activate`: a compatible activation action is available.
- `Toggle`: a compatible toggle action is available.

The TUI never invokes `actions[0]` as a fallback and never mutates checked/selected state locally. Explicit inspector commands `--action-name` and `--action --index` remain available as low-level APIs.

## Known semantic gaps

- GTK4 list selection is exposed primarily through the `Selection` interface; no Selection backend exists yet.
- GTK4 demo list-item actions such as `listitem.scroll-to` and nested expansion actions are intentionally not treated as generic activation.
- Qt6 `QListWidgetItem.Toggle` selects an item, but this name may represent different toolkit-specific behavior elsewhere and must continue to be role-scoped.
- Qt6 menus expose enough structure for inspection, but popup navigation and menu visibility state need a dedicated TUI model.
- Text inputs are presentation/focus-only. `EditableText` operations are not implemented.
- Event subscriptions and incremental cache mutation are not implemented.
