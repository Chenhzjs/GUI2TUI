# Inspector reference

`gui2tui-inspect` is a developer diagnostic tool for examining the raw-to-semantic
AT-SPI boundary. Normal users should start with `gui2tui`.

## Discovery and tree inspection

```bash
gui2tui-inspect --list
gui2tui-inspect --app firefox
gui2tui-inspect --app-id 1
gui2tui-inspect --app firefox --verbose
gui2tui-inspect --watch-events --app firefox
```

Large traversal controls:

```bash
gui2tui-inspect --app firefox --max-depth 20 --max-nodes 5000
gui2tui-inspect --app firefox --timeout-ms 5000
gui2tui-inspect --app firefox --bootstrap auto
gui2tui-inspect --app firefox --bootstrap cache
gui2tui-inspect --app firefox --bootstrap walk
gui2tui-inspect --app firefox --probe-cache
gui2tui-inspect --app firefox --probe-collection
```

Limits and remote-operation failures are visible in output rather than silently
producing a seemingly complete tree.

## Explicit backend operations

Nodes with operations print an `atspi1_...` backend locator.

```bash
gui2tui-inspect --actions NODE_ID
gui2tui-inspect --activate NODE_ID
gui2tui-inspect --action NODE_ID --index 0
gui2tui-inspect --action-name NODE_ID click
gui2tui-inspect --select-child PARENT_NODE_ID --child-index 1
```

`--action-name` prefers exact then ASCII case-insensitive matching; duplicates
are rejected. `--activate` only accepts advertised `click`, `press`, or
`activate`, in that order. It never invokes an arbitrary first action.

Explicit index invocation is low-level diagnosis, not the TUI safety contract.

## Semantic diagnostics

```bash
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

Use `RUST_LOG=debug` for contents-free backend and performance diagnostics.

## Identity

`BackendLocator` is the reversible URL-safe encoding of an AT-SPI unique D-Bus
name plus object path. It is valid only while the original accessible object is
alive. `RuntimeNodeId` is compact and stable only inside one live semantic-cache
session. Application restart always creates a new identity generation.

See [Architecture](architecture.md), [events](events.md), and
[semantic contract](semantic-contract.md) for the internal model.
