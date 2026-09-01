# Phase 4C final issue ledger

Observed in isolated Ubuntu 24.04 arm64/Xvfb sessions on 2026-09-01. Core IR
structural changes: NONE. P0 = 0. Unaccepted P1 = 0.

| ID | Final severity | Evidence | Disposition |
| --- | --- | --- | --- |
| C-01 | FIXED | Mousepad multiline TextInput incorrectly qualified for atomic single-line editing. | `bdcd251` makes multiline plain text Reader-only; live Mousepad and GTK/Qt single-line regressions PASS. |
| C-02 | P2 startup limitation | Final Chrome: complete 5,158-item Cache median 223.23 ms; incomplete 228–5,156-item Cache median 4,073.23 ms. Old binary same-condition median 4,054.31 ms. Diagnostics show 1–3 missing reachable children; empty Document skeleton also rejected. | Causally classified, no unexplained regression. Correct recursive walk retained. Large fresh browser trees may take seconds while Cache is incomplete. |
| C-03 | P2 compatibility/environment | PCManFM-Qt initially exposed directory/Preferences trees; later bridge reported `org.freedesktop.DBus.Error.Disconnected` despite matching accessibility bus addresses. | BLOCKED. No process-name branch, adapter, keyboard fallback or bridge workaround. Writer Options supplies the accepted real settings/list workflow. |
| C-04 | P2 safe degradation | Chrome/Firefox/Electron web controls can expose anonymous actions. | No semantic index-0 inference. Explicit Inspector index remains diagnostic-only. |
| C-05 | P2 accessibility limitation | Writer long input exposes 23 blocks/7 headings of 150 added headings and is `PartialRealized`; page selector is an unsupported Slider. | PASS WITH SAFE PARTIAL REALIZATION. Search wording is scope-honest; no UNO/parser/key injection. |
| C-06 | P2 coverage limitation | Designer has unsupported tree/layered-pane details. | Meaningful Choice/form/command/dialog workflow PASS; unsupported controls remain summaries. |
| C-07 | P2 declared partial | Real VS Code Reader/search works; Monaco editing is not validated. | Electron best-effort evidence retained; anonymous action refused. |
| H-01 | FIXED harness | Transient Loaded status and early identifier counting caused misleading reports. | Stable frame readiness, full tree-line counts and separate Cache-ready/fresh conditions. |

No observed wrong-target write/action, password exposure, product panic or false
whole-document claim remains. Allowed P2 items are safe, documented limitations.

## Deliberately not implemented

Multiline/rich editing, anonymous action inference, application adapters,
DOM/CDP, UNO, toolkit-private APIs, Wayland capture, remote transport, new TTY
attachment, Monaco editing and new semantic/modality kinds.
