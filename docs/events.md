# AT-SPI event and incremental-cache contract

This file records events observed on Ubuntu 24.04 arm64, Xvfb/X11, AT-SPI 2.52 on
2026-08-28. The production path contains no toolkit-name branches.

```text
AT-SPI signal
    -> NormalizedEvent
    -> 40 ms burst coalescing
    -> DirtyScope
    -> node/subtree backend read
    -> single-owner SemanticCache mutation
    -> TUI redraw
```

`gui2tui inspect --watch-events --app NAME` prints the normalized, read-only stream for
one application's unique AT-SPI bus name. A newly added object from that bus is retained even
when it is not yet present in the current tree.

## Normalization and dirty scopes

| Raw event | Normalized event | First implementation refresh |
| --- | --- | --- |
| Object `StateChanged` | `NodeStateChanged` | source node |
| Object `PropertyChange` | `NodePropertyChanged` | source node |
| Object `TextChanged` | `TextChanged` (metadata only; no text payload) | source node |
| Object `ChildrenChanged` | `ChildrenChanged` | parent subtree |
| Object `SelectionChanged` | `SelectionChanged` | container subtree |
| Object `ActiveDescendantChanged` | `ActiveDescendantChanged` | container subtree |
| Window create/destroy/close | `WindowCreated` / `WindowDestroyed` | application fallback |
| Cache add/remove | `CacheAdded` / `CacheRemoved` | application unless a same-burst children event gives a structural scope |
| unclassified event | `Unknown` | application fallback |

Repeated scopes are deduplicated. A subtree subsumes the same locator's node scope, and an
application scope subsumes the complete batch. The current implementation does not attempt an
ancestry optimizer across different locators.

Qt 6.4 emitted historical `siiv(so)` property-event bodies whose property kind was
`accessible-name`. `atspi` 0.30 rejected that spelling while constructing its typed wrapper.
The backend therefore consumes the raw zbus message stream, first tries `atspi::Event`, and has
one protocol-level compatibility decoder in `backend/protocol_compat.rs` for this wire body. It
still produces the same toolkit-independent `NodePropertyChanged` value and contains no
toolkit-name branch.

The producer uses a bounded 2,048-event channel by default. A full channel atomically marks one
pending resynchronization and counts dropped messages; it never silently treats the remaining
prefix as authoritative. The cache owner drains that prefix, performs one full bootstrap, and
then resumes incremental processing. A Chrome Add100 live run with capacity 4 dropped 197 events,
performed one resync to 382 nodes, and ended at `full_snapshots=2` without panic or deadlock.

## Recorded event bursts

| Operation | Raw events | Main kinds | Incremental result |
| --- | ---: | --- | --- |
| GTK fixture Activate | 5 | 2 children, 2 state, 1 property | 13 nodes, 31 ms, no full refresh |
| GTK fixture select Beta | 7 | 2 children, 4 state, 1 selection | 9 nodes, 26 ms, no full refresh |
| Qt fixture Activate | 2 | checked state, accessible-name property | 2 nodes, 3 ms, no full refresh |
| Qt checkbox Toggle | 1 | state | 1 node, 8 ms |
| Qt list-item Select | 1 | selected state | 1 node, 2 ms |
| Qt menu leaf Press | 1 | accessible-name property | 1 node, 10 ms |
| Chrome Activate | 9 | cache add/remove, children, state, text | 5 nodes on the 5,158-node fixture, 212 ms |
| Chrome replace unique subtree | 6 | cache add/remove, children, state | 4 nodes, 32 ms; one identity reconciled |
| Chrome replace duplicate subtree | 10 | cache add/remove, children, state | 5 nodes, 31 ms; no identities guessed |
| Chrome add 100 | 202 | 100 cache add, 100 children, 2 state | 103 nodes, 161 ms |
| Chrome remove 100 | 102 | 100 children, 2 state | 3 nodes, 21 ms |
| Chrome toggle hidden section | 13 | 5 cache add, children, property, state, text | raw watcher verified |

An operation is a burst, not a one-to-one event relationship. Events are primary after a TUI
operation. If none arrives before the action timeout, or an incremental refresh/cache invariant
fails, the owner performs a full semantic snapshot and reports the fallback reason. Pressing `r`
always forces that escape hatch.

## Identity

`RuntimeNodeId` is stable for one `SemanticCache` session. Exact `BackendLocator` matches always
retain it. Within one refreshed parent, unmatched old/new siblings may reconcile only when
`(SemanticRole, name, TextInputKind)` occurs exactly once on both sides. Ambiguity creates new
IDs. Application restart creates a new cache session and never reconciles.

The Chrome unique churn live run changed the target locator and retained runtime ID `161`.
Rebuilding two sibling buttons both named `Duplicate` reported `reconciled=0, new_ids=2`.

## ManagesDescendants

Qt QListWidget was observed with `manages-descendants`. `ActiveDescendantChanged` is normalized
and dirties the container subtree, but a live ActiveDescendant event was not observed in these
fixture operations. Virtualized tree/table browsing is **LIMITED SUPPORT**: the cache does not
claim that an enumerated child set is the complete logical collection.

## Password and anonymous-action safety

TextChanged normalization intentionally omits changed text. Backend node refresh never reads a
password value, and `SemanticCache` defensively drops any value attached to a Password node before
storage. The browser password sentinel had zero matches in normal/verbose/TUI watcher artifacts.

Chrome's anonymous actions remain available only through the explicit inspector index API. They
never produce a semantic TUI capability and are not used as an index-zero fallback.
