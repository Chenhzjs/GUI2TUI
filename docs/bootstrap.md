# Semantic bootstrap strategies

This document records Phase 3A measurements from Ubuntu 24.04 arm64, Xvfb/X11,
AT-SPI 2.52, GTK 4.14.5, Qt 6.4.2, and Google Chrome 152.0.7977.64. It is an
implementation report, not a claim that every toolkit exposes the same cache.

```text
Auto
 ├─ usable org.a11y.atspi.Cache.GetItems
 │    → bulk records → tree reconstruction → selective enrichment
 └─ missing, malformed, empty, or detectably incomplete
      → recursive Accessible walk
```

`--bootstrap auto` is the default. `cache` is a diagnostic forced fast path and
returns a clear error when the cache cannot form a trustworthy tree. `walk`
remains the compatibility fallback, verbose-inspector path, and semantic
reference implementation.

## Cache wire formats and toolkit support

The `atspi` 0.30 proxy first decodes modern `CacheItem` records. If that typed
call fails, GUI2TUI retries the same `GetItems` method using `LegacyCacheItem`.
Both forms immediately normalize to `BulkAccessibleRecord`; no toolkit name is
consulted.

| Application | Wire result | Items | GetItems observation | Bootstrap result |
| --- | --- | ---: | ---: | --- |
| GTK fixture | modern | 20 after full realization; 4 immediately after a fresh start | 0.5–3.2 ms observed | cache when complete; Auto walk fallback when an 8-child panel had only 1 cached child |
| Qt fixture | legacy signature | 0 | 0.128 ms observed | unusable; Auto uses walk |
| Chrome small fixture | modern | 282 | 5.2–12.5 ms observed | cache |
| Chrome large fixture | modern | 5,158 | 80.8–101.5 ms observed | cache |

The Qt result verifies the legacy wire signature, not a successful legacy bulk
tree: this Qt version returned an empty list. Legacy parsing remains covered by
unit tests and Auto does not treat an empty cache as success.

`GetItems` is cache residency, not unconditional tree-membership truth. For an
ordinary reachable node, GUI2TUI rejects a record set when its advertised child
count exceeds the cached children. `State::ManagesDescendants` is exempt because
virtualized containers are expressly allowed to expose only realized children.
The application root is exempt from the count check because Chrome reported
root child data that did not agree with cache residency. One root `GetChildren`
RPC instead repairs the cached parent/index of every root child that is present.
This changed Chrome cache/walk comparison from a wrongly nested auxiliary window
to identical structure. Orphans are ignored, duplicate locators and cycles are
rejected, and any failure falls back in Auto mode.

## Bulk record and reconstruction

The internal normalized record contains locator, application, parent, optional
sibling index/child count, legacy explicit children, interfaces, role, name,
description, states, and enrichment fields. Modern parent/index records and
legacy explicit-child records therefore share one reconstruction algorithm.
Children are ordered by advertised sibling index, then object path for a
deterministic tie-break. Reachable transient objects are retained.

The bulk record supplies role, cached names/descriptions, states, interfaces,
and structure without per-node calls. Selective enrichment reads:

- missing display names only for roles currently presented by the semantic TUI;
- action lists only for current interactive roles that advertise Action; and
- bounded text only for plain editable TextInput nodes.

PasswordText is classified before text enrichment and is never read. Geometry
is not fetched by the normal fast path. The 5,158-node Chrome run issued 2,907
selective enrichment calls plus one root-relationship RPC; enrichment tasks are
concurrently bounded to 32, and most calls were missing names on presentable
generated controls.

Normalized cache/walk output was identical for the GTK fixture after removing
debug-only actions on non-interactive labels/windows. Chrome had identical tree,
roles, names, states, input kinds, and interactive actions; its only normalized
difference was the omitted numeric value `0` on three unsupported resize-handle
Sliders. Qt had no usable bulk records, so Auto and the TUI consistently used
the walk result rather than claiming equivalence for an empty cache.

## Chrome large-fixture benchmark

These three-run development measurements used 700 generated rows (5,158 AT-SPI
nodes). Command wall time includes process startup; backend time is the metric
printed by GUI2TUI.

| Strategy | Backend runs (ms) | Backend median | Wall runs (s) | Wall median |
| --- | --- | ---: | --- | ---: |
| walk | 4993.579, 5150.075, 5391.444 | 5150.075 ms | 5.054, 5.167, 5.410 | 5.167 s |
| cache | 201.751, 191.393, 219.987 | 201.751 ms | 0.254, 0.207, 0.236 | 0.236 s |
| auto | 191.531, 191.328, 203.195 | 191.531 ms | 0.206, 0.205, 0.218 | 0.206 s |

The forced-cache backend median was about 25.5× faster than walk; wall time was
about 21.9× faster. Auto's backend/wall medians were about 26.9×/25.1× faster.
Median forced-cache breakdown was approximately 82.2 ms GetItems, 87.1 ms
enrichment/root repair, and 32.0 ms reconstruction. This exceeds the 2×
acceptance target and meets the sub-1.5-second stretch target on this host.

## Collection probe

`gui2tui inspect --app NAME --probe-collection` is research-only. Neither GTK
nor Qt fixture advertised Collection in the observed cache. Chrome advertised
it broadly (all 5,158 cached objects in the large fixture), but `GetMatches` from the
application root returned zero Buttons, TextInputs, CheckBoxes, and focusable
objects in 0.15–0.30 ms. Collection is therefore not a bootstrap source in this
phase; its ordering/context and practical coverage were not useful here.

## Event subscription during bootstrap

The TUI subscribes before bootstrap, uses the bounded event queue while bulk or
walk loading runs, then drains/coalesces the buffered suffix. CacheAdd events for
locators already represented by the completed baseline (and CacheRemove events
for unknown locators) are no-ops. A live 5,158-node Chrome mutation during the
cache bootstrap replayed nine events, refreshed five backend nodes, ended with
the checked/status state visible to an independent Inspector, and retained
`full_snapshots=1`. Overflow during bootstrap requests the same one-shot full
resynchronization used at runtime.
