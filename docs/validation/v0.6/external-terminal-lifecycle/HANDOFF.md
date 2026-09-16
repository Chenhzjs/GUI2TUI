# GUI2TUI v0.6 Phase 0.6D External and Terminal Lifecycle Handoff

## 1. Status

- Phase: 0.6D External and Terminal Lifecycle.
- Starting HEAD: `046bc36c757ef83475a1878e18d6d17f3966f668`.
- Final HEAD: the evidence/docs commit containing this handoff; the immediately
  preceding production/tests commit is
  `3d1121a27957c7e3b02465a14f9c06aa3665ad94`.
- Branch: `v0.6/runtime-continuity-multi-surface`.
- Worktree: clean at final handoff.
- Production changes: bounded terminal signal transitions, asynchronous direct
  child ownership, two-stage handler shutdown, and private namespace bounds.
- New identity layers: none.
- P0: none.
- P1: none.
- Overall: **PHASE 0.6D EXTERNAL AND TERMINAL LIFECYCLE VALIDATED**.

---

## 2. Commits

- Production/tests: `3d1121a` — `fix: harden external and terminal lifecycle`.
- Evidence/docs: `docs: validate v0.6 external terminal lifecycle` (the commit
  containing this handoff).

---

## 3. Terminal ownership model

- Owner: one `TerminalGuard` and one current Crossterm `EventStream` while the
  TUI is attached.
- TerminalGuard: attachment is the sole production acquisition path; detach is
  idempotent and Drop calls detach.
- Raw mode: enabled on attach and disabled by shared restoration.
- Alternate screen: entered on attach and left by shared restoration.
- Cursor: hidden on attach and shown by shared restoration.
- Reader: one stream while attached, unpolled while detached, replaced once on
  external return, explicit reattach, or resume.
- Detach: stops TUI input polling before restoring terminal modes.
- Reacquire: attaches modes, installs one fresh stream, invalidates the terminal
  frame, redraws, and refreshes semantics after suspension.
- Drop: safely repeats the same detach path.
- Panic: the panic hook restores before delegating to the prior hook; Drop may
  repeat restoration during unwind.
- Signals: Tokio signal streams deliver SIGINT, SIGTERM, SIGTSTP, and SIGCONT;
  no complex work occurs in an async-unsafe handler.
- Suspend: SIGTSTP restores first, then SIGSTOP actually suspends; SIGCONT
  resumes attach, redraw, one reader, and fresh semantic convergence.

---

## 4. Normal and signal exit

- Normal exit: `q` stopped runtime work and restored canonical input and echo.
- SIGINT: actual SIGINT followed the orderly shutdown path with exit status 0.
- SIGTERM: actual SIGTERM produced the equivalent safe teardown.
- Shutdown ordering: stop input/operations, retire application-owned work and
  its event producer, restore through `TerminalGuard`, then exit.
- Terminal result: ICANON and ECHO were set after all three exits.
- Runtime resources: event producer reported a graceful exit; settled tickets
  and external children were zero.
- Result: **PASS**.

---

## 5. Controlled panic

- Harness: debug-only hidden `--test-panic-after-attach` in a PTY.
- Terminal acquired: yes, before the deterministic panic point.
- Panic point: after raw mode, alternate screen, cursor, and keyboard setup.
- Hook: shared `restore_terminal()` ran before the ordinary Rust panic hook.
- RAII: `TerminalGuard` Drop repeated the idempotent detach during unwind.
- Terminal attributes after exit: canonical input and echo restored.
- Alternate screen/cursor: alternate screen left and cursor shown.
- Result: **PASS** for ordinary Rust unwind. `panic=abort`, cleanup double
  panic, corruption, SIGKILL, and power loss are outside the supported claim.

---

## 6. Suspend/resume policy

- Policy: **SUPPORTED** on Linux.
- SIGTSTP: received through Tokio, followed by explicit SIGSTOP only after
  terminal release.
- Terminal before suspension: canonical input and echo restored; alternate
  screen left and cursor visible.
