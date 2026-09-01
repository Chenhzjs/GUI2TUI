# Browser accessibility probe

This records the Phase 2B live probe performed on 2026-08-28. Values below are observed results,
not inferred browser behavior.

## Environment

- Ubuntu 24.04.4 arm64 in OrbStack, Xvfb/X11, AT-SPI 2.52.
- Official Google Chrome stable 152.0.7977.64, package `152.0.7977.64-1`.
- Chrome ran as the ordinary VM user with its sandbox enabled.

Ubuntu's `chromium-browser` and `firefox` apt entries were snap transition packages. The probe
instead installed the official arm64 Chrome `.deb` from
`https://dl.google.com/linux/direct/google-chrome-stable_current_arm64.deb`.

Launch command (profile path shortened for readability):

```bash
google-chrome-stable \
  --force-renderer-accessibility=complete \
  --disable-gpu \
  --disable-dev-shm-usage \
  --no-first-run \
  --no-default-browser-check \
  --disable-background-networking \
  --user-data-dir=/tmp/gui2tui-chrome-profile \
  --window-size=1200,900 \
  file:///workspace/tests/fixtures/browser_fixture.html
```

`gui2tui inspect --list` discovered the application as `Google Chrome`.

## Small fixture observations

The 277-node tree contained these web-content mappings:

| HTML content | AT-SPI observation | Current semantic mapping |
| --- | --- | --- |
| `<h1>` | heading/static text subtree | Label |
| text input | `entry`; Text interface returned no printable value in this probe | TextInput, value unavailable |
| password input | `password text` | TextInput, Password; no value read |
| checkbox | `check box`, checked/checkable states | CheckBox |
| button | `button`, Action interface | Button |
| status div | `status bar` plus static-text child | StatusBar + Label |
| `<select>` | `combo box`; popup Menu has Selection | Unknown(combo box) + Menu |

Chrome returned `GetActions = [('', '', ''), ('', '', '')]` for fixture buttons: both names,
descriptions, and keybindings were empty. Explicit low-level
`--action NODE --index 0` activated the controlled fixture and changed the checkbox to checked and
the status to `Status: activated`. The conservative semantic resolver does not turn anonymous
index 0 into Activate, so browser TUI activation remains unavailable rather than guessing.

The fixture password sentinel occurred zero times in:

- normal inspector output;
- verbose inspector output;
- a recorded 120x40 TUI terminal transcript; and
- the Chrome process log.

## Scale results

`browser_large_fixture.html?count=N` generates repeated section/button/input/list structures.
Every result is a full untruncated inspector traversal, run three times.

| Generated rows | Printed AX nodes | Minimum | Median | Maximum | Traversal errors |
| ---: | ---: | ---: | ---: | ---: | --- |
| 25 | 427 | 0.455 s | 0.485 s | 0.490 s | none observed |
| 100 | 952 | 1.033 s | 1.076 s | 1.098 s | none observed |
| 250 | 2,002 | 2.152 s | 2.238 s | 2.263 s | none observed |
| 700 | 5,152 | 5.925 s | 5.929 s | 5.941 s | none observed |

The small 277-node fixture took 0.332 / 0.335 / 0.358 seconds (min/median/max).

## Object lifetime and dynamic tree

The `Churn target` button initially had object path
`/org/a11y/atspi/accessible/270`. Activating `Replace subtree` rebuilt a semantically equivalent
DOM subtree; the replacement button kept role/name but received path
`/org/a11y/atspi/accessible/278`. Querying actions on the old BackendLocator returned
`org.freedesktop.DBus.Error.UnknownObject`, classified by GUI2TUI as an unavailable/stale object.

Dynamic controls produced these complete snapshots:

| Mutation | Printed nodes | Snapshot time | Observation |
| --- | ---: | ---: | --- |
| baseline with visible hidden section | 282 | 0.322 s | five extra semantic nodes visible |
| Add 100 items | 382 | 0.423–0.479 s | 100 `Dynamic item` nodes present |
| Toggle hidden section off | 377 | 0.464 s | hidden section count changed to zero |
| Remove 100 items | 277 | 0.370 s | dynamic item count changed to zero |

These results show that semantically equivalent browser objects can change BackendLocator and
that full snapshots already cost about six seconds at roughly 5,000 nodes. Phase 2B does not
implement event subscriptions, cache mutation, or identity reconciliation.
