# Semantic content navigation

This document records the Phase 3E implementation and the live Ubuntu 24.04 arm64/Xvfb results
from 2026-08-30. It describes observed AT-SPI behavior; it is not a toolkit API catalogue.

## Architecture

```text
Relational Semantic Graph
├── task/control analysis → TuiScene controls and commands
└── content analysis      → SemanticContentModel
                              ├── Reader
                              ├── Outline
                              ├── indexed/loaded search
                              └── VirtualCollectionModel
```

`SemanticCache` remains the authoritative live arena. `SemanticContentModel` is a derived index,
and `ContentBlockId` is independent from both `RuntimeNodeId` and `SceneElementId`. The main scene
contains one bounded `DocumentSummary` per content root instead of materializing every paragraph.
Controls with scene bindings remain in the task model.

## Data model

The implementation is in [`src/content`](../src/content):

- `SemanticContentModel`: root, kind, metadata, root blocks, block index, navigation indexes, and
  `Complete`/`PartialRealized`/`Unknown` completeness.
- `ContentBlock`: content-local identity, source runtime node, kind, label, lazy text state,
  children, and interactive source nodes.
- `ContentBlockKind`: heading, paragraph, text, link, list/list item, quote, landmark, form/table
  anchor, comment, opaque media, group, or unknown.
- `VirtualCollectionModel`: owner, realized children, selected children, optional active
  descendant, logical completeness, and optional known total. It never treats a realized child
  count as a logical total.

Password `TextInput` nodes are excluded before block construction. The backend also rejects the
AT-SPI `password text` role before constructing a Text proxy.

## Document and text backend

The backend uses the actual `atspi 0.30` proxies:

- `DocumentProxy::get_attributes`, `get_locale`, `current_page_number`, and `page_count`;
- `TextProxy::character_count`, `get_string_at_offset(..., Granularity::Paragraph)`, and bounded
  `get_text` fallback chunks;
- `HypertextProxy::get_n_links`.

Reader startup requests only 12 visible blocks plus 6 lookahead blocks. One source is limited to
16 ranges per materialization. GTK 4.14 returned a non-advancing paragraph range for TextView, so
the generic backend falls back to a bounded 4,096-character chunk rather than looping or eagerly
reading the whole object.

`ContentCache` is a separate LRU with a 512 KiB / 256-range default budget. Text/property/children
events invalidate ranges for the affected source before the content model is rebuilt. There is no
full application refresh on the normal content mutation path.

## Reader, outline, and search

The Reader is terminal-native: text wraps to terminal width, headings use `#`, links use `[Link]`,
list items use bullets, quotes use `>`, and opaque media uses `[Media]`. `j/k` and page keys move by
semantic block, `o` opens the heading outline, `/` opens content search, and Escape restores the
previous scene focus.

Search covers semantic labels and text ranges already loaded into the bounded cache. It returns a
`ContentBlockId`, source runtime node, match range, and safe preview. A progressive full-document
scan is **NOT IMPLEMENTED**: doing it correctly requires a cancellable asynchronous fetch task and
would defeat the bounded first-viewport contract. The live fixtures expose useful heading/link
labels before body fetch, while loaded-range search covers what the user has read; this phase does
not justify a second, eager scan architecture.

## Live compatibility

| Application | Root/AT-SPI interfaces | Blocks | Headings | Links | Forms | Reader result |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| LibreOffice Writer 24.2 | `Document` semantic role; descendant Text objects; root `manages-descendants` | 17 | 4 | 0 | 0 | PASS, partial |
| Firefox 154 | Web document; Document/Text/Hypertext | 52 | 4 | 3 | 18 | PASS |
| Chrome 152 | Web document; Document/Text/Hypertext | 76 | 4 | 3 | 18 | PASS |
| GTK 4.14 TextView | read-only multiline TextInput; Text | 1 | 0 | 0 | 0 | PASS without Document role |
| Qt 6.4 QTextEdit | read-only multiline TextInput; Text/EditableText | 1 | 0 | 0 | 0 | **FAILED**: Qt process segfaulted while serving the Text read |
| Electron | — | — | — | — | — | **BLOCKED**: Electron binary download stalled in the Linux VM |

No production rule names any application or toolkit. Qt's failure is isolated in
`qt6_rich_text_fixture.py`; it does not weaken the generic content contract or the stable Qt
control regression fixture.

## Live measurements

| Application | Semantic nodes | Bootstrap | Main scene before → after | First Reader viewport |
| --- | ---: | ---: | ---: | ---: |
| GTK rich text | 42 | 5.1 ms Cache | 19 → 19 | 6.9 ms; 1 RPC, 2 ranges, 170 bytes |
| Firefox fixture | 261 | 329–390 ms walk fallback | 121 → 98 | 12.0 ms; 4 RPCs/ranges, 251 bytes |
| Chrome fixture | 331 | 19–32 ms Cache | 93 → 57 | 34.5 ms; 3 RPCs/ranges, 171 bytes |
| LibreOffice fixture | 1,976 | 2.41–2.47 s walk fallback | 40 → 23 | 38.2 ms; 14 RPCs/ranges, 547 bytes |
| Chrome large fixture | 5,158 | 202–328 ms Cache | 3,541 → 2,134 | first-frame total 423/431/565 ms |

The Chrome large fixture intentionally contains 1,402 form controls, so the remaining 2,134 scene
elements are not all document prose. The live reachability audit reported 1,405 content targets
reachable and zero unreachable; the compression did not discard controls merely to improve a
benchmark.

## Dynamic and virtualized content

Replacing the browser article paragraph generated 8 events coalesced into 3 dirty scopes. The
runtime refreshed 31 nodes in 57 ms, reconciled the replaced subtree, invalidated the affected
content ranges, and independently read back `Status: replaced article paragraph.`

The Qt list live probe exposed `manages-descendants`, 3 realized children, one selected child, and
no trustworthy logical total. Selecting Beta via the child's named `Toggle` was confirmed in a
new snapshot. Chrome, Firefox, and Qt interaction probes emitted selection/state events but no
`ActiveDescendantChanged`; normalization and cache support exist, but the event was **NOT
OBSERVED** live.

`Collection.GetMatches` was advertised by Chrome, Firefox, and LibreOffice roots, but the tested
button/input/checkbox/focusable queries returned zero results. GTK and Qt fixtures exposed no
Collection nodes. It is not used as the content source.

## Diagnostics

```bash
gui2tui-inspect --app firefox --dump-content
gui2tui-inspect --app firefox --dump-outline
gui2tui-inspect --app firefox --probe-document
gui2tui-inspect --app firefox --dump-virtual-collections
gui2tui-inspect --app firefox --audit-content-reachability
gui2tui-inspect --app firefox --dump-scene
```

Raster rendering, PDF/ODT parsing, DOM/CDP, UNO, and toolkit-native model access are not used.