- Reader: not polled while stopped.
- SIGCONT: resumed the stopped transition.
- Terminal reacquisition: modes and exactly one fresh `EventStream` acquired.
- Fresh redraw: alternate-screen entry and immediate full frame observed.
- Fresh semantic state: ordinary full reload ran without manufacturing a new
  application generation.
- Fresh operation: `Lifecycle activate` succeeded after resume and resize.
- Result: **PASS**. The GUI may change while stopped, so resume never treats the
  pre-suspend scene as current truth.

---

## 7. Terminal reader lifecycle

- Creation: once on initial TUI attachment.
- Normal owner: the attached TUI loop.
- External detach: the stream is dropped before spawning the configured child.
- Handler: GUI2TUI consumes no terminal input while the handler owns it.
- Reacquire: one new stream after child wait/reap.
- Suspend: reader is unpolled after terminal detach.
- Resume: one replacement stream.
- Shutdown: no reader survives the product process.
- Repeated-cycle reader count: logical `1 -> 0 -> 1` for each of four cycles;
  stable FD observation was `19 / 19 / 19` before/peak/final.
- Result: **PASS**.

---

## 8. External handler process model

- Spawn: `tokio::process::Command`, direct executable plus configured arguments.
- argv: exactly one standalone `{file}` becomes the owned candidate path.
- Shell: none; shell `-c` policy remains rejected.
- Child ownership: the local `ExternalTextHandler` owns one `Child`.
- Wait: asynchronous foreground wait, integrated with runtime signals and app
  liveness.
- Cancellation: GUI target loss revokes authority but does not kill the editor.
- Generation invalidation: child may finish; final ticket/locator checks refuse
  publication and preserve modified data privately.
- Shutdown: first stop retires all semantic/background authority and waits for
  the foreground editor; a repeated stop explicitly forces product exit and
  preserves the file under its bounded TTL.
- Exit/reap: normal and first-stop completion paths await the child exactly once.
- Zombie behavior: none across four normal cycles, stale return, conflict, or
  deferred shutdown.

---

## 9. External handler terminal handoff

- Before: TUI has raw mode, alternate screen, hidden cursor, and one reader.
- Detach: reader dropped and shared terminal restoration invoked.
- Handler ownership: fixture observed canonical input and echo.
- Return: child status awaited and reaped.
- Reinitialize: `TerminalGuard::attach` reacquired all product terminal modes.
- Reader: exactly one new stream.
- Redraw: TUI frame redrawn.
- Fresh semantics: event drain/full semantic synchronization precedes final
  authority decisions.
- Result: **PASS**.

---

## 10. Repeated external handoff

- Cycles: 4.
- Reader: one attached, zero during handler, one on every return.
- FDs: `19 / 19 / 19` before/peak/final.
- Children: one during each handler, zero settled.
- Artifacts: one leased during edit, zero after each successful verified write.
- Tickets: one current ticket per edit, zero settled.
- Terminal: canonical/echo in handler, raw TUI after return, restored on exit.
- Fresh operation: a current Action succeeded after the fourth cycle.
- Result: **PASS**.

---

## 11. Handler active during target/app loss

- Target: qualified complete plain multiline `TextInput`.
- Generation: G1 only.
- Handler: held at a deterministic barrier after modifying the candidate.
- Loss: G1 application exited and a same-named replacement registered.
- Ticket: retired with G1.
- Child: allowed to finish and reaped; no automatic termination.
- Candidate: retained privately as non-authoritative user data.
- Writeback: none.
- Artifact: one regular 0600 candidate inside a 0700 namespace.
- Fresh app: replacement retained its own original text.
- Result: **PASS**.

---

## 12. Handler active during shutdown

- Handler: foreground, modified, and held open.
- Shutdown: first SIGTERM immediately retired the generation, ticket, event
  producer, and publication authority while leaving the editor usable.
- Child policy: defer once for user completion and reap; repeated SIGTERM is an
  explicit escape for a handler that never exits.
- Terminal: already transferred in canonical/echo mode; no competing TUI reader.
- Artifact: completed deferred shutdown preserves the stale candidate for
  ordinary startup recovery; forced exit uses a private expiry-deferred lease.
