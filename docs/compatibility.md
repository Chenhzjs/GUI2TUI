# Compatibility matrix

## Static acquisition and topology (Phase 3H, 2026-08-31)

| Capability | GTK Picture | LibreOffice embedded Image | Chrome / Firefox references |
| --- | --- | --- | --- |
| No-reference semantic visual | PASS | PASS, actual inline embedded Image | reference-first preserved |
| Generic RenderedSnapshot | PASS, visual-only native client, 480×180 | UNAVAILABLE: native/AT-SPI geometry mismatch | not needed; reference path does not capture |
| Ambiguous coordinates | PASS safe refusal for Picture-under-Label layout | PASS safe refusal | NOT TESTED for acquisition |
| Headless materialization | PASS, 6,419-byte PNG, SHA-256, TTL cleanup | unavailable without reliable bounds | Chrome reference inspect: PASS, 0 bytes |
| Same-host actual viewer | PASS, EOG PNG visually verified, 0 payload bytes | unavailable | Phase 3G reference handoff regression PASS |
| TUI without endpoint | PASS: startup/F4 0 captures; explicit m 1 capture; Esc/Quit | NOT TESTED this phase | descriptor/reference resolution independent of endpoint |
| Wayland / remote snapshot companion | NOT IMPLEMENTED / NOT TESTED | NOT IMPLEMENTED / NOT TESTED | NOT TESTED |

See [coordinate limits, deployment matrix and evidence](static-acquisition.md).
RenderedSnapshot is not an original embedded artifact and does not promise an unoccluded image.

Validated through 2026-08-30 in Ubuntu 24.04 arm64, Xvfb/X11, AT-SPI 2.52, GTK 4.14.5,
Qt 6.4.2 through PyQt 6.6.1, Chrome 152, and Firefox 154.0.1.

## External modality resolution (Phase 3G)

| Capability | GTK4 | Qt6 | Chrome | Firefox | LibreOffice |
| --- | --- | --- | --- | --- | --- |
| Image semantic role | PASS (`Image`) | ACCESSIBILITY-LIMITED (`QLabel`) | PASS (`Image`) | PASS (`Image`, fresh profile 2026-08-31) | PASS (`Image`) |
| Generic resource reference | UNRESOLVED | UNRESOLVED | PASS (Image GetURI / Accessible attribute) | PASS (surrounding Hyperlink; video poster `src`) | UNRESOLVED |
| PDF/video/model hyperlink descriptors | NOT TESTED | NOT TESTED | PASS (`GetURI(0)`) | PASS (`GetURI(0)`) | NOT TESTED |
| Native video source | NOT TESTED | NOT TESTED | UNRESOLVED (`Video`) | UNRESOLVED (poster Image is not video source) | NOT TESTED |
| TUI → separate recording broker | read-only | no Image candidate | PASS (4 references, 0 payload bytes) | PASS (4 references, 0 payload bytes) | read-only |
| Portable artifact from accessibility | UNRESOLVED | UNRESOLVED | UNRESOLVED | UNRESOLVED | UNRESOLVED |
| Safe fallback | unavailable | unavailable | reference handoff / unavailable video | reference handoff / unavailable video | unavailable |

This matrix is updated only from live AT-SPI evidence. Application-specific APIs
are forbidden; toolkit-independent broker and transport behavior is tested separately.
Recording handlers prove dispatch, not downloaded content or visual rendering.
`NAnchors` failed in both browser probes; a single bounded `GetURI(0)` read succeeded.
See [Phase 3G validation](phase3g-validation.md) for commands and validation boundaries.

