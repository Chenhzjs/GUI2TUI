# GUI2TUI v0.6 Phase 0.6E Integrated Runtime Continuity Handoff

## 1. Status

- Starting HEAD: `78fb9899a6071c65940002fed1f13fe818c4a867`.
- Final HEAD: the evidence/docs commit containing this handoff; its production
  parent is `406b629` (`fix: converge integrated runtime continuity`).
- Branch: `v0.6/runtime-continuity-multi-surface`.
- Worktree: clean at final handoff.
- Production changes: full semantic refreshes and fresh selected generations
  now use a public Accessible tree walk; overflow events received during the
  first resync cause one bounded second walk instead of incremental replay.
- Tests added: one Linux/Xvfb/AT-SPI/PTY integrated live harness and bounded
  extensions to existing controlled fixtures; no new unit-test framework.
- P0: none.
- P1: none.
- Overall: **PHASE 0.6E INTEGRATED RUNTIME CONTINUITY VALIDATED**.

## 2. Integrated runtime model

0.6A operation tickets prevent retired work from publishing. 0.6B retires the
old application generation and requires an exact current selection before
opening another. 0.6C owns one bounded event producer, queue, semantic cache,
scope set, and UI history per application view. 0.6D owns one terminal reader,
direct child, and private artifact lease across external handoff and supported
terminal transitions.

The integrated path retained one `RuntimeSessionId` while generations G1
through G7 replaced every application-owned cache, event subscription, scene,
scope, focus history, commands, and tickets. Semantic authority remained:

```text
RuntimeSessionId
+ ApplicationGenerationId
+ exact BackendLocator
+ current InteractionScope
+ current capability and fresh public semantics
```

The integrated campaign exposed a concrete cache convergence defect: after an
overflow and same-looking window replacement, a missed removal could remain in
the AT-SPI Cache and later re-enter a full scene. A current public Accessible
walk now establishes full-refresh, overflow, resume, and fresh-generation
truth. Events remain bounded invalidation/wakeup evidence.

## 3. Continuous mixed lifecycle

One GUI2TUI process, PID 375 in the final isolated run, performed this sequence:

1. G1 selected the GTK4 fixture and completed `Activate safely` with current
   readback.
2. G1 opened a modal, refused a frozen background command, executed the modal
   close in current scope, then retired when the fixture was deliberately
   restarted.
3. Explicit selection opened GTK4 G2. Two windows were present while 2,000
   accessibility changes overflowed the 2,048-slot bounded queue. The old
   secondary window was destroyed and a same-looking replacement created. A
   frozen old command was refused, the fresh replacement command worked, the
   other window remained usable, and the final cache had 49 nodes/locators and
   two current scopes.
4. Explicit selection opened the independent Qt fixture as G3. G2's producer
   retired, histories reset, `Fresh operation` succeeded, and a later GTK event
   did not enter G3's event queue.
5. Explicit selection opened the GTK3 lifecycle fixture as G4. One external
   edit succeeded and cleaned its candidate; one concurrent GUI edit caused a
   conflict and preserved exactly one private candidate. Each handler return
   reacquired the terminal and one reader.
6. A third handler remained open while its G4 application disappeared and a
   same-looking instance appeared. The child completed and was reaped, its
   candidate was preserved, no replacement target changed, and explicit
   selection opened usable G5.
7. G5 opened the existing External Modality reference view for `Lifecycle
   diagram`; switching away retired the origin. The mixed run did not repeat
   0.6D's static-snapshot materialization campaign.
8. G6 started a delayed Toggle, then the application was replaced with an
   externally identical instance. The old result could not publish; explicit
   selection opened G7 and a fresh Toggle succeeded.
9. G7 received actual SIGTSTP. The PTY was canonical with echo while stopped.
   The fixture independently changed its Toggle while GUI2TUI was stopped.
   SIGCONT reacquired raw/alternate-screen ownership, installed one reader,
   increased the fresh snapshot count, reflected current GUI state, and a new
   Toggle succeeded.
10. Normal `q` retired G7's producer and restored canonical input, echo,
    alternate screen, and cursor state.

The external-handler × shutdown cross-scenario used a second TUI process
because shutdown necessarily ends the primary process. No other stage boundary
was presented as part of the continuous PID 375 sequence.

## 4. Late-work and generation isolation

- G6's delayed operation was owned by its ticket and exact locator.
- Destruction retired G6 before the same-looking replacement could receive a
  result; `rejected_late_results` reached 2 in the primary run.
- G7 began only after fresh public enumeration and explicit user selection.
- The old secondary-window command could not bind to the replacement locator.
- The G4 external candidate remained data only. It neither found nor wrote the
  replacement GTK3 text target.
- Commands and focus history reset to zero at application selection; old event
  producers stopped before the fresh view opened.

Result: no old task, event, command, candidate, name, PID, geometry, historical
focus, or `RuntimeNodeId` migrated authority into a new generation.

## 5. Dynamic surface and event convergence