- Write authority: revoked on the first signal.
- Process result: completed child reaped; repeated-signal escape exited without
  an infinite product wait.
- Result: **PASS**.

---

## 13. External-text final authority

- Ticket: current final completion ticket required.
- Generation: must still equal the session generation.
- Locator: exact current `BackendLocator` required.
- Scope: exact active scope and allowed target required.
- Readback: current text conflict check, backend write acceptance, and resulting
  authoritative text equality required.
- Handler exit: success only allows candidate processing; it grants no GUI
  authority.
- Stale result: preserved privately when modified, never published as success.
- Replacement: never found by label, role, geometry, index, PID, or
  `RuntimeNodeId`.
- Result: **PASS**.

---

## 14. Artifact ownership model

- Namespace: exact GUI2TUI UID-owned runtime root and marked operation directory.
- Directory permissions: 0700.
- File permissions: 0600 and single hard link.
- Lease: shared live flock; recovery requires exclusive flock.
- Bound: 256 operation namespaces, 256 files per namespace, TTL capped at 1,800
  seconds, and external candidates capped by the existing text byte limit.
- Success cleanup: file and namespace removed after verified write.
- Stale/conflict preservation: modified candidate retained as private data.
- Shutdown: first-stop completion follows ordinary preservation; repeated-stop
  continuation is marked unavailable to startup recovery until expiry.
- Startup recovery: scans only the exact owned root and validates marker, UID,
  permissions, lease, manifest, links, and registered entries.
- Authority: artifact data never authorizes a target or workflow resume.
- Symlink/foreign-file safety: no-follow opens and conservative refusal; no
  recursive/glob deletion of unknown entries.

---

## 15. Artifact lifecycle evidence

- Successful edit: zero candidate artifacts after each verified write.
- Stale edit: one private candidate, no replacement write.
- Conflict: concurrent authoritative GUI value B won; candidate C was preserved
  and never overwrote B.
- Shutdown: modified candidate stayed available while needed and remained
  non-authoritative.
- Startup recovery: ordinary unlocked stale/conflict/shutdown residue was
  removed; expiry-deferred active-handler residue was skipped until expiry.
- Count/bound: synthetic namespace cap and expiry test passed; live cycles did
  not accumulate success artifacts.
- Permissions: source and live `lstat` verified 0700/0600.
- Result: **PASS**.

---

## 16. External Modality lifecycle

- Identity: existing `ExternalModalityId` only.
- Authority: acquisition/presentation identity, never operation authority.
- Capture: one explicit bounded static materialization task/ticket.
- Origin: exact current runtime owner and locator.
- Origin disappearance: generation invalidation cancelled active work and
  cleared the current modality view.
- Application loss: event producer and modality tickets retired.
- Artifact: none survived the exercised loss.
- Count: maximum 8 current session artifacts.
- TTL: 300 seconds; per-artifact size also remains bounded.
- Shutdown: cancellation removes publication authority and owned artifact RAII
  performs cleanup.
- Result: **PASS**.

---

## 17. Process/resource observations

Before / peak / final:

| Resource | Before | Peak | Final |
| --- | ---: | ---: | ---: |
| terminal readers | 1 | 1 | 0 after exit |
| external children | 0 | 1 | 0 on reaped paths |
| zombies | 0 | 0 | 0 |
| FDs, repeated handoff | 19 | 19 | 19 before product exit |
| tickets | 0 | 1 per operation | 0 settled |
| candidate artifacts | 0 | 1 active/preserved case | 0 success path |
| modality artifacts | 0 | 0 in origin-loss case | 0 |
| threads | not sampled | not sampled | not sampled |
| RSS | not sampled | not sampled | not sampled |

Logical ownership, ticket retirement, child wait/reap, FD stability, and
artifact bounds are the acceptance basis.

---

## 18. Shutdown ordering

- Input: stop accepting TUI input; detach reader before terminal transfer.
- Operations: invalidate generation and cancel/drain tickets.
- Events: owned producer receives explicit graceful shutdown.
- External work: ordinary completion is reaped; first stop waits without GUI
  authority; repeated stop preserves bounded data and exits.
