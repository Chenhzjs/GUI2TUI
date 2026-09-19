# GUI2TUI v0.6 Runtime Continuity & Multi-Surface Robustness Milestone Handoff

## 1. Status

- Starting HEAD: `5f452c495d702a0382f36d6770dcefef7ed21e87`.
- Final HEAD: the documentation/status commit containing this handoff.
- Branch: `v0.6/runtime-continuity-multi-surface`.
- Worktree: clean at final handoff.
- Production changes: none during Milestone Qualification / Close.
- Tests changed: none.
- Functional development: **COMPLETE**.
- Milestone qualification: **QUALIFIED**.
- P0: none.
- P1: none.
- Overall: **GUI2TUI v0.6 RUNTIME CONTINUITY & MULTI-SURFACE ROBUSTNESS
  MILESTONE QUALIFIED**.

## 2. Commits

- Production: no milestone-close production commit. The final v0.6 production
  commit is `406b629` (`fix: converge integrated runtime continuity`); earlier
  bounded production commits are indexed by their phase handoffs.
- Tests: no milestone-close test commit.
- Evidence/docs: `docs: qualify v0.6 runtime continuity milestone` (the commit
  containing this handoff).

## 3. v0.6 final architecture

0.6A made `ApplicationGenerationId` the application-authority epoch within the
existing `RuntimeSession`, with final operation tickets preventing cancelled or
retired work from publishing. 0.6B separated transport recovery from semantic
reauthorization: current enumeration and explicit application selection create
a fresh generation. 0.6C tied event producers, queues, caches, scopes, scenes,
bindings, focus history, and recent commands to the current application view
and bounded their retention. 0.6D established direct child ownership, private
bounded artifacts, and one terminal owner/reader across handler handoff,
signals, unwind panic, and Linux suspension. 0.6E proved these mechanisms
compose in a mixed continuous runtime and corrected full refresh so current
public Accessibility structure, rather than stale cache residency, establishes
the semantic baseline.

The final authority model remains `RuntimeSessionId` +
`ApplicationGenerationId` + exact `BackendLocator`, further constrained by
current `InteractionScope`, current advertised capability and public state,
the operation-specific contract, and authoritative readback. No new runtime
identity or framework was needed.

## 4. Phase qualification summary

- Discovery: **COMPLETE**; conclusion A, existing authority model sufficient
  with bounded hardening. Evidence:
  [`docs/planning/v0.6-runtime-continuity.md`](../../../planning/v0.6-runtime-continuity.md).
- 0.6A: **COMPLETE / VALIDATED**. Evidence:
  [`runtime-epoch-late-work/HANDOFF.md`](../runtime-epoch-late-work/HANDOFF.md).
- 0.6B: **COMPLETE / VALIDATED**. Evidence:
  [`transport-application-recovery/HANDOFF.md`](../transport-application-recovery/HANDOFF.md).
- 0.6C: **COMPLETE / VALIDATED**. Evidence:
  [`surface-event-resource-continuity/HANDOFF.md`](../surface-event-resource-continuity/HANDOFF.md).
- 0.6D: **COMPLETE / VALIDATED**. Evidence:
  [`external-terminal-lifecycle/HANDOFF.md`](../external-terminal-lifecycle/HANDOFF.md).
- 0.6E: **COMPLETE / VALIDATED**. Evidence:
  [`integrated-runtime-continuity/HANDOFF.md`](../integrated-runtime-continuity/HANDOFF.md).

## 5. Stable authority rules

- RuntimeSessionId: identifies one runtime session; recovery does not replace it
  merely because transport or terminal ownership changes.
- ApplicationGenerationId: the application-authority epoch. Application loss,
  switch, or reauthorization retires the old generation and its publication
  authority.
- BackendLocator: exact AT-SPI object authority within the current generation;
  authority for L1 never transfers to similar replacement L2.
- RuntimeNodeId: snapshot/presentation continuity only; never operation
  authority.
- InteractionScope: only current visible and invokable scope may resolve an
  operation; modal background bindings are refused.
- Operation ticket: cancellation or generation retirement revokes final
  publication and success authority even if backend work later returns.
- Current capability: an operation requires compatible capability/action and
  fresh public semantics; visibility, focus, names, indices, and geometry do
  not manufacture it.
- Authoritative readback: GUI/backend acceptance and operation-specific fresh
  readback establish success. Optimistic local state does not.

## 6. Stable recovery rules

- Transport: reconnect restores communication only.
- Application selection: a current enumeration plus explicit selection binds
  an exact current application.