| Feature | GTK4 | Qt6 | Chrome | Firefox |
| --- | --- | --- | --- | --- |
| Application discovery | PASS | PASS | PASS (Chrome 152) | PASS (Firefox 154.0.1) |
| Button role | PASS (`button`) | PASS (`button`) | PASS (`button`) | PASS (`button`) |
| Button action | PASS (`Click`) | PASS (`Press`) | PARTIAL: two anonymous actions; explicit index 0 worked | PARTIAL: anonymous actions only |
| Plain TextInput | PASS (`text`) | PASS (`text`) | PARTIAL (`entry`; value unavailable) | PASS (`entry`; value available) |
| EditableText interface | PASS | PASS | NOT EXPOSED | EXPOSED |
| Atomic SetTextContents | PASS | PASS | UNAVAILABLE | REJECTED/NORMALIZED: call returned true, independent read-back stayed unchanged |
| Commit events | `TextChanged` delete + insert | `TextChanged` delete + insert (plus one legacy property signal) | N/A | no target event for rejected atomic write; native edits emit `TextChanged` |
| TUI edit loop | PASS | PASS | Read-only by contract | PARTIAL: local editor works and safely reports rejected GUI write |
| Password TextInput | PASS (`password text`) | PASS (`password text`) | PASS (`password text`) | PASS (`password text`) |
| Password redaction | PASS | PASS | PASS: sentinel absent from normal/verbose/TUI/log | PASS: sentinel absent from normal/verbose/TUI/events/log |
| Checkbox state | PASS | PASS | PASS | PASS |
| Checkbox action | NO ACTION EXPOSED in fixture | PASS (`Toggle`) | PARTIAL: anonymous actions only | PARTIAL: anonymous actions only |
| List | PASS; parent `Selection` | PASS; `Table` interface | PARTIAL: HTML select is `combo box`; popup has `Selection` | PARTIAL: HTML select maps to `combo box` |
| ListItem selection | PASS through parent `Selection.select_child` and Choice overlay | PASS through item `Toggle` and Choice overlay | N/A for fixture ComboBox | PASS for HTML select through visible parent Selection |
| Selection backend | PASS | PASS through child action | hidden parent rejected; safely unavailable | PASS |
| Menu inspection | NOT TESTED in bundled fixture | PASS | PASS for HTML select popup | NOT TESTED |
| OpenMenu | NOT TESTED | PASS (`ShowMenu`) | NOT TESTED | NOT TESTED |
| MenuItem activation | NOT TESTED | PASS (`Press`) | NOT TESTED | NOT TESTED |
| Browser tree | N/A | N/A | PASS: small fixture 277 printed nodes | PASS: fixture tree 234 nodes after dismissing first-run spotlight |
| Browser large tree | N/A | N/A | PASS: up to 5,152 nodes | NOT TESTED |
| Browser object churn | N/A | N/A | PASS: old locator became stale, new locator changed | NOT TESTED |
| Keyboard TUI loop | PASS | PASS | Text input read-only | PARTIAL: Begin/Edit/Commit executes; GUI read-back rejects unchanged value |
| Mouse TUI loop | PASS (automated SGR input) | PASS (automated SGR input) | N/A | NOT TESTED |
| Application-gone handling | PASS | PASS | NOT TESTED | NOT TESTED |
| Raw event watcher | PASS | PASS (including Qt-compatible property body) | PASS | PASS: native input emitted focus/caret/selection/text events |
| Incremental button update | PASS: 13 nodes / 31 ms | PASS: 2 nodes / 3 ms | PASS: 5 nodes / 212 ms on 5,158-node tree | NOT TESTED |
| Incremental list selection | PASS: 9 nodes / 26 ms | PASS: 1 node / 2 ms | NOT TESTED | NOT TESTED |
| Runtime identity churn | NOT TESTED | NOT TESTED | PASS: unique reconciled; duplicates rejected | NOT TESTED |
| ComboBox semantic role | PARTIAL / ACCESSIBILITY-LIMITED: options unavailable, production popup calls 0, read-only | PASS: named options + child `Toggle`; Beta selected with `ShowMenu` calls 0 | PARTIAL: named options exposed, but hidden parent Selection rejected; read-only | PASS: named options + visible parent Selection; Beta selected |
| Choice overlay | PASS safe degradation; separate List PASS | PASS ComboBox/Radio/List | PASS read-only degradation | PASS selection |
| Choice disclosure | Unavailable / not used | NotRequired | NotRequired for discovery; no safe selection | NotRequired |
| Choice dismissal | Unknown for unavailable GUI popup; TUI Esc local | NotApplicable | NotApplicable | NotApplicable |
| RelationSet | PASS: five `LabelledBy` | PASS: `LabelFor` + `LabelledBy` | PASS: `EmbeddedBy`, `MemberOf` on large fixture | PASS: `NodeChildOf`, `LabelledBy`, `DescribedBy`, `LabelFor` in first-run UI |
| Interaction scopes | Window/ModalDialog/Popup PASS | Window/ModalDialog/Popup PASS | Window PASS | Window/Dialog PASS in prior probe |
| Command hierarchy/search | PASS | PASS | anonymous actions remain excluded | anonymous actions remain excluded |
| Cache.GetItems bootstrap | PASS when complete; Auto detects partial cache | LEGACY SIGNATURE, EMPTY; Auto walk fallback | PASS, modern, 5,158 items | PARTIAL: modern, 217 items / 27.048 ms; incomplete record triggers walk fallback |
| Bounded event overflow recovery | NOT TESTED live flood | NOT TESTED live flood | PASS: capacity 4, 197 dropped, one resync | NOT TESTED |
| Collection.GetMatches probe | no Collection in fixture cache | no Collection in fixture cache | PARTIAL: advertised broadly, root queries returned zero | PARTIAL: 248 Collection nodes in the latest probe; root queries returned zero |
| Semantic content model | PASS: read-only multiline TextView without Document role | PARTIAL: model detected; Qt 6.4 crashed serving Text read | PASS: 76-block fixture | PASS: 52-block fixture |
| Reader | PASS: 1 block / 170 bytes | FAILED: fixture process segfaulted on Text query | PASS: 18-block viewport | PASS: 18-block viewport |
| Outline | N/A: no headings in fixture | N/A | PASS: 4 headings | PASS: 4 headings |
| Content search | PASS: loaded range | NOT TESTED after Text crash | PASS: indexed + loaded | PASS: indexed + loaded |
| VirtualCollectionModel | complete fixture list | PASS: `PartialRealized`, 3 realized, total unknown | PARTIAL: ARIA/list semantics probed | PARTIAL: select semantics probed |
| ActiveDescendantChanged | NOT TESTED | NOT OBSERVED | NOT OBSERVED | NOT OBSERVED |