- Artifacts: RAII cleans success; explicit private policy preserves refusal.
- Terminal: shared guard restores or intentionally leaves ownership with the
  foreground handler.
- Process exit: only after owned runtime teardown; Drop is the final idempotent
  terminal backstop.

---

## 19. 0.6A reuse

- Late work: completion after retirement cannot publish.
- Ticket: final external-text ticket remains mandatory.
- External stale write: app replacement live case refused it.
- Publication authority: unchanged session + generation + locator + scope +
  capability + authoritative readback contract.

---

## 20. 0.6B reuse

- App loss: leaves the view safely unavailable and retires old operations.
- Fresh generation: a replacement is fresh authority only after ordinary
  selection/recovery.
- Recovery: never inferred from the candidate artifact or same app name.
- Old bindings: refused.

---

## 21. 0.6C reuse

- Event producer: explicit shutdown observed.
- History: current-structure pruning regressions passed.
- Resource ownership: logical reader/child/artifact/ticket ownership settled.
- Overflow: representative bounded overflow/full-resync regression passed.
- Shutdown: application-owned producer ended before runtime destruction.

---

## 22. Genericity audit

- Toolkit: no production branch.
- App name: validation selector only, never semantic authority.
- Window title: no authority.
- PID authority: none; child PID is diagnostic observation only.
- Geometry: presentation only.
- RuntimeNodeId: snapshot identity only.
- Target matching: exact locator/current generation only.
- Shell injection: none; direct argv.
- Private APIs: none.
- Input injection: none.
- Result: **PASS**.

---

## 23. Required result summary

```text
TERMINAL_NORMAL_EXIT_RESTORED=PASS
TERMINAL_SIGINT_RESTORED=PASS
TERMINAL_SIGTERM_RESTORED=PASS
TERMINAL_CONTROLLED_PANIC_RESTORED=PASS
TERMINAL_SUSPEND_RESUME=PASS
TERMINAL_READER_SINGLE_OWNER=PASS
EXTERNAL_HANDLER_TERMINAL_REACQUIRE=PASS
REPEATED_EXTERNAL_HANDOFF_CONTINUITY=PASS
EXTERNAL_HANDLER_PROCESS_REAPED=PASS
EXTERNAL_HANDLER_SHUTDOWN_SAFE=PASS
EXTERNAL_TEXT_STALE_AUTHORITY_REFUSAL=PASS
EXTERNAL_ARTIFACT_SUCCESS_CLEANUP=PASS
EXTERNAL_ARTIFACT_STORAGE_BOUNDED=PASS
EXTERNAL_ARTIFACT_PRIVATE_PERMISSIONS=PASS
EXTERNAL_MODALITY_ORIGIN_LOSS_SAFE=PASS
EXTERNAL_MODALITY_STORAGE_BOUNDED=PASS
POST_EXTERNAL_LIFECYCLE_FRESH_OPERATION=PASS
NO_NEW_LIFECYCLE_IDENTITY=PASS
NO_EXTERNAL_TARGET_REATTACHMENT=PASS
```

---

## 24. Existing capability regression

- 0.6A: retired generation and cancelled-ticket tests plus live stale external
  result passed.
- 0.6B: ambiguous application and exact current selector choice tests passed.
- 0.6C: event-producer exit, overflow, recency, and focus-history tests passed.
- Text/Action: four verified text updates and fresh Action passed live.
- Selection: exact GTK-style parent Selection regression passed.
- External text: success, stale, conflict, shutdown, and repeated handoff passed.
- External Modality: origin-loss cancellation and bound passed.
- Password: synthetic secret absent from live tree; content and renderer
  exclusion tests passed.
- v0.4 scope/fresh scene: modal-scope and retired-transition tests passed.

---

## 25. Tests / quality

- New focused tests: one permanent Rust test for expiry-deferred active-handler
  artifacts and the global namespace cap.
- Targeted: focused artifact/ticket/modality tests, representative 0.6A/B/C,
  Action, Selection, password, modal scope, and resize regressions passed.
