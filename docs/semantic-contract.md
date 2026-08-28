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

## Atomic plain-text editing

`InteractionCapability::EditText` is present only when all of the following are true:

- the semantic role is `TextInput`;
- `TextInputKind` is `Plain`;
- AT-SPI advertises both `Text` and `EditableText`;
- the node has the AT-SPI `editable` state.

Enter on such a focused input begins a local `EditSession`. The session loads the complete value
using `Text.CharacterCount` and `Text.GetText(0, count)`, so the inspector's 256-character display
limit can never truncate an editing source. Enter commits one atomic
`EditableText.SetTextContents`; Escape discards the local buffer. The semantic cache is updated
only after a GUI read-back, normally prompted by `TextChanged`. A missing event falls back to one
node refresh, never a full application snapshot.

The 2026-08-28 Ubuntu live test confirmed this contract for GTK4 and Qt6. Chrome 152 exposed
`Text` but not `EditableText`, so its plain HTML inputs remain read-only; no keyboard injection is
used. Firefox 154 exposed `EditableText` on the plain HTML input and accepted the D-Bus method
call, but the authoritative GUI read-back remained unchanged and no target `TextChanged` arrived.
That result is treated as an application-normalized/rejected write, not success. In other words,
the cross-browser contract depends on observed interface capability plus GUI confirmation, never
on the proxy method's boolean alone. Firefox observations are recorded in the compatibility
matrix.

Password inputs never receive `EditText`, and the backend independently rejects password roles
before creating either a Text or EditableText edit proxy. External target changes mark an edit
conflict, and locator replacement rejects/cancels commit rather than writing through an old or
reconciled object identity.

## Interaction capability

`InteractionCapability` is derived from the semantic role, advertised node actions, parent
capabilities, and the node's relationship to its parent:

- `None`: focus may still be allowed, but Enter/Space and mouse activation report `No compatible semantic action for "…"`.
- `Activate`: a compatible activation action is available.
- `Toggle`: a compatible toggle action is available.
- `Select`: either a compatible list-item action exists or its direct parent can select children.
- `OpenMenu`: the menu item advertises a compatible show-menu action.
- `EditText`: a plain TextInput satisfies the explicit interface/state contract above.

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
- Text editing is atomic single-line replacement only. Remote caret/selection synchronization,
  multiline/rich text, IME-specific handling, and clipboard operations are not implemented. The
  local cursor is Unicode-scalar-safe but not grapheme-cluster-aware, so a combining sequence or
  multi-code-point emoji may take more than one edit step. Tab is blocked until the session is
  explicitly committed or cancelled.
- `manages-descendants` and ActiveDescendant events are retained, but virtualized collection
  traversal is LIMITED SUPPORT.
