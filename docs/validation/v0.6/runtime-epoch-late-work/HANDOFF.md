# GUI2TUI v0.6 Phase 0.6A — Runtime Epoch and Late-Work Isolation

## 1. Status

- Phase: 0.6A — Runtime Epoch and Late-Work Isolation
- Starting HEAD: `6c8f7b084b4b6b99c72f5e1b6a94122a392d348a`
- Final HEAD: the evidence/docs commit containing this handoff
- Branch: `v0.6/runtime-continuity-multi-surface`
- Worktree: clean at handoff
- Production changes: bounded final-ticket publication gates, exact
  application-owner liveness, and operation-observer timeout/isolation
  hardening
- New identity layers: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.6A RUNTIME EPOCH AND LATE-WORK ISOLATION VALIDATED**

v0.6 remains internal. No package version, RC, tag, release artifact, public
release, reconnect-policy redesign, or later v0.6 phase was started.

## 2. Commits

- Production/tests: `e4eaad1` (`fix: isolate late operation work by
  application generation`)
- Evidence/docs: `docs: validate v0.6 late-work isolation` (the commit
  containing this handoff)

## 3. Existing authority model

- `RuntimeSessionId`: isolates operation tickets from every other runtime
  session.
- `ApplicationGenerationId`: remains the application-authority epoch inside
  one runtime session.
- `BackendLocator`: remains the exact AT-SPI bus/object authority for the
  selected application and operation target inside that generation.
- `RuntimeNodeId`: remains snapshot/presentation identity only and is never
  used to relocate or authorize a replacement target.

The Discovery conclusion remains **SUFFICIENT WITH SMALL HARDENING**.

- New `RuntimeEpochId`: **NO**.
- New `SurfaceGenerationId`: **NO**.

## 4. Generation-owned work contract

Generation-owned work includes semantic mutation calls, transition and
selection observation, external-text handler completion/writeback, and their
operation-associated status/result/refresh publication. It captures the
runtime session, application generation, operation ticket, exact application
and target locators, semantic intent/capability, and scope required by the
operation.

Application invalidation, runtime shutdown, or explicit ticket cancellation
removes publication authority. Exact target disappearance independently makes
the captured target authority stale. Retired work may finish internally,
release resources, and preserve an already-produced private recovery artifact.
It may not:

- **mutate:** resolve or issue a new semantic mutation against G2 or a
  replacement target;
- **confirm:** turn backend acceptance or later matching GUI state into a
  `Confirmed` result for the retired operation;
- **publish:** replace current status/result or attribute current truth to G1;
- **overwrite:** write external candidate content into a replacement GUI
  object.

## 5. Operation ticket lifecycle

- Create: after current operation authority is captured and validated, before
  the associated asynchronous backend/observer work starts.
- Register: `RuntimeSession::begin` stores the ticket, kind, and cancellation
  token under a monotonic operation ID.
- Active bound: unchanged at 32.
- Cancel: cancelling the token makes `complete` reject the ticket and removes
  its active registry entry when completion is attempted.
- Generation invalidation: `invalidate_application` drains every active
  operation, cancels every token, and clears application/generation authority.
- Completion: `RuntimeSession::complete` is the single final publication gate
  for operation-associated results.
- Removal: accepted completion removes the ticket; cancelled completion,
  invalidation, timeout completion, and shutdown do not retain an indefinitely
  active ticket.
- Late completion: wrong session/generation is `StaleIdentity`; missing,
  already-completed, or cancelled authority is `Cancelled`; neither may
  publish.
- Shutdown: `RuntimeSession::shutdown` enters stopping, invalidates the
  application and drains/cancels active tickets, then enters stopped.

The deterministic cancellation test also proves that capacity is available
again after a cancelled ticket's rejected completion.

## 6. Final completion gate

- Session: the ticket session must equal the current `RuntimeSessionId`.
- Generation: the ticket generation must equal the current
  `ApplicationGenerationId`.
