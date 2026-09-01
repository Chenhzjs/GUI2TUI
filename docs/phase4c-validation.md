# Phase 4C — real-world validation and release-candidate evidence

**REAL-APPLICATION GATES COMPLETE; FINAL RC PIPELINE PENDING.**

The real-application acceptance gates are closed for the explicitly documented
v0.1 scope. Final Phase 4C validation additionally requires the non-publishing
dual-architecture RC pipeline recorded below. This is not a claim that every
application exposes complete or actionable accessibility semantics. No public
release or tag has been created. Core IR structural changes: **NONE**.

See the [issue ledger](validation/phase4c-issues.md), [machine-readable live
results](validation/phase4c/results.json), [limitations](limitations.md), and
[compatibility matrix](compatibility.md).

## Environment and method

Validation ran on 2026-09-01 in isolated OrbStack Ubuntu 24.04.4 aarch64,
Xvfb/X11 and AT-SPI 2.52, from a macOS arm64 host. Representative versions:
GTK3 3.24.41, GTK4 4.14.5, Qt 6.4.2, Mousepad 0.6.1, Chrome 152.0.7977.64,
Firefox 154.0.1, LibreOffice 24.2.7 and VS Code 1.135.0.

Every workflow used a private D-Bus session, Xvfb display, HOME/XDG directories
and application profile. Interactions used terminal events and public AT-SPI
objects. No DOM/CDP, UNO, toolkit-private API, application adapter, guessed
keyboard fallback or document parser was used.

## Closed gates

### Chrome Cache readiness and first-frame latency

Fresh browser sessions expose two causally distinct conditions. Five independent
samples were retained for each condition on the final Linux release binary:

| Condition | Cache items | First frame samples (ms) | Median |
| --- | --- | --- | ---: |
| Cache ready | 5,158 each | 223.23, 226.99, 215.27, 219.90, 226.68 | **223.23 ms** |
| Cache incomplete | 231, 5,142, 5,156, 231, 228 | 7,243.08, 3,722.65, 4,073.23, 7,303.74, 3,643.89 | **4,073.23 ms** |

Forced Cache diagnostics on incomplete samples reported one to three missing
reachable child references with zero invalid parent links. Small 228/231-item
inventories and near-complete 5,142/5,156 inventories both occur. Auto rejects
these inventories and performs the recursive correctness walk. A reachable
childless Document cache skeleton is also rejected generically; accepting it
previously allowed a fast but materially incomplete scene.

The old binary measured **4,054.31 ms** under the same fresh/incomplete setup.
The final median is 0.47% slower, far below the 20–25% regression threshold and
not an unexplained code regression. Historical ~200 ms data was the Cache-ready
condition, now reproduced at 223.23 ms. There is no fixed startup sleep, busy
poll, application-name branch, or incomplete-tree performance shortcut.

v0.1 expectation: large browser trees can require a several-second correctness
fallback while the accessibility cache is incomplete. Once all 5,158 records
are resident, bulk bootstrap remains about 0.2 seconds.

### Real list/settings-heavy workflow

LibreOffice Writer Options supplies the accepted real workflow (1,978 initial
nodes; 1,431 advertised-action nodes):

1. Open `Options...` from the semantic command hierarchy.
2. Select named `General` through the Options tree's parent Selection interface.
3. Toggle `Use data for document properties` through TUI, confirm the changed
   checked state independently with Inspector, then restore it.
4. Invoke `Cancel`, confirm the modal dialog disappears, and confirm the
   document context remains usable.

Result: **PASS**. PCManFM-Qt remains separate blocked compatibility evidence;
no product workaround was added.

### Real Qt application workflow

Qt Designer (311 nodes) completed a real workflow:

1. In New Form, invoke the named QVGA portrait option's advertised `Toggle` and
   independently observe it selected.
2. Toggle and restore `Show this Dialog on Startup`.
3. Invoke `Create`, then invoke the real `Form Settings...` command.
4. Observe the new dialog, close it with its advertised semantic action, and
   retain the created form context.