- Application restart: creates a fresh `ApplicationGenerationId`; a remembered
  name, PID, title, or similar tree cannot reauthorize it.
- Window recreation: may remain inside the current application generation, but
  the destroyed locator and binding retire and the replacement receives only
  fresh exact authority.
- Old bindings: never reactivate after application, surface, scope, or locator
  retirement.
- Non-idempotent replay: never automatic; the user chooses the next operation
  from the current fresh scene.
- Fresh generation: owns a fresh cache, event subscription, scope, focus,
  scene, bindings, and commands.

## 7. Event, cache and refresh correctness

- Event ownership: each current application view owns its event producer; the
  producer exits with the view and old-generation events cannot enter the new
  view.
- Event overflow: the bounded 2,048-event queue discards incomplete history and
  requests full semantic convergence.
- Current public tree: current public Accessibility containment and relations
  establish which semantic objects exist.
- Full refresh: **a full semantic refresh is a correctness boundary. Current
  public Accessibility structure, not cache residency or an incomplete event
  history, establishes the current semantic baseline.**
- Cache residency: cached presence cannot preserve or restore a destroyed
  surface. Cache identity remains useful only after reconciliation with the
  current public walk.
- Surface disappearance: removes exact scope, focus, command, scene, and
  binding authority; a same-looking replacement is fresh.
- Scene reconstruction: overflow resynchronization walks the current tree,
  allows events received during resync to request one bounded second walk, and
  then rebuilds the current cache, scope, scene, and bindings.

Normal runtime updates remain incremental. The correctness-boundary rule does
not require a full walk after every event, and presentation-only spatial
geometry remains separate from semantic existence and authority.

## 8. External and terminal lifecycle

- Handler: optional user-configured direct argv with exactly one `{file}`;
  there is no shell interpretation.
- Child process: locally owned, awaited, and reaped. Target retirement revokes
  authority without blindly killing an editor that may contain unsaved work.
- Semantic authority: handler completion or candidate possession never extends
  the original generation, ticket, locator, scope, capability, or readback
  authority.
- Candidate artifact: modified stale/conflict data may remain recoverable as
  private data but can never locate or attach to a replacement GUI target.
- Artifact bounds: owned directories are 0700, candidates 0600; recovery is
  limited to 256 namespaces and 256 files per namespace with TTL capped at
  1,800 seconds.
- External Modality: acquisition/presentation identity remains separate from
  mutation authority; current session storage is limited to 8 artifacts with a
  300-second TTL.
- Terminal owner: exactly one current owner; TUI modes are restored before an
  external handler or suspension receives the terminal.
- Terminal reader: exactly one while attached, none consuming handler input,
  and one freshly created after reacquisition.
- Supported exit paths: normal exit, SIGINT, SIGTERM, and ordinary Rust unwind
  restore raw mode, alternate screen, cursor, keyboard/mouse state, canonical
  input, and echo as applicable.
- Suspend/resume: Linux SIGTSTP restores and actually stops; SIGCONT reacquires,
  redraws, installs one reader, and performs fresh semantic convergence without
  inventing a generation change.

SIGKILL, power loss, `panic=abort`, cleanup double panic, memory corruption,
and other destruction without a userspace cleanup opportunity are outside the
restoration guarantee.

## 9. Integrated runtime evidence

- Continuous GUI2TUI process: PID 375 in the final 0.6E isolated run.
- Application generations: G1 through G7 were observed in that same process.
- Mixed lifecycle sequence: initial Action; modal enter/exit and stale command
  refusal; two-window event overflow and same-looking replacement; GTK-to-Qt
  application switch; external edit success and conflict; handler-active target
  loss; External Modality origin retirement; delayed operation during
  application replacement; SIGTSTP/SIGCONT while GUI state changed; fresh
  Toggle; normal exit and terminal restoration.
- Cross-scenario evidence: late operation × application replacement and event
  overflow × multi-window replacement ran inside PID 375. External handler ×
  shutdown ran in a separate supplemental TUI process because shutdown ends the
  primary process; it is not represented as part of PID 375.
- Late operation: G6 result was rejected after replacement; G7 required fresh
  selection and accepted a new exact operation.
- Surface/event convergence: real overflow discarded incomplete history,
  current-tree resync removed the destroyed surface, the frozen old command was
  refused, and the unaffected window remained usable.
- External/terminal integration: successful handlers were reaped, conflict and
  stale candidates remained private, terminal detach/reacquire kept one reader,
  and the separate first-SIGTERM path retired authority while preserving the
  active editor until completion.
- Recovery usability: fresh Action/Toggle operations succeeded after switches,
  replacements, overflow, external staleness, and terminal resume.