- Cancellation: the registered operation must still exist and its token must
  not be cancelled.
- Operation-specific readback: exact target, capability, scope, and required
  fresh semantic postcondition remain independently mandatory.
- Publication: only after both semantic verification and successful final
  `RuntimeSession::complete` may the result/status/reload be published.

A successful backend invocation is never substituted for this gate. Selection,
transition-action, and external-text paths now discard late failure and success
returns before status or refresh publication.

## 7. Application-exit late operation

- Fixture: controlled PyQt6 `gui2tui-v06a-fixture` under an isolated Linux
  D-Bus/Xvfb session.
- Operation: exact `Toggle` on `Delayed toggle`; the public invocation returns
  while desired-state confirmation remains pending in the bounded observer.
- G1: exact first application owner and generation.
- Exit: A1 is terminated after the deterministic fixture marker and observer
  quiescence marker.
- Late backend result: invocation acceptance cannot confirm; the still-active
  observer detects exact owner loss before any broad refresh.
- Ticket: application invalidation drains it; observed active count is zero.
- Confirmation: none after invalidation.
- Mutation: the newly started application remains at its initial state.
- Current runtime: safely unavailable until existing explicit F5 recovery
  opens a fresh generation.
- Result: **PASS**.

## 8. Application-restart late operation

- A1: PyQt6 runtime fixture with the stable app name and window title.
- G1: captured from A1's exact unique D-Bus owner.
- Target: exact `Delayed toggle` locator in A1.
- Operation: delayed Toggle plus bounded transition observation.
- Restart: A1 is terminated and the same fixture is started again before the
  old operation can publish.
- A2: same application name, title, labels, and semantic roles as A1.
- G2: established only through the existing explicit F5 recovery; A2's exact
  application locator is verified different from A1's.
- Late G1 result: discarded after generation invalidation/final ticket refusal.
- A2 mutation: none; its fresh checkbox remains unchecked.
- Current result/status: no retired G1 success is installed.
- Fresh G2 operation: exact `Fresh toggle` works and fresh GUI semantics show
  `Status: fresh=True` and checked state.
- Result: **PASS**.

## 9. Same-generation target replacement

- Old target: `Replaceable toggle` in the current PyQt6 application.
- Old locator: captured before operation invocation.
- Replacement: a new same-label/same-role checkbox replaces the old object.
- New locator: freshly observed and proven different from the old locator.
- Generation: unchanged.
- Late completion: the old exact-target observation returns `Stale`.
- Replacement mutation: none inherited from the old operation.
- Confirmation: old operation does not confirm; after an explicit fresh scene
  refresh, a new operation on the new locator confirms normally.
- Result: **PASS**.

Window or target recreation did not create a new application generation and
did not require a surface generation. Exact `BackendLocator` authority was
sufficient.

## 10. Cancellation

- Operation: deterministic `TransitionObservation` ticket.
- Cancellation: its supplied token is explicitly cancelled while registered.
- Underlying work: may return; physical backend-call cancellation is not
  required for semantic safety.
- Late result: final `complete` returns `Cancelled` and removes the entry.
- Confirmation: impossible through that retired ticket.
- Current scene: no synthetic state is installed; a fresh ticket can be
  created and completed immediately.
- Result: **PASS**.

If an already-reached GUI mutation later appears in fresh authoritative state,
the scene may truthfully show it, but the cancelled operation's historical
outcome remains cancelled/unconfirmed.

## 11. External-text final authority

- Target: exact multiline text target from the existing GTK live fixture.
- Generation: G1 plus the original exact locator/scope/value.
- Handler: configured external text handler paused by deterministic ready and
  resume markers.
- Lifecycle change: A1 exits and a same-looking A2 starts while the handler is
  open.
- Candidate: modified candidate is returned only after A2 exists.
- Final ticket: exact application-owner synchronization invalidates G1;
  `complete_external_text_ticket` rejects the retired ticket.