- Linux live: isolated Debian D-Bus, AT-SPI, Xvfb, GTK3, and PTY campaign passed.
- PTY: direct termios checks covered normal exit, SIGINT, SIGTERM, panic,
  actual stopped state, resume, handler transfer, and shutdown.
- macOS: supported source quality passed; AT-SPI live qualification is Linux.
- fmt: **PASS**.
- check: **PASS**.
- test: **PASS**.
- clippy: **PASS**.
- docs: **PASS**.
- diff: **PASS**.

The full source matrix ran once at final phase close after production and
documentation were complete.

---

## 26. Remaining issues

- P0: none.
- P1: none.
- P2: none specific to 0.6D.

0.6E integrated qualification, v0.7 environment work, and release work are not
0.6D defects.

---

## 27. Architecture conclusion

- Did existing authority model remain sufficient? **YES**.
- Was new lifecycle identity required? **NO**.
- Was ProcessSupervisor required? **NO**.
- Was TerminalManager rewrite required? **NO**.

Stable rules: external work never extends semantic target authority; privately
recoverable candidates never reattach authority. Owned artifacts are private,
bounded, leased, and conservatively recovered. The terminal has one current
owner. Supported handoff, exit, ordinary unwind panic, and Linux suspension
restore or transfer all product-controlled state coherently. A configured
handler runs as one directly owned child, completed children are awaited and
reaped, and shutdown retires authority before waiting for user completion.

---

## 28. v0.6 roadmap

- Discovery: **COMPLETE**.
- 0.6A: **COMPLETE / VALIDATED**.
- 0.6B: **COMPLETE / VALIDATED**.
- 0.6C: **COMPLETE / VALIDATED**.
- 0.6D: **COMPLETE / VALIDATED**.
- 0.6E: **PLANNED / NOT AUTHORIZED**.

Only 0.6D newly became complete.

---

## 29. Recommended next direction

**A. Recommend 0.6E — Integrated Runtime Continuity Qualification.** Do not
start it without explicit user authorization.

---

## 30. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`.
- HEAD: evidence/docs commit containing this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Public tags: unchanged; `v0.3.0` remains the latest planned public pre-1.0
  release.
- v0.6.0 tag/release: none.

---

## 31. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4/v0.5 milestone HANDOFF、
`docs/planning/v0.6-runtime-continuity.md`、`docs/planning/v0.6-roadmap.md`、
0.6A/0.6B/0.6C HANDOFF，以及本 0.6D HANDOFF。分支是
`v0.6/runtime-continuity-multi-surface`；HEAD 是包含本 HANDOFF 的
`docs: validate v0.6 external terminal lifecycle` 提交，其父提交精确为
`3d1121a27957c7e3b02465a14f9c06aa3665ad94`。v0.4/v0.5 均为
COMPLETE / MILESTONE QUALIFIED；v0.6 Discovery 的结论 A 仍有效；0.6A、
0.6B、0.6C、0.6D 均 COMPLETE / VALIDATED。权限模型仍是
`RuntimeSessionId + ApplicationGenerationId + BackendLocator`。外部工作绝不
延长语义目标权限；候选数据可以私密保留，但绝不重新附着目标权限。终端
只有一个当前所有者；正常退出、SIGINT、SIGTERM、普通 unwind panic、外部
交接及 Linux SIGTSTP/SIGCONT 均已验证，SIGKILL/断电不作恢复承诺。终端
reader 保持单所有者。外部制品目录 0700、文件 0600，最多 256 个命名空间、
每空间 256 文件、TTL 最长 1800 秒；当前模态制品最多 8 个、TTL 300 秒。
处理器是直接 argv 子进程，正常完成被 wait/reap；首次关机信号先撤销权限
并等待用户完成，重复信号是显式强制退出。下一步只建议 0.6E Integrated
Runtime Continuity Qualification；0.6E 未自动授权。v0.6 仍是内部里程碑，
不得创建 RC/tag/release；v0.7 仍在以后；v1.0.0 仍是下一计划公开版本。
不要自行扩展范围。