The GTK4 path combined modal scope, simultaneous windows, same-looking window
replacement, and a real bounded event overflow. The initial implementation
could replay events over an incomplete cache-derived baseline and later revive
the destroyed window. The production fix makes every full application refresh
walk the current public tree and treats events buffered during overflow resync
as evidence to take one bounded second walk.

The final run observed one current main window after closing the replacement,
equal node/locator counts, queue depth zero, and a successful operation in the
unaffected window. Old scope and binding authority were refused.

## 6. External and terminal integration

- Handler: direct configured argv, one `{file}` placeholder, no shell.
- Success: child completed, was waited/reaped, authoritative readback contained
  the edit, candidate count returned to zero, and the TUI reacquired raw mode.
- Conflict: GUI state remained `lifecycle authoritative B`; one 0600 candidate
  under a 0700 namespace preserved user data without write authority.
- Target loss: terminal was detached while the child remained user-owned; app
  loss retired the ticket; completion was reaped and preserved privately.
- Suspend/resume: actual SIGTSTP/SIGCONT restored and reacquired PTY state,
  advanced `full_snapshots`, kept generation G7, and accepted a fresh operation.
- Shutdown cross: first SIGTERM retired semantic authority and restored the
  terminal while leaving the active user handler intact. After the handler
  finished, GUI2TUI reaped it and exited 0. The same candidate remained private,
  and a replacement GUI target was unchanged.
- Reader: the one process reported terminal detach/resume counts `4/4` after
  three handler handoffs plus suspension; FD count stayed 19 and no child or
  zombie remained at any sample.

## 7. Recovery and continued usability

Application disappearance invalidated the current generation and event
producer. Recovery never matched a remembered name into authority. The user
opened the selector and chose an item from the current enumeration, which
created a fresh cache, scope set, scene, bindings, commands, event queue, and
generation. Fresh Action/Toggle operations succeeded after GTK replacement,
Qt selection, external-handler staleness, late-operation replacement, overflow,
and terminal resume.

## 8. Resource continuity

The primary run sampled logical resources at six points:

| Point | Gen | Nodes/locators | Scopes | Bindings | Queue | Tickets | FDs | Threads | Children/zombies | Text artifacts | RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| initial GTK4 | 1 | 49/49 | 2 | 17 | 0/2048 | 0 | 19 | 14 | 0/0 | 0 | 20,988 |
| post-overflow GTK4 | 2 | 49/49 | 2 | 17 | 0/2048 | 0 | 19 | 14 | 0/0 | 0 | 23,696 |
| Qt | 3 | 8/8 | 2 | 3 | 0/2048 | 0 | 19 | 14 | 0/0 | 0 | 23,696 |
| post-external GTK3 | 4 | 9/9 | 2 | 4 | 0/2048 | 0 | 19 | 14 | 0/0 | 1 | 23,712 |
| recovered GTK3 | 5 | 9/9 | 2 | 4 | 0/2048 | 0 | 19 | 14 | 0/0 | 2 | 23,720 |
| post-resume Qt | 7 | 7/7 | 2 | 3 | 0/2048 | 0 | 19 | 14 | 0/0 | 2 | 23,736 |

The two candidates are the intentionally preserved conflict and stale-target
data. Existing per-namespace count and TTL bounds apply. Cache size followed
the current application rather than accumulating prior generations. FDs and
threads were flat after startup; RSS was observed only as supporting data.

## 9. Cross-scenario evidence

### Late operation × application replacement

G6's delayed Toggle was pending when its process exited. A same-looking Qt
instance appeared, but no old status or Toggle state reached it. G7 required
fresh selection and accepted a new exact operation. **PASS**.

### External handler × shutdown

The separate shutdown process retired authority on SIGTERM, restored the PTY,
left the handler and candidate available to the user, rejected replacement
writeback, reaped the completed child, and exited without a zombie. **PASS**.

### Event overflow × multi-window replacement

GTK4 held two windows while the event storm overflowed. One window was closed
and replaced by an identical surface. The old frozen command was refused, the
fresh locator worked, the unaffected window remained usable, and a current
walk converged to one remaining window. **PASS**.

## 10. Existing capability regression

- v0.4/v0.5 representative behavior: Action and Toggle, modal scope, frozen
  command refusal, dynamic surface replacement, and fresh-scene continuation
  ran live. Selection and PageTab/Expand were not rerun in 0.6E.
- Complex text: success, conflict, target loss, fresh recovery, and shutdown
  were integrated live.
- Password: the GTK3 secret was absent from public inspection and modality
  discovery; the existing password content exclusion test also passed.
- External Modality: current reference discovery/view and origin retirement ran
  in the primary process. Static snapshot materialization was not rerun; 0.6D
  remains the qualification source for capture/storage lifecycle.

## 11. Required result summary