- Writeback: no write is attempted against A2.
- Readback: no false G1 readback or confirmation is attributed to A2.
- False success: none; the stale path leaves current-generation status
  untouched.
- Replacement: A2 retains `alpha line`; after fresh explicit recovery, a new
  external edit writes and confirms normally.
- Result: **PASS**.

The stale modified candidate is preserved by the existing recovery policy as
one private regular artifact with mode 0600. Artifact cleanup breadth remains
0.6D; artifact existence never grants semantic write authority.

## 12. Status/result publication

- Current mechanism: `TuiApplication` is the single mutable owner of current
  scene/status; operation-associated async paths return to that owner and must
  pass `RuntimeSession::complete` before publishing.
- Old-generation publication: rejected transition, selection, and external
  results return without status, result, cache-refresh, or semantic-success
  publication.
- New-generation current result: fresh G2 operations publish normally through
  fresh tickets.
- Ordering: a newer G2 user operation cannot execute concurrently while an
  older handler owns the same mutable `TuiApplication`; invalidation and fresh
  recovery occur before another user operation can publish.
- Protection: generation invalidation plus the final ticket gate prevents the
  retired return from becoming current; harmless current-truth refreshes are
  not attributed as old-operation success.
- Result: **NOT_APPLICABLE** for the requested live “R2 exists before O1
  returns” ordering, because that ordering is not representable by the current
  single-owner UI architecture. The underlying no-old-publication invariant is
  **PASS** by deterministic generation/cancellation tests and live restart
  evidence.

## 13. No retry

- Press: never automatically replayed.
- Toggle: never automatically replayed.
- Activate: never automatically replayed.
- Desired-state operations: Set Text, Set Value, and Expand intent do not
  migrate across generations.
- Generation change: retires old intent and requires a fresh binding/ticket.
- Automatic replay: **NO**.
- Expected: **NO**.

## 14. Task ownership

- Operation-associated tasks: action-invocation task, bounded transition
  observer, selection observation, and external-text completion are covered.
- JoinHandles: the existing action invocation handle remains locally owned and
  is aborted after a decisive observation when still running; existing capture
  and modality handles retain their existing local ownership.
- Cancellation: ticket tokens remove publication authority; application loss
  additionally cancels modality/capture ownership already held by the TUI.
- Detached work: an underlying backend call may finish, but has no independent
  route to mutable runtime state or publication after its owner returns.
- Late publication: every affected path is gated or discarded.
- What remains for 0.6C: general event-producer JoinHandle ownership, history
  bounds, cache/resource churn, and event lifecycle qualification.
- What remains for 0.6D: terminal panic/suspend behavior, external process and
  artifact cleanup breadth, and terminal/external lifecycle qualification.

No `TaskSupervisor`, task registry, background-work manager, or runtime manager
was introduced.

## 15. v0.4/v0.5 reuse

- Transition observer: the existing single bounded observer remains the
  operation-adjacent verifier.
- Fresh reads: remain exact and authoritative; exact-target read failure is
  stale and timeout remains pending/timeout, never success.
- Scope: captured/current `InteractionScope` continues to confine operation
  authority.
- Locator: exact `BackendLocator` prevents both cross-generation and
  same-generation replacement migration.
- Selection: existing exact target and target-specific readback are retained,
  now with final publication gating on every completion/error path.
- Task truth: later application state does not rewrite primitive operation
  truth.
- Manual continuation: fresh scenes and fresh user operations remain the only
  continuation mechanism.

## 16. Race audit

- Operation vs target disappearance: exact target locator/readback returns
  `Stale`; no replacement search or confirmation.
- Operation vs app exit: exact unique-owner liveness invalidates the generation
  and drains tickets before observer refresh/publication.
- Operation vs app restart: same name/title cannot satisfy A1's exact owner or
  generation; G2 requires fresh recovery.
- Observer vs cancellation: authority evaluation plus final ticket completion
  refuses confirmation.