- Resource continuity: six samples kept FDs at 19, threads at 14, children and
  zombies at zero outside an active handler, queue/tickets at zero when settled,
  and caches sized to the current generation. Candidate count rose from 0 to 2
  only for intentionally retained conflict/stale recovery data under existing
  bounds.

## 10. v0.6 exit gates

1. **PASS — Discovery complete:** the Discovery documented live evidence,
   failure analysis, conclusion A, and five bounded phases.
2. **PASS — 0.6A:** live and regression evidence rejected late results,
   cancelled publication, stale locators, and external final-ticket writeback.
3. **PASS — 0.6B:** transport loss/recovery, duplicate-name selection, A → B →
   A, unavailable state, and bounded retry all required fresh authority.
4. **PASS — 0.6C:** per-view producer retirement, bounded queue overflow,
   surface churn, current-history pruning, and logical resource bounds passed.
5. **PASS — 0.6D:** direct child wait/reap, artifact privacy/bounds, one reader,
   normal/signal/panic restoration, and Linux suspend/resume passed.
6. **PASS — 0.6E:** PID 375 supplied continuous G1–G7 mixed evidence; all three
   required cross-scenarios had actual evidence, with shutdown explicitly
   separated as supplemental.
7. **PASS — no authority migration:** retired generation, locator, ticket,
   command, and binding results did not affect replacements.
8. **PASS — fresh reauthorization:** transport and application recovery used
   current enumeration and explicit selection to create a fresh generation.
9. **PASS — refresh convergence:** current public tree walks removed destroyed
   surfaces after lost removal events and real overflow.
10. **PASS — continued interaction:** current Action/Toggle, modal, complex
    text, external modality reference, and fresh post-recovery operations
    remained usable.
11. **PASS — external/terminal ownership:** completed children were reaped,
    candidates followed safe policy, and supported terminal paths restored or
    transferred ownership coherently.
12. **PASS — bounded resources:** producers, queues, cache/view state, history,
    tickets, readers, children, candidates, and modality artifacts followed
    current ownership or explicit caps.
13. **PASS — genericity/security/privacy:** no app/toolkit production branch,
    private API, injection, fuzzy authority, password export, or filesystem
    semantic bypass was introduced.
14. **PASS — final-source qualification:** 0.6E ran Linux live integration and
    the complete source-quality matrix on the final production source.
15. **PASS — severity gate:** P0 = 0 and P1 = 0.
16. **PASS — healthy baseline:** final source and evidence support using this
    internal milestone as the starting point for a separately authorized v0.7
    Discovery.

## 11. Existing capability regression

- v0.4 continuation: modal scope, frozen-command refusal, dynamic replacement,
  and fresh-scene continuation ran in 0.6E. The broader v0.4 campaign was not
  repeated; its milestone handoff remains the qualification source.
- v0.5 task baseline: representative Action/Toggle ran in 0.6E. Selection and
  PageTab/Expand were not rerun; the v0.5 milestone evidence remains valid.
- v0.3 verified mutation: complex text success, conflict, stale-target refusal,
  recovery, and handler shutdown ran in the integrated evidence. The full v0.3
  release campaign was not repeated.
- Password: synthetic secret content was absent from inspection and modality
  discovery, and the existing exclusion regression passed in 0.6E.
- External Modality: reference discovery/view and origin retirement ran in PID
  375; static snapshot materialization was not rerun and remains qualified by
  0.6D evidence.
- Spatial presentation: current scene reconstruction and redraw were exercised;
  the historical v0.2 compatibility/layout matrix was not repeated.

## 12. Genericity and forbidden audit

- Application/toolkit branches: none added to production behavior.
- Private APIs: none.
- Input injection: none; no XTest, uinput, keyboard, mouse, or coordinate
  fallback.
- Coordinate authority: none; geometry remains presentation evidence.
- Name/index identity: neither names, PIDs, titles, historical indices, nor
  `RuntimeNodeId` grant authority.
- Filesystem semantic bypass: none; artifacts carry data, never GUI authority.
- Fuzzy target matching: none; operations require exact current locators.
- New runtime frameworks: none; no runtime epoch identity, workflow engine,
  process supervisor, window manager, or terminal manager rewrite.
- Result: **PASS**.

## 13. Quality basis

0.6E completed these checks once after the final production and evidence
changes:

- Final-source Linux qualification: **PASS** — Ubuntu 24.04 arm64-compatible
  isolated Xvfb/session-D-Bus/AT-SPI run with GTK3, GTK4, Qt6, and PTY.
