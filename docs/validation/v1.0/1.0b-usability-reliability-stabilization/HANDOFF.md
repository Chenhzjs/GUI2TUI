# GUI2TUI v1.0B Usability, Reliability & Stabilization HANDOFF

## 1. Phase status

- Phase: **1.0B — Usability, Reliability & Stabilization**.
- Authorization: explicitly authorized by the user; 1.0C, RC, tag and
  release remain unauthorized.
- Result: **VALIDATED**. The formal requalification soak passed after the
  validation-only fresh-prompt and PTY EOF cleanup fix.
- Package version: `0.3.0`; no version change was made.

The phase fixed no production defect. It added only validation harnesses and
this evidence. The current product payload is therefore the same production
source as the validated 1.0A package; the new live harnesses build the current
source in disposable Linux containers and do not change product behavior.

## 2. Git and source identity

- Starting HEAD: `82129f847625c9c376a6c79c49b665b43da2211d`.
- Starting branch: `v1.0/integration-stabilization`.
- Starting worktree: clean.
- Final HEAD: `8778d0b9ad3cf6cf9a92d4b56da7fda9d385195a` for the validated
  production/validation source identity; this documentation update does not
  change the production payload.
- Remote: local branch remains ahead of `origin`; no push was performed.
- Published tags `v0.1.0`, `v0.1.1`, `v0.2.0` and `v0.3.0` were not moved.

No Rust production source, architecture, package version or release metadata
was changed. Validation-only changes are:

- `tests/live/v10b_terminal_recovery.py`: PTY terminal-size, resize, overlay,
  refresh and post-resize semantic-operation evidence.
- `tests/live/v10b_soak_scale.py`: bounded 45-minute installed-image soak and
  resource sampling.
- `tests/live/v07d_wayland_driver.py`: generic wait for the current public
  tree before starting the TUI, fixing a fixture-realization race in the
  validation driver rather than product behavior.

## 3. Campaign 1 — terminal and integrated recovery

### Terminal size and resize matrix

The installed user-prefix binary was driven through a real Docker PTY at:

| Rows × columns | TUI process | action text reachable | panic | result |
| --- | --- | --- | --- | --- |
| 48 × 160 | alive | yes | no | PASS |
| 32 × 100 | alive | yes | no | PASS |
| 24 × 72 | alive | yes | no | PASS |
| 16 × 60 | alive | yes | no | PASS |
| 12 × 50 | alive | yes | no | PASS |

The campaign also resized while the Choice presentation overlay was open,
cancelled it, resized again, executed `Reorder current items` through the
Command Palette, refreshed, and executed `Reset current items`. Fresh
Inspector readback confirmed `selected Gamma` and then `Selected: Alpha`.
The resize path did not create semantic identity, replay an operation, close
the modal incorrectly, or migrate authority. No terminal-too-small refusal
was needed in the tested lower bound; the smallest tested frame remained
alive and navigable.

### Navigation and fallback

Tab/arrow/Enter/Escape, Command Palette, modal cancel, Reader/chooser paths,
and the existing enhanced-key paths were covered by the current 1.0A package
evidence plus the 1.0B PTY campaign. No additional keyboard protocol layer was
needed. The Command Palette remained an alternate reachable entry for the
post-resize semantic operation.

### Integrated runtime campaign

The current source passed the existing v0.6E integrated harness in an isolated
Linux arm64 Xvfb/AT-SPI session. It ran one continuous TUI process through:

1. ordinary semantic operation and modal-scope refusal;
2. event storm and multi-window replacement with overflow resynchronization;
3. GTK → Qt → GTK3 application switching;
4. successful, conflicting and stale external-handler handoffs;
5. application loss, same-looking replacement and fresh explicit selection;
6. delayed operation retirement and stale-result refusal;
7. SIGTSTP/SIGCONT, GUI mutation while suspended, fresh post-resume scene;
8. normal exit and terminal restoration.

