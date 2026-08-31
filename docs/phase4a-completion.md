# Phase 4A completion evidence

Status: **PHASE 4A RUNTIME & RECOVERY HARDENING VALIDATED**.
Architecture: **CORE ARCHITECTURE FROZEN FOR v0.1**.
Core Semantic/Region/Scene/Command/Choice/Content/Table/Modality resource IR changes: **NONE**.
All changes are ownership/recovery plumbing, redacted metrics, tests and documentation.

## Reproduction

Linux commands below require an isolated Xvfb/AT-SPI session, Python pexpect/pyte and the existing
fixtures. Use a disk-backed target directory: `/tmp` on the validation VM is a bounded tmpfs.

```bash
export CARGO_TARGET_DIR=/home/chenhz/gui2tui-p4a-target
DISPLAY_NUMBER=:116 SOAK_SECONDS=1805 bash scripts/phase4a-soak-linux.sh
DISPLAY_NUMBER=:120 bash scripts/phase4a-daemon-recovery-linux.sh
DISPLAY_NUMBER=:114 TEST_PAYLOAD_TRANSFER=1 bash scripts/phase4a-endpoint-loss-linux.sh
DISPLAY_NUMBER=:118 bash scripts/phase4a-reader-terminal-linux.sh
DISPLAY_NUMBER=:123 bash scripts/phase4a-qt-quarantine-linux.sh
```

Display numbers must be unused. Harness results are printed as `RESULT_DIR=...`. No harness
targets the host user's desktop accessibility services. Debug-only failpoints/paced transport
are absent from release builds. No production crash key or remote transport was introduced.

## Accessibility daemon: real death, held absence, fresh generation

Run `/tmp/gui2tui-p4a-daemon-rDnVuv` queried the real `org.a11y.Bus` owner and launcher PID,
then SIGKILLed its accessibility dbus-daemon child and launcher. An isolated XDG_DATA_HOME service
wrapper inhibited activation for **5.145 seconds**; `NameHasOwner(org.a11y.Bus)=false` was checked.
This is not merely a missing fixture with a healthy daemon.

The TUI survived, invalidated generation 1 immediately and retained only a read-only view.
There were four attempts with 100/200/400/800 ms backoff; attempts did not increase during the
subsequent 1.5-second observation. Removing the inhibition restored actual AT-SPI activation.
Restarting the fixture and pressing F5 created generation 2, rejected the saved old locator,
and a real Click was independently confirmed as checked checkbox plus activated status.

## Standalone artifact crash windows

One shared `OwnedArtifactDirectory` contract is used by both broker and materializer. No second
scavenging protocol exists. Filename registration precedes file creation; incomplete updates use
a known `ownership.pending` metadata filename. Recovery validates every entry before deleting any.

The subprocess tests force `exit(86)` without Rust destructors (not an unwind or synthetic success).
For each window, startup recovers exactly one abandoned namespace and leaves no registered payload.

| Failpoint | Observed crash boundary | Recovery |
| --- | --- | --- |
| A | Owned directory constructor complete, no payload filename | PASS |
| B | Payload filename registered durably, no payload file | PASS |
| C | Empty payload file created | PASS |
| D | First 65,536 bytes of a 131,072-byte artifact | PASS |
| E | Complete payload, no materialization metadata/completion | PASS |
| F | Completion marker written, before returning to caller | PASS |

These test process death, not power-loss durability. A raw mkdir failure before the directory
becomes an initialized owned namespace cannot have a payload; unidentified directories are never
claimed or recursively scavenged. Foreign/unregistered entries, symlinks and hardlinks cause refusal;
another live session/reaper lease causes a skip. The TTL reaper only recovers its exact directory.

## Endpoint interruption

Production same-host RenderedSnapshot Open still passes a local file reference with zero network
payload. Its real broker-kill-while-awaiting-handler test passed separately.

