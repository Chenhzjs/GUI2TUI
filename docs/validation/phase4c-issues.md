# Phase 4C issue ledger

Observed 2026-09-01 in isolated Ubuntu 24.04 arm64/Xvfb sessions. A passing
workflow is not a claim of complete application support. Production source
changes are limited to the generic fix below; core IR structural changes: NONE.

| ID | Severity / class | Reproduction and evidence | Disposition |
| --- | --- | --- | --- |
| C-01 | P1 / A+B | Mousepad 0.6.1 exposes its document as editable, multi-line TextInput. Before the fix, Enter opened the atomic single-line editor; no write was performed. Content catalog was empty. | FIXED in `bdcd251`: reject MultiLine in walk/cache edit capability and backend write/read validation; recognize plain multi-line Text as read-only Reader content. Three new unit tests; actual Mousepad Reader/search/About/restart PASS. |
| C-02 | P1 / C, attribution incomplete | Chrome 152, 5,158 nodes: Cache.GetItems sometimes returns ~230 chrome-only records, sometimes ~5,148 records, but ordinary reachable records advertise uncached children. Forced cache errors; Auto correctly walks. Warm first frame 4.202 s; previous binary in the same fresh-session setup 4.054 s. | OPEN performance acceptance gate. Not introduced by C-01. An isolated `--no-sandbox` comparison also falls back (3.772 s); disabling sandbox is NOT a fix or installation recommendation. Do not repeat the historical 200 ms claim as a current fresh-profile result. No incomplete tree accepted for speed. |
| C-03 | P2 / C+D, cause not established | PCManFM-Qt 1.4.1 directory and Preferences probes exposed real trees (settings 176 nodes). Repeated isolated workflow runs subsequently did not register on AT-SPI within 30 s; profile/new-window/daemon/preferences launch variants tried. | PARTIAL / workflow BLOCKED. X11 Preferences really exists. Bridge debug logs report `org.freedesktop.DBus.Error.Disconnected` / `Not connected to D-Bus server`; X11 and session accessibility bus addresses agree. Pre-activating GetAddress + Registry Ping did not fix it. `aboutToQuit` at harness teardown is not evidence of spontaneous exit. No production workaround or toolkit adapter. |
| C-04 | P2 / C | Chrome/Firefox expose anonymous web-control actions. Firefox's first selected modality has no resolvable reference; Chrome's linked-image reference resolves. | Deliberate safe degradation. No action-index fallback; explicit Inspector index in controlled browser mutation tests is diagnostic-only. No fake Open in either headless TUI. |
| C-05 | P2 / C | Writer long input document has 150 added headings, but the observed content projection exposes 17 blocks/4 headings and marks PartialRealized. | Partial Reader/search is real; whole-document coverage NOT VALIDATED. No parser/UNO extraction or fake completeness. |
| C-06 | P2 / C | Qt Designer exposes many tree/cell/layered-pane objects outside supported operations. Closing New Form and browsing command results works. | Read-only Unsupported summaries; full form-design workflow NOT VALIDATED. |
| C-07 | Environment, recovered | `/tmp` tmpfs was full during initial build/download; official VS Code Linux arm64 download timed out after bounded transfers. | Own new build/download/evidence moved to `/var/tmp`. Historical directories were not erased. Resumed download completed: real VS Code 1.135.0 Reader/search and anonymous-action refusal PASS, full editor compatibility PARTIAL. |
| H-01 | Harness | Polling `--list` before an app registers, extra Esc after Reader close, and assuming the transient Loaded status persists caused false failures. Early node counts counted printed IDs rather than every tree node. | Harness corrected: bounded empty-list polling, two-level exit, stable interactive-frame readiness, full tree-line counts. Curated `results.json` corrects early counts from saved trees. Failed attempts are not silently promoted to passes. |

## Classification policy

A = generic implementation bug; B = generic heuristic weakness; C = accessibility
limitation; D = application-specific quirk without adapter; E = explicit non-goal.
No observed P0 (wrong target write/action, password exposure, or product panic) in
the completed tests. C-02 remains unaccepted P1: **NOT READY TO RELEASE v0.1.0**.
The remaining verification gaps are not recategorized as passing limitations.

## Deliberately not implemented

Multiline editing, anonymous action inference, application-name branches,
DOM/CDP, UNO/document extraction, new IR types, cross-host transport, compositor,
and new-TTY attachment. These remain outside the frozen v0.1 scope.