```text
INTEGRATED_CONTINUOUS_RUNTIME=PASS
MIXED_LIFECYCLE_AUTHORITY_ISOLATION=PASS
MIXED_SURFACE_EVENT_CONVERGENCE=PASS
MIXED_EXTERNAL_TERMINAL_CONTINUITY=PASS
APP_RECOVERY_FRESH_AUTHORITY=PASS
LATE_OPERATION_REPLACEMENT_REFUSAL=PASS
EXTERNAL_HANDLER_SHUTDOWN_INTEGRATION=PASS
OVERFLOW_MULTI_WINDOW_CONVERGENCE=PASS
POST_RECOVERY_SEMANTIC_USABILITY=PASS
TERMINAL_RESTORATION_AND_SINGLE_READER=PASS
LOGICAL_RESOURCE_GROWTH=BOUNDED
NO_STALE_AUTHORITY_MIGRATION=PASS
NO_NEW_RUNTIME_FRAMEWORK=PASS
```

## 12. Tests / quality

- Targeted: late-generation refusal, event-producer shutdown, 10,000-event
  queue bound, continuing-handler artifact bound, and password content
  exclusion passed.
- Linux live: final `tests/live/v06e_run_linux.sh 143` passed in Ubuntu
  24.04 arm64-compatible container tooling with isolated Xvfb, session D-Bus,
  AT-SPI, GTK3, GTK4, Qt6, and a PTY.
- macOS: source quality matrix passed on the arm64 host; Linux AT-SPI live work
  is platform-specific.
- fmt: **PASS**.
- check: **PASS**.
- test: **PASS**.
- clippy: **PASS**.
- docs: **PASS**.
- diff: **PASS**.

The full source-quality matrix ran once after final production and documentation
changes.

## 13. Remaining issues

- P0: none.
- P1: none.
- P2: none specific to 0.6E.

Validation limits: Selection, PageTab/Expand, and External Modality static
materialization were not rerun. Their prior milestone evidence remains valid;
the integrated path exercised representative Action/Toggle, modal, dynamic,
external text, event, recovery, and terminal behavior. Milestone Qualification
/ Close is separate work, not a 0.6E defect.

## 14. Architecture conclusion

`RuntimeSessionId + ApplicationGenerationId + BackendLocator`, combined with
current scope, capability, and public state, remains sufficient. 0.6A through
0.6D safely compose after the bounded current-tree convergence fix. v0.6 does
not require a new identity, workflow/task engine, event model, process
supervisor, or terminal manager rewrite.

Stable integrated rule: a full semantic refresh is a correctness boundary. It
uses the current public Accessible tree; cache residency and queued events may
wake convergence but cannot preserve or reintroduce a destroyed surface.

## 15. v0.6 roadmap

- Discovery: **COMPLETE**.
- 0.6A: **COMPLETE / VALIDATED**.
- 0.6B: **COMPLETE / VALIDATED**.
- 0.6C: **COMPLETE / VALIDATED**.
- 0.6D: **COMPLETE / VALIDATED**.
- 0.6E: **COMPLETE / VALIDATED**.
- Functional development: **COMPLETE**.
- Milestone qualification: **PLANNED / NOT AUTHORIZED**.

0.6E completion does not perform or authorize v0.6 Milestone Qualification /
Close.

## 16. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`.
- HEAD: evidence/docs commit containing this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Public tags: unchanged; `v0.3.0` remains the latest planned public pre-1.0
  release.
- v0.6.0 tag/release: none.

## 17. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4/v0.5 milestone HANDOFF、
`docs/planning/v0.6-runtime-continuity.md`、`docs/planning/v0.6-roadmap.md`、
0.6A/0.6B/0.6C/0.6D HANDOFF，以及本 0.6E HANDOFF。分支是
`v0.6/runtime-continuity-multi-surface`；HEAD 是包含本 HANDOFF 的
`docs: validate v0.6 integrated runtime continuity` 提交，其父提交为
`406b629`。v0.4、v0.5 均 COMPLETE / MILESTONE QUALIFIED；v0.6 Discovery
结论 A 仍成立；0.6A/B/C/D/E 均 COMPLETE / VALIDATED，functional
development COMPLETE。权限模型仍为 `RuntimeSessionId +
ApplicationGenerationId + BackendLocator`，并要求当前 scope、capability 与
fresh public semantics。事件只唤醒/标记失效；full refresh、overflow
convergence、resume 与 fresh selection 以当前 public Accessible walk 建立
事实。资源随当前 generation/surface 有界，旧 producer、ticket、command、
binding、candidate 均不能迁移权限。外部候选可私密有界恢复，但绝不自动
附着新目标；直接 argv 子进程正常 wait/reap，shutdown 先撤销权限再按既有
defer-once 契约保护用户数据。终端只有一个当前 owner/reader；正常退出、
外部交接和 Linux SIGTSTP/SIGCONT 已集成验证。无 P0/P1；0.6E 未重跑
Selection、PageTab/Expand 和 External Modality 静态 materialization，这些由
既有里程碑证据覆盖。下一步只能建议用户另行授权 v0.6 Milestone
Qualification / Close；不得自动开始。v0.7 未授权，v0.6 仍为内部里程碑，
不得 RC/tag/release；v1.0.0 仍是下一次计划公开发布。不要自行扩展任务。