Observed markers were `INTEGRATED_CONTINUOUS_RUNTIME=PASS`,
`MIXED_SURFACE_EVENT_CONVERGENCE=PASS`,
`MIXED_EXTERNAL_TERMINAL_CONTINUITY=PASS`,
`NO_STALE_AUTHORITY_MIGRATION=PASS`, and
`TERMINAL_RESTORATION_AND_SINGLE_READER=PASS`.

The event-pressure portion proved bounded queue recovery from a public
Accessibility event storm and fresh-tree convergence; it did not use event
order as operation truth. Application replacement rejected old locators and
old results, including same-looking replacements.

Repeated external handoff was covered in the integrated lifecycle campaign by
multiple successful/conflicting/stale handoffs and by the existing installed
package Docker/SSH and Wayland external-handler evidence. Candidate files,
terminal ownership and handler reaping remained bounded. A separate
high-frequency handler benchmark was not run and is not needed for the phase
contract.

## 4. Campaign 2 — bounded soak and scale

The formal requalification soak used the current-source Linux headless image, an installed
user-prefix package layout, a managed Xvfb session, a real PTY, and the GTK
selection fixture. Duration was the required 45 minutes (`2700` seconds).
The workload was deliberately bounded and user-shaped:

- 1 initial Choice overlay open/cancel;
- alternating Command Palette reorder/reset operations every 30 seconds;
- fresh authoritative Inspector readback after every operation;
- a TUI refresh every fifth cycle;
- samples every 60 seconds;
- clean reset and TUI exit at the end.

The requalification reached 2,700.25 seconds and completed with exit code 0,
91 semantic operations, 18 refreshes and 46 resource samples. RSS ranged from
8,600 to 12,296 KiB and finished at 9,140 KiB. FD count remained 17, thread
count 13, child-process count 13, artifact files 0 and operation namespaces 0.
The shell exited only after a fresh post-quit prompt and explicit PTY EOF.

The integrated v0.6E resource samples independently showed:

- FD count: 19 at all sampled TUI points;
- threads: 14 at all sampled TUI points;
- zombies: 0;
- event queue depth: 0 at all sampled points;
- cache nodes equal cache locators at all sampled points;
- active operations: 0 at every sampled quiescent point;
- private text artifacts: bounded and preserved only for the intentional
  conflict/stale cases;
- modality artifacts: 0.

No linear FD, thread, child, artifact, queue, ticket or retired-generation
growth was observed. No periodic global cache purge was introduced.

### Current scale evidence

The current-source live scale points were the GTK selection fixture (small
tree), the v0.6 mixed GTK/Qt/GTK3 surfaces (7–49 semantic nodes at sampled
points), GTK Demo (91 nodes, recursive walk 64.924 ms), and Mousepad (319
nodes, current public tree acquired in the same isolated managed session).
The historical 5,158-node Chrome evidence remains useful architectural
evidence but was not relabelled as a new current-source 1.0B measurement.

No current-source 5k-node campaign was run in this phase. That is an evidence
boundary for 1.0C, not a discovered product failure. Partial realization,
node-count limits and timeout behavior remain governed by the existing
semantic/content contracts.

## 5. Performance baseline and proposed 1.0C budgets

This phase froze proposed, measurable budgets for final qualification; they
are not claims that every arbitrary application meets them:

| Measurement | Proposed 1.0C budget |
| --- | --- |
| Backend operation/readback | bounded by the configured default 5,000 ms backend deadline; no indefinite wait |
| Command Palette / ordinary TUI redraw | completes within 1 second after settled input on the tested core fixtures |
| Resize redraw | no semantic mutation or replay; current target remains reachable within 1 second on supported sizes |
| Event-overflow recovery | fresh public-tree convergence within the configured bounded transition/operation timeout, with explicit status on timeout |
| 45-minute soak | no monotonic FD/thread/child/artifact/queue growth; quiescent operation count returns to zero |
| Large/partial tree | no fabricated unrealized content; complete vs partial status remains explicit; timeout/truncation remains bounded |
| Resource high-water | allocator warmup is allowed; unexplained growth correlated with cycle count is a release blocker |