For the stronger byte-transfer gate, the debug-only harness paced the existing `send_artifact`
protocol using an **actual GTK-acquired RenderedSnapshot**, not a fabricated producer. In
`/tmp/gui2tui-p4a-endpoint-YujDDY`, the broker was SIGKILLed after partial payload transmission;
the producer stopped after reading **160 bytes**. TUI remained responsive, reported EndpointLost,
active operations returned to zero, no Opened result appeared, and the semantic scene remained valid.
The next broker advertised `image/png` instead of `image/*`, required a new authorization and
accepted a fresh handoff. No old broker grant was reused.

Endpoint disconnect cancels endpoint-owned operation tickets. A unit test injects late success
after reconnect: rejected, `rejected_late_results=1`; a fresh operation succeeds. This late-result
injection is **UNIT VERIFIED**, not a claimed live forged response.

## Reader and terminal

Run `/tmp/gui2tui-p4a-reader-Y1h4Xe` used Chrome's real accessible document:

- Non-first Reader block **244 → 244** across SIGUSR1/SIGUSR2 while document events occurred.
- Removing the selected paragraph while detached: old block **274 → valid fallback 241**.
- Debug controlled panic after raw/alternate-screen entry restored **ICANON, ECHO, cursor visible,
  alternate-screen exit**. SIGKILL terminal restoration is impossible and is not claimed.

Reader restoration uses ContentBlockId, not terminal row. Scene/materialization rebuild now clears
a deleted Reader position instead of retaining an invisible stale block ID.

## Deployment boundary

| Deployment | Status |
| --- | --- |
| Headless semantic TUI/materialization | SUPPORTED |
| Same-host graphical endpoint | SUPPORTED |
| Same-process same-PTY detach/resume | LIVE VERIFIED |
| New terminal process/new TTY attachment | NOT IMPLEMENTED; not a v0.1 gate |
| Remote companion | Architecture-ready; production transport NOT IMPLEMENTED |
| Wayland static acquisition | NOT IMPLEMENTED |
| Individual real slow Accessible fixture | NOT TESTED |

## Application regressions

| Test | Actual result |
| --- | --- |
| GTK controls (`phase4a-live-linux.sh gtk`, `CONTROLS_ONLY=1`) | Plain edit commit/cancel, Button, Password refusal, non-actionable Checkbox PASS |
| GTK event storm / lifecycle | 12,004 received, 2,844 dequeued, 9,160 dropped, high-water 2,048, exactly 1 resync; authoritative final GUI state confirmed; 3 generations / 28.119 s |
| Qt controls (same harness, `qt`) | Plain edit commit/cancel, Button, Password refusal, Checkbox toggle, Choice Beta PASS |
| Qt generation churn | 3 generations / 18.092 s, stale IDs rejected, terminal restored |
| Qt Text quarantine | Quarantined in generation 1, fresh Declared/non-quarantined capability in generation 2 |
| Chrome content (`phase3f-live-linux.sh chrome`) | 399 nodes; real Reader/indexed and progressive search; 3×3 and 3×2 tables; keyboard cell navigation PASS |
| Chrome lifecycle | 2 generations / 15.911 s, detach/events/resize and stale old generation PASS |
| Chrome reference (`phase3g-live-linux.sh chrome`) | reference_only=1, artifact_bytes=0, recorded_invocations=1 |
| Chrome large startup (`FAST_BENCHMARK=1 ... chrome-large`) | 5,166 nodes, Cache path 465/513/469 ms, median 469 ms; RSS 42,252/42,244/42,212 KiB |
| LibreOffice content | 1,976 nodes, 17 content blocks, 4 headings; Reader/search; Table 2×1 exposed and sampled |
| LibreOffice command/exit (`OFFICE_RECOVERY=1 phase3g-live-linux.sh libreoffice`) | TUI command search → About → Close; actual GUI exit → TUI remains alive PASS |
| LibreOffice embedded image | 1,963-node document; no trustworthy reference → unavailable, artifact_bytes=0, no invocation |
| Broker independent SIGKILL regression | 4,096 partial bytes, exactly one namespace recovered, active broker untouched, clean SIGTERM |