## Qt accessibility activation

In the headless Xvfb session, Qt did not appear in the registry while both `org.a11y.Status` properties were false. The test set:

```bash
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>'
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
  --method org.freedesktop.DBus.Properties.Set org.a11y.Status ScreenReaderEnabled '<true>'
```

After restarting the Qt fixture, `GetAll org.a11y.Status` returned both properties as true and `gui2tui-inspect --list` reported `gui2tui-qt-fixture`.

## Measured snapshot samples

| Application | Printed nodes | Inspector wall time |
| --- | ---: | ---: |
| GTK4 fixture | 13 | 0.049 s |
| Qt6 fixture | 19 | 0.050 s |
| gtk4-demo initial window | 91 | 0.144 s |
| Chrome browser fixture | 277 | 0.332 / 0.335 / 0.358 s (min/median/max) |
| Chrome large fixture, 25 generated rows | 427 | 0.455 / 0.485 / 0.490 s |
| Chrome large fixture, 100 generated rows | 952 | 1.033 / 1.076 / 1.098 s |
| Chrome large fixture, 250 generated rows | 2,002 | 2.152 / 2.238 / 2.263 s |
| Chrome large fixture, 700 generated rows | 5,152 | 5.925 / 5.929 / 5.941 s |

The GTK/Qt snapshot entries are single observations, while every Chrome scale row is three complete
inspector traversals. These are development measurements rather than a benchmark. No transient
object error was printed during the recorded scale runs. Chrome's roughly linear multi-second
cost above 2,000 nodes and verified locator churn are evidence for opening the event-cache design
gate. Phase 2C subsequently validated the common event cache. A repeat run of the 700-row fixture
printed 5,158 nodes in 5.595 s; its local checkbox/status mutation then refreshed only five backend
nodes in 212 ms with zero additional full snapshots. Add/remove 100 controls refreshed 103/3 nodes
in 161/21 ms respectively.

Phase 3A forced-cache bootstrap rebuilt the same 5,158-node Chrome tree in a 201.751 ms backend
median (0.236 s command wall median), versus 5,150.075 ms (5.167 s wall) for walk. Auto measured
191.531 ms backend / 0.206 s wall median. See
[bootstrap.md](bootstrap.md) for the three runs, selective-enrichment count, partial-cache guard,
and Collection probe.

## Browser environment and caveat

The probe used official Google Chrome stable 152.0.7977.64 (`google-chrome-stable` package
152.0.7977.64-1, arm64) on the same Xvfb session, launched as an unprivileged user with
`--force-renderer-accessibility=complete`. The sandbox remained enabled; `--no-sandbox` was not
used. See [browser-probe.md](browser-probe.md) for commands, dynamic-tree results, and the action
compatibility caveat.

## Firefox Phase 3B probe