- External text vs replacement: handler return synchronizes liveness, final
  ticket refusal precedes writeback/publication, and candidate is only a
  recovery artifact.
- Old result vs new status: operation paths cannot publish without a current
  ticket; the concurrent R2-before-O1 ordering is structurally unavailable in
  the single-owner TUI.
- Shutdown vs operation: shutdown drains/cancels tickets; deterministic registry
  test proves all 32 owners cancel and no late completion can publish.

## 17. Genericity audit

- App name: validation selector only; never production authority.
- Window title: validation description only.
- Toolkit: no production branch.
- PID: unused for authority.
- `RuntimeNodeId`: presentation/snapshot identity only.
- Name matching: no replacement reauthorization.
- Geometry: unused for semantic authority.
- Index: unused as durable identity.
- Timing: barriers/log markers establish ordering; waits only bound completion.
- Retry: no semantic retry.
- Private APIs: none.
- Input injection: none.
- Result: **PASS**.

The production liveness addition uses only the exact unique bus owner already
contained in `BackendLocator`; the registry check remains a second exact-root
check. No fixture name, title, label, toolkit, geometry, PID, or timing appears
in production policy.

## 18. Required live evidence

- Blocked action: **PASS** — invocation returned but observer remained pending
  until A1 termination.
- Same-looking restart: **PASS** — same app/title/labels, distinct exact owner,
  no mutation or confirmation in A2.
- Target replacement: **PASS** — same generation and same label/role, distinct
  locator, old result stale, fresh replacement usable.
- Cancellation: **PASS** — deterministic token cancellation unit.
- External edit: **PASS** — delayed candidate refused after app replacement;
  private recovery artifact retained.
- Fresh G2 usability: **PASS** — fresh Toggle and external edit both work.
- Status ordering: **NOT APPLICABLE** for live R2-before-O1 concurrency; final
  publication isolation independently passed.
- Shutdown: **PASS** — deterministic active-registry shutdown cancellation and
  bounded live TUI exits without crash/hang.

## 19. Required result summary

```text
LATE_OPERATION_GENERATION_ISOLATION=PASS
OPERATION_APP_EXIT_LATE_COMPLETION_ISOLATED=PASS
FRESH_GENERATION_AFTER_LATE_WORK_USABLE=PASS
LATE_TARGET_REPLACEMENT_LOCATOR_ISOLATION=PASS
GENERATION_INVALIDATION_RETIRES_ACTIVE_TICKETS=PASS
CANCELLED_OPERATION_LATE_CONFIRMATION_REFUSAL=PASS
EXTERNAL_TEXT_FINAL_TICKET_STALE_REFUSAL=PASS
LATE_OLD_STATUS_CANNOT_OVERRIDE_CURRENT=NOT_APPLICABLE
ACTIVE_OPERATION_SHUTDOWN_ISOLATION=PASS
NO_NONIDEMPOTENT_REPLAY=PASS
NO_NEW_AUTHORITY_IDENTITY=PASS
```

## 20. Existing capability regression

- Text: existing source suite passed.
- Toggle/action: live G2 Toggle plus existing source suite passed.
- Value: existing source suite passed.
- Selection: existing source suite passed.
- Table row: existing source suite passed.
- PageTab: representative GTK and Qt v0.5B live evidence passed.
- Expand: representative expand/collapse v0.5B live evidence passed.
- External text: stale-return and fresh-G2 live paths passed.
- Password: existing source suite passed; no export rule changed.
- v0.4 transition/scope: existing source suite and representative v0.5B live
  continuation/stale-target evidence passed.
- v0.5 task primitives: representative hierarchy/PageTab campaign passed; full
  source suite covers selection and chooser composition.

No full historical live campaign was repeated.

## 21. Tests / quality

- New focused tests: 2 — retired generation cannot confirm even if a late
  cache state matches; explicitly cancelled ticket cannot publish and releases
  registry capacity.