Application/backend latency is to be reported separately from GUI2TUI
processing. 1.0C must repeat the small/medium/current and, if available,
large-tree measurements on the exact candidate source rather than turning the
proposed budgets into retrospective PASS claims.

## 6. Failure UX and recovery guidance

Source audit and live recovery evidence found actionable existing wording,
with no production change required. Examples include:

- stale operation: refresh and choose from the current interface;
- application disappearance: tasks discarded, with F5/search, application
  selection, diagnostics and quit paths;
- Accessibility/session failure: retry or run Doctor rather than treating it
  as a control failure;
- unsupported public semantics: safe refusal rather than success;
- password editing: explicitly disabled;
- external handler failure/conflict: candidate remains private and the GUI
  remains authoritative.

Messages do not expose passwords, candidate text, tokens, SSH keys, bus
addresses or unnecessary private paths. No new P0/P1 failure-UX defect was
reproduced.

## 7. Headless environments and geometry

Current-source representative results:

| Environment | Result |
| --- | --- |
| Managed Xvfb / Docker interactive PTY | PASS within v0.7 boundaries |
| Same-host real SSH → Managed | PASS within v0.7 boundaries |
| Weston headless/Pixman native Wayland | QUALIFIED WITH EXPLICIT LIMITATIONS |
| Weston XWayland | QUALIFIED WITH EXPLICIT LIMITATIONS |
| Wayland static capture | UNSUPPORTED / DEFERRED |
| Wayland over real SSH | NOT TESTED |
| Linux native VT | NOT TESTED |
| ordinary desktop compatibility | OPTIONAL / NOT TESTED |
| native x86_64 | not available; retained `QUALIFIED WITH EMULATION LIMITATION` |

The Wayland campaign exercised GTK and Qt identity/semantics, operation and
readback, resize, dynamic refresh, external text, restart and stale locator
rejection. It also exercised collapsed geometry fallback for native Wayland
and XWayland. No false positive was observed in the shared-origin rejection
heuristic; no heuristic change was justified.

The x86_64 status was not silently promoted: this arm64 host has no native
x86_64 evidence, and no infrastructure or permission expansion was made.

## 8. Real-application sanity

In a separate disposable managed container, two distro applications were
started without application-specific GUI2TUI logic:

- GTK Demo: 91-node public tree, ordinary GTK4 application semantics;
- Mousepad: 319-node public tree with menus, actions and text-oriented
  structure.

Application enumeration and public semantic acquisition passed. This was a
sanity corpus, not a product support list. A full common-task completion
campaign against those applications was intentionally not added to 1.0B;
their Accessibility exposure remains the authority and any absent semantics
remain an application limitation.

## 9. Architecture, genericity and security audit

No new architecture layer is required. The existing path remains:

```text
AT-SPI -> semantic cache/content -> topology/presentation
       -> TuiScene/SceneBinding -> user UiIntent
       -> verified backend operation -> fresh authoritative readback
       -> current scene and manual continuation
```

No WorkflowEngine, TaskSession, DeploymentManager, SessionManager,
ApplicationManager, new backend, application/toolkit adapter, OCR, screenshot
semantic inference, keyboard/mouse injection, coordinate click, action-index
guess, fuzzy matching or backing-file bypass was added. Resize and narrow
layout changed presentation only. Password exclusion, private candidate
ownership, terminal ownership, generation isolation, stale refusal and
authoritative readback invariants remain unchanged.

## 10. Severity and remaining evidence boundaries

### P0

None reproduced. No wrong-target mutation, authority migration, private
content exposure, unsafe artifact, non-recoverable terminal state or false
success was observed.

### P1

None reproduced. Core Managed/Docker/SSH/Wayland representative flows,
integrated recovery, resize, soak and resource ownership passed within their
declared limits.

### P2 / known limitations

