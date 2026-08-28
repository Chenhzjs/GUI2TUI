# GUI2TUI cross-toolkit semantic contract

This document records behavior observed in the Ubuntu 24.04 arm64 Xvfb test session on 2026-08-28. It is a compatibility record, not a list of assumed toolkit behavior.

## Identity and live cache

- `BackendLocator` is the encoded AT-SPI unique bus name plus object path. It can relocate an object only while that object and application bus name remain alive.
- `RuntimeNodeId(u64)` is unique and stable inside one `SemanticCache` session.
- Exact `BackendLocator` matches preserve runtime identity during node/subtree refresh.
- Locator churn reconciles only a unique sibling-local `(role, name, TextInputKind)` fingerprint.
  Ambiguous replacements receive new IDs. Application restart always creates a new session.

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
| List | GTK4 demo: role `list`, `Selection` interface | Qt6: role `list`, `Table` interface, no Action interface | group heading; container capability is retained outside the renderer |
| ListItem | GTK4 demo: role `list item`, action `listitem.scroll-to`; selected through the parent | Qt6: role `list item`, action `Toggle`; Toggle set `selected` | focusable/selectable; selected `*` and keyboard focus `>` are independent |
| ComboBox | NOT TESTED | NOT TESTED | mapped from `combo box`; focusable read-only `[ label ▼ ]` unless a future verified operation exists |
| MenuBar | not tested in bundled GTK fixture | role `menu bar`, Action interface, no advertised action | `Menu:` heading |
| Menu | not tested in bundled GTK fixture | popup role `popup menu` mapped to Menu | terminal-native menu heading |
| MenuItem | not tested in bundled GTK fixture | top item `ShowMenu`; leaf `Press` changed the fixture label to `Status: menu activated` | `ShowMenu` becomes OpenMenu; leaf `Press` becomes Activate |

Action-name comparisons in the TUI are ASCII case-insensitive because GTK and Qt expose different capitalization. The selected action must still be in the role-specific compatibility list.

## Interaction capability

`InteractionCapability` is derived from the semantic role, advertised node actions, parent
capabilities, and the node's relationship to its parent:

- `None`: focus may still be allowed, but Enter/Space and mouse activation report `No compatible semantic action for "…"`.
- `Activate`: a compatible activation action is available.
- `Toggle`: a compatible toggle action is available.
- `Select`: either a compatible list-item action exists or its direct parent can select children.
- `OpenMenu`: the menu item advertises a compatible show-menu action.

The TUI never invokes `actions[0]` as a fallback and never mutates checked/selected state locally. Explicit inspector commands `--action-name` and `--action --index` remain available as low-level APIs.

## Container selection contract

Selection establishes an explicit boundary between user meaning and backend mechanics:

```text
UiIntent::Select
        ↓
SemanticOperation::SelectNode(RuntimeNodeId)
        ↓
SelectionStrategy
   ┌────┴──────────────────────────┐
   │                               │
node advertises compatible     parent advertises
action                         SelectChildren
   │                               │
InvokeAction(locator, name)    SelectChild(parent locator,
                              original direct-child index)
```

The Ubuntu live tests exercised both branches without inspecting toolkit names:

- GTK4 demo: a ListItem's own `listitem.scroll-to` action was rejected as a selection action;
  the parent List's AT-SPI `Selection.select_child(index)` selected the item.
- Qt6 fixture: the ListItem's own `Toggle` action selected it. The semantic operation remained
  `Select`; the toolkit action name did not turn the item into a toggle control.

`SemanticNode.index_in_parent` preserves the index from the backend's original direct-child
array. `TuiViewModel` indexes `RuntimeNodeId` to parent ID, child index, backend locator, and
container capabilities. The renderer does not know about AT-SPI Selection.

## Menu intent contract

- `MenuItem + ShowMenu` resolves only as `OpenMenu`.
- `MenuItem + Press` resolves only as `Activate`.
- `ShowMenu` is never accepted as an Activate fallback, and no first action is invoked.
- After OpenMenu or Activate, GUI2TUI primarily consumes GUI events and refreshes their dirty
  node/subtree. Popup visibility and contents always come from the application; timeout or
  inconsistency falls back to a full snapshot.

## Event contract

Raw AT-SPI event types normalize without toolkit names. State/property/text changes dirty one node;
selection/children/active-descendant changes dirty a container subtree; lifecycle and unresolved
events use the application fallback. A 40 ms window coalesces each burst. The single cache owner is
the only tree writer. See [events.md](events.md) for actual GTK, Qt, and Chrome sequences.

## Known semantic gaps

- Multi-selection, deselection, Selection child enumeration, and SelectionChanged events are not implemented.
- GTK4 demo list-item actions such as `listitem.scroll-to` and nested expansion actions remain intentionally excluded from generic activation.
- Qt6 `QListWidgetItem.Toggle` is accepted only for the ListItem Select operation; it is not a global alias.
- Menu keyboard hierarchy, Escape/back behavior, and popup focus trapping are not implemented; the current model opens the real menu and refreshes its semantic snapshot.
- Chrome 152 on Linux arm64 returned two completely unnamed AT-SPI actions for web controls. Explicit `--action --index 0` worked on the controlled fixture, but the semantic resolver correctly refuses an anonymous action.
- Text inputs are presentation/focus-only. `EditableText` operations are not implemented.
- `manages-descendants` and ActiveDescendant events are retained, but virtualized collection
  traversal is LIMITED SUPPORT.