- Targeted checks: focused Rust tests, existing bounded-registry shutdown test,
  Python compilation, shell syntax, formatting, and diff checks passed during
  implementation.
- Linux live: isolated D-Bus/Xvfb run of `tests/live/v06a_run_linux.sh` passed
  all required generation/restart/locator/external-text cases.
- macOS: final source-quality matrix passed on the supported development host;
  AT-SPI live evidence is Linux-only.
- fmt: **PASS** — `cargo fmt --all -- --check`.
- check: **PASS** — `cargo check --all-targets`.
- test: **PASS** — `cargo test --all-targets`.
- clippy: **PASS** — `cargo clippy --all-targets -- -D warnings`.
- docs: **PASS** — `python3 scripts/check-docs.py`.
- diff: **PASS** — `git diff --check`.

The full source matrix ran once at final phase close after production and
documentation were complete.

## 22. Remaining issues

- P0: none.
- P1: none.
- P2: none specific to 0.6A.

Planned reconnect/application recovery (0.6B), general event/cache/resource
ownership (0.6C), and external/terminal lifecycle breadth (0.6D) are not 0.6A
defects.

## 23. Architecture conclusion

- Did `RuntimeSessionId + ApplicationGenerationId + BackendLocator` remain
  sufficient? **YES**.
- Was `RuntimeEpochId` required? **NO**.
- Was `SurfaceGenerationId` required? **NO**.
- Was a task supervisor/runtime manager required? **NO**.

Stable late-work rule: `ApplicationGenerationId` is the application-authority
epoch inside a `RuntimeSession`. Work started under a retired generation may
finish internally but cannot mutate, confirm, publish, or overwrite into a
newer generation. Cancellation removes publication authority even if backend
work later returns. `BackendLocator` remains exact target authority within a
generation.

## 24. v0.6 roadmap

- Discovery: **COMPLETE**.
- 0.6A: **COMPLETE / VALIDATED**.
- 0.6B: **PLANNED / NOT AUTHORIZED**.
- 0.6C: **PLANNED / NOT AUTHORIZED**.
- 0.6D: **PLANNED / NOT AUTHORIZED**.
- 0.6E: **PLANNED / NOT AUTHORIZED**.

Only 0.6A changed status. No later phase is automatically authorized.

## 25. Recommended next direction

**A. Recommend 0.6B — Transport and Application Recovery.**

Do not start it without explicit user authorization.

## 26. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`
- HEAD: the evidence/docs commit containing this handoff
- Worktree: clean at handoff
- Remote: synchronized at handoff
- Package version: `0.3.0` unchanged
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, and `v0.3.0` unchanged
- v0.6.0 tag/release: none

## 27. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone `HANDOFF.md`、v0.5
milestone `HANDOFF.md`、`docs/planning/v0.6-runtime-continuity.md`、
`docs/planning/v0.6-roadmap.md`，以及本 0.6A `HANDOFF.md`。分支为
`v0.6/runtime-continuity-multi-surface`；精确 HEAD 见本 handoff 所在提交。
v0.4、v0.5 均为 COMPLETE / MILESTONE QUALIFIED。v0.6 Discovery 结论 A
仍成立：`RuntimeSessionId + ApplicationGenerationId + BackendLocator` 是
充分的 authority model，不要添加 `RuntimeEpochId` 或
`SurfaceGenerationId`。0.6A 已 COMPLETE / VALIDATED：retired generation
work 可以在内部结束，但不得向新 generation mutate、confirm、publish 或
overwrite；即使后端工作稍后返回，cancellation 也已移除 publication
authority；`BackendLocator` 仍是同 generation 内 exact target authority；
不得自动重放非幂等操作；external-text 最终成功必须持有当前有效的 final
ticket authority。建议下一方向仅为 0.6B Transport and Application Recovery，
0.6B **没有自动授权**。v0.6 仍是 internal milestone，不做 RC、tag 或
release；v0.7 仍属后续；v1.0.0 仍是下一计划公开版本。不要自行扩展范围。
