# Progressive content operations

Phase 3F extends the bounded Phase 3E Reader without making a full document scan part of startup
or ordinary `/` search.

## Runtime model

```text
SemanticCache (authoritative GUI semantics)
        ↓ derived
ContentCatalog
        └─ SemanticContentModel
              └─ ContentArena
                    ├─ HashMap<ContentBlockId, ContentBlock>
                    ├─ ordered ContentBlockId list
                    └─ HashMap<RuntimeNodeId, Vec<ContentBlockId>>
        ↓
Reader / Outline / Indexed Search / Progressive Search / Collection / Table
```

Base `ContentBlockId` values derive from cache-session-stable `RuntimeNodeId` values. This keeps
Reader positions and search results stable through unrelated node changes. Ambiguous semantic
replacement still receives a new runtime ID and therefore a new block ID. Loaded paragraph ranges
remain bounded by the 512-KiB/256-range LRU.

Text/property events invalidate only ranges and blocks belonging to their source. A safe local
patch refreshes labels/summary text in place. Children/window/cache lifecycle events request a
structural content rebuild. Rebuilds still preserve IDs for unchanged runtime sources.

## Search contracts

`/` searches only semantic labels and text already resident in `ContentCache`. It never starts a
document scan. `Ctrl-F` inside Search explicitly starts `ContentSearchSession`:

```text
25-ms scheduler tick
  ├─ at most 4 semantic blocks
  └─ at most 2 Text RPCs
        ↓
stream each match immediately
```

Escape changes the session to `Cancelled`; future ticks issue no RPC. A single in-flight D-Bus
operation is still governed by the normal backend timeout. Complete models display `scanned / total`;
partial models only display the scanned count. Mutation invalidates results for the changed source;
replacement/removal of the search root cancels the session. Password nodes never enter the content
model, cursor, cache, result preview, or logs.

The Chrome 7,266-node cancellation run scanned 312 of 7,009 blocks and issued 42 Text RPCs before
Escape. It then stopped and restored the Reader. The 184-block Chrome run completed incrementally;
LibreOffice completed 17 blocks and GTK completed one block. Search ranges remained inside the
existing LRU.

## Runtime Text trust

```text
Unsupported
Declared ── bounded successful read ──→ Verified
Declared ── runtime failure ──────────→ Quarantined
Verified ── runtime failure ──────────→ Quarantined
Quarantined ── no automatic retry in this app generation
```

The state machine uses observed behavior only; no toolkit or application name appears in the
decision. Password sources are never declared/probed. Restarting the application creates a new
runtime and evaluates capabilities again.

In the isolated Qt 6.4 QTextEdit fixture, the first bounded Text probe still caused the target
fixture to segfault. GUI2TUI detected loss, retained no optimistic content, and performed zero
automatic retries. The user-facing Reader degrades to unavailable content. GTK TextView, Chrome,
Firefox, and LibreOffice completed bounded reads through the same state machine. Firefox 154 was
run from the previously installed official Mozilla arm64 tarball. Its fixture grew from an initial
36-node browser shell to a 283-node tree after accessibility events; the resulting content model
contained 75 blocks and its explicit scan streamed results without a toolkit-specific path.

## Virtual structures

`VirtualCollectionModel` now owns a terminal navigation position. The position is a realized
`RuntimeNodeId`, not a row number. ActiveDescendant is optional: selected/focused states and
realized-child changes are sufficient. If the current item disappears, navigation selects the
active/selected item or the nearest remaining realized item. No logical total is fabricated and no
input is injected to force more realization.

`SemanticTableModel` records owner, optional logical dimensions, realized cells, headers,
row/column spans, completeness, and a semantic cell position. Small tables can be rendered inline;
the Table task moves over realized cells without emitting every cell into the main scene. Live
standard Table probes returned Chrome HTML table `3 × 3`, Chrome ARIA grid `3 × 2`, and
LibreOffice Writer `2 × 1` realized structure.

Chrome's HTML table was also opened through the real Reader search result into the terminal Table
task. The captured frame showed the semantic-cell navigation help, and a Right-key operation moved
the model's `RuntimeNodeId`-backed cell position. Qt's live list fixture exposed two
`PartialRealized` lists with three realized items each (`Alpha`, `Beta`, `Gamma`) and no fabricated
logical total.

Collection APIs remain optional acceleration. In this run Chrome and LibreOffice exposed the
interface, but `GetMatchesFrom` and `GetMatchesTo` returned zero and `GetActiveDescendant` returned
`UnknownMethod`. Correctness uses the semantic cache and events instead.

## Content scopes and task handoff

Content roots are classified using active/focused state, window/dialog scope, showing state, and
semantic size:

- `Primary`: default Reader root.
- `ActiveTransient`: active dialog/transient content.
- `BackgroundSecondary`: retained in the catalog but omitted from the default scene.

There is no title or application-name rule. Chrome's main web document was Primary; two independent
Omnibox documents were BackgroundSecondary. Enter on a Reader FormAnchor or safely bound Link moves
focus to the existing semantic task scene. Escape restores the saved `ContentBlockId` position.

## Phase 3F performance samples

These are development measurements on OrbStack Ubuntu 24.04 arm64 under Xvfb, not a benchmark:

| Application | Semantic/content size | Progressive result |
| --- | ---: | --- |
| GTK TextView | 1 block | complete, 1 match, 0 new Text RPC |
| LibreOffice Writer | 1,976 nodes / 17 blocks | complete, 4 matches, 0 new Text RPC |
| Firefox 154 | 283 nodes / 75 blocks | streamed results; 0 new Text RPC in the captured run |
| Chrome generated document | 7,266 nodes / 7,009 blocks | cancelled at 312 blocks / 42 Text RPCs; no later RPC |
| Chrome 5K startup | 5,166 semantic nodes | 487 / 496 / 532 ms; median 496 ms; RSS 40,620–40,708 KiB |

The Chrome startup run includes cache bootstrap, arena construction, relation budget, content
analysis, scene compilation, and first TUI frame. The configured relation budget remained 256.

`OpaqueSurfaceProvider`, file parsing, DOM/CDP, UNO, input injection, and toolkit-private APIs remain
unimplemented.