Firefox 154.0.1 for Linux aarch64 was installed from Mozilla's official release tarball because
Ubuntu's `firefox` APT package is a Snap transition package and Snap was unavailable in the test
VM. It was launched on Xvfb with a fresh test profile and the local browser fixture; no
Firefox-specific semantic branch or accessibility command-line flag was used. The desktop AT-SPI
status properties were already enabled. First-run spotlight UI had to be dismissed before the Web
document became the active accessible subtree.

The fixture exposed 234 nodes through recursive inspection. `Cache.GetItems` used the modern
signature and returned 217 records in 27.048 ms, but one cached object advertised a missing child,
so forced cache reconstruction correctly rejected it and Auto retained the walk fallback. The
plain HTML input exposed `Text` + `EditableText` and state `editable`. Firefox returned `true` from
`SetTextContents`, but emitted no target value event and an independent full-text read remained
unchanged. GUI2TUI therefore reported a normalized/rejected update and did not mutate its semantic
cache optimistically. Native editing of the same field emitted `TextChanged` plus caret/selection
events, proving the event stream itself was active. Password redaction counts were zero in normal,
verbose, TUI, event, and Firefox log captures.

## Phase 3C semantic presentation

The generic transcompiler was subsequently exercised against GTK4, Qt6,
LibreOffice Writer, Java Swing, Electron, Chrome, Firefox, and a GTK opaque
drawing-area fixture. It reconstructs fields/forms/commands/selections and
preserves sparse graphical content without toolkit-name branches. Exact node,
region, scene, timing, and action results are in
[transcompiler.md](transcompiler.md).

Phase 3E additionally validated terminal-native content reading in LibreOffice Writer (1,976
nodes, 17 blocks, 4 headings), Chrome, Firefox, and a generic GTK TextView. The measurements and
Qt rich-text bridge failure are recorded in [content-navigation.md](content-navigation.md).

## Phase 3F progressive content operations

| Capability | GTK4 | Qt6 | Chrome | Firefox | LibreOffice |
| --- | --- | --- | --- | --- | --- |
| Indexed Reader search | PASS | BRIDGE LIMITED | PASS | PASS | PASS |
| Explicit progressive scan | PASS | QUARANTINED AFTER FIRST FAILURE | PASS | PASS | PASS |
| Search cancellation | UNIT PASS | N/A | LIVE PASS | NOT TESTED | LIVE PASS |
| Password excluded from scan | PASS | PASS | PASS | PASS | N/A |
| Semantic table API | NO TABLE | NO TABLE IN PROBE | PASS (HTML table + ARIA grid) | PASS (HTML table + ARIA grid) | PASS |
| Virtual collection navigation | UNIT PASS; live collection unavailable | PASS (`PartialRealized` fixture lists) | PASS (realized table rows) | UNIT PASS; live collection unavailable | PASS (realized table cells) |

`BRIDGE LIMITED` means the Qt rich-text fixture crashed during its first bounded Text probe. The
runtime then quarantined that source and issued no automatic retry; it does not identify Qt by
name. Firefox 154 was rerun from Mozilla's installed arm64 tarball. Detailed commands and measurements are in
[progressive-content.md](progressive-content.md).

## Phase 3D relational/contextual probe

The completion run replaced traversal-prefix enrichment with a contextual priority scheduler.
Chrome 5,158 nodes issued only 256 relation RPCs (8.860–9.244 ms) and reached its first TUI frame
in 491–496 ms internally, while LibreOffice exposed 478 reachable safe leaves without rendering
them as 478 default scene rows. Complete cross-family counts and the safe cross-toolkit Choice
results are in [relations.md](relations.md).

## Phase 4C real-application release-candidate validation (2026-09-01)

See [the final report](phase4c-validation.md): **real-application gates complete;
final RC pipeline pending**. Mousepad, GTK/Qt fixtures, a meaningful Qt
Designer workflow, Chrome, Firefox, Writer, Writer-long, Writer Options,
VS Code best-effort and static-image paths have fresh evidence. PCManFM-Qt
remains a documented blocked P2 with no adapter.

Chrome 5K has two measured startup conditions: five complete-Cache samples had
a 223.23 ms median; five incomplete-Cache samples had a 4,073.23 ms median and
used the correctness walk. The same-condition old binary median was 4,054.31 ms,
so this is not an unexplained product regression. Writer-long remains explicitly
`PartialRealized`; Reader/search wording never claims complete source coverage.
The frozen core architecture is unchanged. See the linked report and final
[issue ledger](validation/phase4c-issues.md) for exact scope.