Qt's ComboBox accessible **owner name stays Alpha**, while its child `ListItem "Beta"` becomes
`[selected,transient]`. The TUI correctly derives `Choice: Beta` from selection, not the stale owner
name. The first regression assertion incorrectly expected the owner to be renamed; the saved
independent tree corrected the test expectation, not production semantics.

## Soak and quality

The second mixed soak (`/tmp/gui2tui-p4a-soak-s1216Z`) completed **1,808.591 seconds** independently
of the first. It exercised the shared ownership/reaper and endpoint-ticket recovery code; the
subsequent backend-unavailable mouse/search guards were separately verified in the final daemon
run. No soak failure was patched and joined to a shorter run. Both full durations stand alone.
The harness samples RSS/FD/thread count, cache nodes, queue depth, active operations, artifacts,
endpoint and generation every 30 seconds, flushing JSONL immediately.

| Time (s) | Generation | RSS (KiB) | FD | Threads | Operations | Artifacts | Queue |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 3.712 | 1 | 17,232 | 17 | 13 | 0 | 0 | 0 |
| 930.111 | 13 | 14,464 | 17 | 13 | 0 | 0 | 0 |
| 1,808.590 | 25 | 15,060 | 17 | 13 | 0 | 0 | 0 |

Final counts: **25 generations, 24 detach/resumes, 61 resizes, 12 endpoint restarts, 160 semantic
actions, 60 inspector search/content operations, 7 dialogs, 6 materializations, 6 references,
1 overflow/resync episode**. The main TUI's initial edit was unconfirmed (count=0); **8 independent
peer TUI edits** during this same soak interval were read-back confirmed and separately recorded.

All 62 samples held FD=17 and threads=13. Final operations, artifacts and queue were zero. RSS
ranged 12,236–17,232 KiB; after the five-minute warm-up its linear-fit slope was **+0.336 KiB/min,
R²=0.0001**. Last-half slope was +24.533 KiB/min with R²=0.1591, amid non-monotonic fluctuations.
There is no observed sustained approximately linear RSS growth in this window. This is a finite
thirty-minute conclusion, not a claim of mathematically bounded memory for arbitrary uptime.
Tokio task count is not separately observable; OS thread count and operation registry were sampled.

Machine-readable evidence: [analysis](validation/phase4a-soak-final.json),
[all 62 samples](validation/phase4a-soak-final.csv), [peer edits](validation/phase4a-edit-peer.jsonl),
[daemon identities and backoff](validation/phase4a-daemon.json).

The soak's main runtime uses GTK controls. Search/content and modality operations also invoke
independent inspector/local-handler clients against the same isolated session; it is not a claim
that the controls fixture supplies a document Reader. Reader is independently live-tested above.

macOS and Linux: fmt/check/test/clippy/diff checks PASS at the current code revision;
**203 library + 2 inspector = 205 tests**, compared with baseline 199. Linux release all-target
check also passed. All runtime statistics exclude GUI body, input values, passwords,
queries, credential URIs and payload.

A completed earlier full run (`/tmp/gui2tui-p4a-soak-PDz6xj`) lasted **1,808.775 s**, with 25
generations, 24 detach/resumes, 61 resizes, 12 endpoint restarts, 160 actions, 60 inspector
search/content operations, 7 dialogs, 6 materializations, 6 references and one overflow. Its 62
samples had constant FD=17 and threads=13. RSS first/middle/last was 17,324/19,184/18,420 KiB;
late-half linear-fit slope was -76.399 KiB/min. This completed run predates the last recovery-path
guard fixes; the second completed run exercises the updated ownership path. Neither duration is
added to the other. Its attempted initial edit was **not confirmed (count=0)**.

The second run also permits a separately counted EditableText peer (`phase4a_soak_edit_peer.py`)
against its same isolated GUI application. This peer uses another real TUI and independent
Inspector read-back; it never borrows the main runtime's PTY. Races with planned application
replacement are logged as failed attempts, not counted as writes, and the peer discards the old
client before selecting a fresh generation.