- Native x86_64 remains emulation-limited on the current host.
- Native VT, ordinary desktop compatibility and Wayland over SSH remain
  untested and are not support claims.
- The current-source large-tree measurement is bounded to the available
  319-node real application plus historical 5k evidence; exact 1.0C candidate
  package scale numbers remain to be repeated.
- Real applications are sanity evidence, not an exhaustive compatibility
  matrix; public Accessibility omissions remain safe limitations.

These are documented evidence/support boundaries, not hidden product claims.

## 11. Qualification package and quality matrix

Because no production Rust payload changed, the final 1.0A qualification
package remains the applicable package artifact:

- version: `0.3.0`;
- architecture: aarch64;
- production source commit: `4f90f09621ac72965b8eff8d4db1c6f41fafbdc6`;
- archive SHA-256:
  `4da7ef82d0ef8bd657ba22fe0f8b7638d737f21008eff2e0978113361b621e57`;
- ABI: glibc max 2.34 as recorded by the package ABI report;
- installed prefix: isolated unprivileged user prefix outside the checkout.

The 1.0B live images rebuilt the unchanged production payload from the
current source identity `82129f847625c9c376a6c79c49b665b43da2211d` plus
validation-only working-tree harnesses. No production payload divergence was
introduced.

Quality matrix executed once against the current source before documentation
close:

```text
cargo fmt --all -- --check                         PASS
cargo check --all-targets --locked                 PASS
cargo test --all-targets --locked                  PASS
  300 library tests, 2 inspector tests, 5 user CLI tests; 0 failed
cargo clippy --all-targets --locked -- -D warnings PASS
python3 scripts/check-docs.py                      PASS (95 files, 328 links)
git diff --check                                   PASS
```

The final documentation close must rerun `python3 scripts/check-docs.py` and
`git diff --check` after this file and roadmap updates.

## 12. Proposed v1.0 support contract for final qualification

This is proposed for 1.0C; it is not yet frozen:

- Supported core topology: unprivileged Linux user-prefix install, Managed
  Xvfb, user session D-Bus/AT-SPI, Docker interactive PTY, and same-host SSH
  into the Managed session, within the recorded v0.7 environment contract.
- Architecture: aarch64 qualified by package/live evidence; x86_64 remains
  package/emulation-limited until native evidence exists.
- Terminal: a real interactive PTY with ordinary Tab, arrows, Enter, Escape,
  resize and the documented Command Palette fallback; very small terminals
  must retain safe navigation or report an explicit bounded limitation.
- Wayland: headless Weston/Pixman and XWayland representative support with
  geometry limitations; no static-capture, Wayland-over-SSH or broad desktop
  claim.
- Accessibility: only public AT-SPI semantics and explicitly advertised
  capabilities qualify. Partial realization, missing selected/current truth,
  rich/secret text and pointer-only/private semantics degrade safely.
- Stability: no monotonic descriptor/thread/child/artifact/queue growth in the
  proposed bounded soak; no stale-generation publication.
- Performance: default backend deadline 5 seconds, one-second settled
  redraw/resize target on tested core fixtures, bounded tree traversal and
  explicit partial/truncation state. Exact large-tree candidate numbers remain
  a 1.0C check.
- Deferred/unsupported: Remote Companion, native VT, broad desktop matrix,
  arbitrary drag/drop, rich-text fidelity, exhaustive virtualization,
  pointer-only context menus, and application-specific adapters.

## 13. Final status and precise 1.0C handoff

`PHASE 1.0B USABILITY, RELIABILITY & STABILIZATION VALIDATED`

The recommended next scope is **1.0C — Final Qualification & Release
Readiness**, which remains separately unauthorized. It must freeze the support
contract, repeat exact candidate
package provenance and large-tree measurements, audit final documentation,
and produce an RC qualification conclusion. It must not create an RC, tag or
Release without separate authorization.

1.0C — Final Qualification & Release Readiness remains unauthorized and was
not started.

No 1.0C work was started by this phase.