The Form Settings D-Bus action call timed out after 5 seconds even though the
dialog appeared. Acceptance therefore uses authoritative GUI/Inspector state,
not the RPC return alone. Result: **PASS** without a Qt/Designer production branch.

### Writer long-document boundary

The generated input contains 150 additional headings. AT-SPI exposed 1,984
nodes and the Reader model exposed **23 blocks / 7 headings**, marked
`PartialRealized`. Reader, Outline, indexed/progressive search and cancellation
remain usable. Presentation now says `Exposed semantic search` and
`Exposed semantic content exhausted (document coverage partial or unknown)`;
it never claims the entire source document was searched.

An ordinary `Go to Page...` semantic command was attempted. LibreOffice exposed
the page selector as `Slider value="1"`, for which v0.1 has no validated mutation
contract. GUI2TUI safely cancelled rather than injecting keys or guessing. The
model remained 23 blocks before and after, so no newly realized content was
fabricated. Result: **PASS WITH SAFE PARTIAL REALIZATION**.

## Final workflow matrix

| Application | Discovery | Controls / commands | Content | Lifecycle / modality | Result |
| --- | --- | --- | --- | --- | --- |
| Mousepad | PASS, 319 nodes | About command/dialog | multiline Reader/search | death, stale locator, fresh generation | PASS |
| GTK fixture | PASS, 44 nodes | edit/cancel/Button; checkbox read-only | password excluded | event cache | PASS |
| Qt fixture | PASS, 31 nodes | edit/cancel/Button/Checkbox/Choice | password excluded | quarantine resets per generation | PASS |
| Qt Designer | PASS, 311 nodes | Choice, form option, Create, Form Settings | unsupported complex widgets summarized | modal context | PASS for listed workflow |
| Chrome | PASS, 391-node fixture | anonymous actions refused; explicit diagnostic mutation only | Reader/Outline/search/table | reference zero-payload; death/restart | PASS for listed workflow |
| Firefox | PASS, 281 nodes | anonymous actions refused | Reader/Outline/search/table | unresolved modality degrades safely | PASS for listed workflow |
| Writer | PASS, 1,978 nodes | About and Options workflows | Reader/Outline/search | modal close/context restore | PASS for exposed content |
| Writer long | PASS, 1,984 nodes | safe page-navigation degradation | 23 blocks/7 headings, PartialRealized | no false completeness | PASS WITH SAFE PARTIAL REALIZATION |
| VS Code | PASS discovery, 367 nodes | anonymous Manage refused | Reader/search | isolated profile | PARTIAL by declared scope |
| Static GTK image | PASS, 3 nodes | explicit acquisition only | N/A | headless materialize + same-host viewer | PASS |
| PCManFM-Qt | prior probe only | bridge disconnected on repeat | N/A | no workaround | BLOCKED / P2 |
| Calc | NOT TESTED | NOT TESTED | NOT TESTED | NOT TESTED | optional, non-blocking |

## Security and regression

Final live runs passed Mousepad multiline-to-Reader, GTK/Qt single-line atomic
editing, password refusal, Qt Choice, Chrome/Firefox content workflows,
Writer/Writer-long, Qt Designer, Writer Options, VS Code and static acquisition.
Chrome headless reference inspection transferred zero payload. Generic unit
regressions pass for password redaction, anonymous-action refusal, stale identity,
unsafe capture geometry, denied endpoint payload and artifact ownership.

Automated tests: 225 library + 2 Inspector CLI + 3 user CLI = **230 tests**.
macOS and Linux both passed fmt, all-target check, all-target test, warnings-denied
clippy and `git diff --check`.

## Freeze and release pipeline

Production changes were limited to generic Cache completeness diagnostics/
fallback and honest partial-content presentation. Core semantic/content/scene/
modality IR structures are unchanged. A source search found no application or
toolkit name driving production behavior.

The final source commit and non-publishing GitHub dual-architecture pipeline are
recorded after the final run in this document's release evidence update. Public
release remains **NOT PUBLISHED** until an explicit release action.