- macOS source quality: **PASS** on the arm64 host; Linux AT-SPI live validation
  remains platform-specific.
- fmt: **PASS** — `cargo fmt --all -- --check`.
- check: **PASS** — `cargo check --all-targets`.
- test: **PASS** — 298 library, 2 inspect CLI, and 4 user CLI tests.
- clippy: **PASS** — `cargo clippy --all-targets -- -D warnings`.
- docs: **PASS**.
- diff: **PASS**.

Milestone Close changed documentation only. It did not repeat Rust or live
qualification. It ran `python3 scripts/check-docs.py` and `git diff --check`
against this close, and both passed.

## 14. Remaining issues and explicit boundaries

- P0: none.
- P1: none.
- P2: none specific to the v0.6 milestone.

Selection, PageTab/Expand, External Modality static materialization, and the
full historical compatibility matrices were not rerun in 0.6E or Milestone
Close; their phase/milestone evidence remains the stated basis. Unsupported
terminal destruction paths remain excluded as described above. Wayland, SSH,
headless/deployment qualification, installers, environment diagnosis, remote
companion work, and the final 1.0 integration campaign belong to later
separately authorized work and are not v0.6 defects.

## 15. Milestone conclusion

- Is v0.6 Runtime Continuity & Multi-Surface Robustness milestone qualified?
  **YES**.
- Did v0.6 require a new runtime identity or framework? **NO**.
- Is the current healthy source suitable as the baseline for v0.7 Discovery?
  **YES**, when that Discovery is explicitly authorized.

## 16. Roadmap to 1.0

- v0.4: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.5: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.6: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.7: **PLANNED / NOT AUTHORIZED**.
- 1.0: **PLANNED / NOT AUTHORIZED**; v1.0.0 remains the next planned public
  release.

## 17. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`.
- HEAD: the `docs: qualify v0.6 runtime continuity milestone` commit containing
  this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Existing public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, and `v0.3.0`, unchanged.
- v0.6.0 tag: absent.
- v0.6.0 release: absent.

## 18. Next recommended direction

Recommend **v0.7 — Deployment & Environment Completeness Discovery**.

Authorization status: **NOT YET AUTHORIZED**. This task did not start it.

## 19. Next Codex context

新会话先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md` 和本 v0.6 milestone HANDOFF；需要追溯证据时
再读 v0.6 Discovery、roadmap 与 0.6A–0.6E HANDOFF。当前分支是
`v0.6/runtime-continuity-multi-surface`，HEAD 是包含本 HANDOFF 的
`docs: qualify v0.6 runtime continuity milestone` 提交，其父提交为
`5f452c495d702a0382f36d6770dcefef7ed21e87`。v0.4、v0.5、v0.6 均为
COMPLETE / MILESTONE QUALIFIED / INTERNAL；0.6A–0.6E 均已验证，P0/P1 为
0。权限模型仍是 `RuntimeSessionId + ApplicationGenerationId + exact
BackendLocator`，并要求当前 scope、capability、public semantics、ticket 与
权威 readback；`RuntimeNodeId` 仅服务展示连续性。transport recovery 只恢复
通信，semantic reauthorization 必须依据当前枚举和明确选择建立 fresh
generation。full refresh 是正确性边界：当前 public Accessible tree，而非
cache residency 或不完整事件历史，建立当前语义基线；正常事件处理仍为增量，
geometry 仍只用于展示。event producer、队列、cache、focus/command history、
ticket 与外部资源均随当前所有权或明确容量保持有界。外部工作不延长语义权限；
私有 candidate 可以在期限内恢复用户数据，但绝不能重新附着 GUI target；直接
argv 子进程由本地 wait/reap。终端只有一个当前 owner/reader，已验证正常退出、
SIGINT、SIGTERM、普通 unwind panic、外部交接及 Linux SIGTSTP/SIGCONT；
SIGKILL、断电和 `panic=abort` 不在清理保证内。0.6E 在同一 PID 375 中验证
G1–G7 混合路径，external handler × shutdown 则是明确分开的补充进程；最终
源码通过 Linux live、macOS source quality 与完整 Rust 质量矩阵，本次关闭只
复查文档与 Git。实际限制包括 0.6E 未重跑 Selection、PageTab/Expand、
External Modality 静态 materialization 和完整历史兼容矩阵，沿用既有证据。
下一步只能建议 v0.7 Deployment & Environment Completeness Discovery，尚未授权；
不得自行开始。v0.6 仍是内部里程碑，不创建 v0.6.0 RC/tag/release；v1.0.0
仍是下一计划公开版本。禁止自行扩展任务。
