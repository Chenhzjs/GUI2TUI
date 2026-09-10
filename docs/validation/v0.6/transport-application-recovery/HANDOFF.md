# GUI2TUI v0.6 Phase 0.6B — Transport and Application Recovery

## 1. Status

- Phase: 0.6B — Transport and Application Recovery
- Starting HEAD: `b87805982d8f97553fe04a937d805681cc3db9fd`
- Final HEAD: the evidence/docs commit containing this handoff
- Branch: `v0.6/runtime-continuity-multi-surface`
- Worktree: clean at handoff
- Production changes: exact current-enumeration application selection,
  duplicate-name refusal, explicit A-to-B selection, and transport-only
  bounded reconnect
- New identity layers: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.6B TRANSPORT AND APPLICATION RECOVERY VALIDATED**

v0.6 remains internal. No package version, RC, tag, release artifact, public
release, process supervision, semantic retry, or later v0.6 phase was started.

## 2. Commits

- Production/tests: `37207bc` (`fix: separate transport recovery from app
  authority`)
- Evidence/docs: `docs: validate v0.6 application recovery` (the commit
  containing this handoff)

## 3. Recovery architecture

- Transport state: says only whether the current AT-SPI connection can
  communicate and enumerate applications.
- Application authority: one exact `ApplicationRef` selected from the current
  enumeration and opened as a fresh application generation.
- `RuntimeSessionId`: retained within the running GUI2TUI process, including
  reconnect and application switching.
- `ApplicationGenerationId`: retired on loss/switch and freshly allocated only
  after exact current application selection succeeds.
- `BackendLocator`: exact current application/object authority inside that
  generation.
- Fresh scene: bootstrap builds a new cache, event subscription, scopes,
  focus, commands, choices, and `TuiScene`; no old scene is reactivated.
- New Recovery subsystem: **NO**.
- New identity: **NO**.

The Discovery result remains **SUFFICIENT WITH SMALL HARDENING**.

## 4. Transport vs semantic authority

Reconnect establishes a usable AT-SPI connection and a fresh public
enumeration. It does not select an application, restore a generation, attach
old locators, or revive bindings, focus, scopes, commands, tickets, or
operations. Exact continuity could be accepted only if the same exact current
backend application instance were provable across the loss. The current stack
cannot prove that across an accessibility-bus lifecycle change, so it
conservatively requires fresh user selection. Descriptive names, titles, PIDs,
indices, controls, layout, and geometry are never continuity proof.

## 5. Initial selection vs recovery

- Initial selector: `--app NAME` keeps its documented convenience against one
  fresh enumeration; exactly one case-insensitive exact match may open.
- Recovery selector: no remembered name exists as authority. F5 after restored
  transport and `b` both open a current enumeration.
- Descriptive name: display/search text only after authority loss.
- Exact backend owner: carried by the selected current `ApplicationRef`.
- Duplicate name: exact-name selection now returns `AmbiguousApplication`
  rather than the first entry.
- Fresh selection: one explicit selector row yields its exact locator and the
  same backend snapshot that enumerated it.

Current display index distinguishes rows for the user only. It is discarded
with that enumeration and never persists as application identity.

## 6. Application switching

- A: PyQt6 `gui2tui-v06b-app-a`, `Application A`, `A control`.
- B: simultaneous PyQt6 `gui2tui-v06b-app-b`, `Application B`, `B control`.
- Old generation: A/G1 is invalidated before B is installed.
- Fresh generation: B/G2 is opened from its exact current selector entry.
- Old A binding: unreachable after whole-view replacement; it mutates neither
  A nor B.
- Fresh B binding: Toggle changes only B's independently observed state.
- Return to A: explicit B-to-A selection opens A/G3. Even though the same A
  process/locator survives, the original G1 binding does not regain validity.
- Result: **PASS**.

## 7. Duplicate-name recovery

- Instances: two simultaneous independent PyQt6 processes.
- Names: both `gui2tui-v06b-duplicate`.
- Titles: both `Duplicate Window`.
- Backend owners: two distinct exact current locators, proven by indexed
  inspection.
- Recovery condition: explicitly select one, mutate it, terminate it while
  its same-looking peer remains.
- Automatic selection: **NO**; runtime becomes unavailable.
- Expected: **NO unless exact continuity is proven**.
- User selection: current selector exposes both entries with display-only
  enumeration numbers; the survivor is authorized only by a new choice.
- Wrong-app mutation: none before selection; after selection exactly the
  chosen survivor changes.
- Result: **PASS**.

## 8. No-application state

- Selected app: the remaining duplicate fixture after explicit recovery.
- Exit: selected app and every other controlled fixture are terminated while
  AT-SPI remains healthy.
- Transport: remains available and enumerates zero controlled applications.
- Old scene: retained only as non-actionable display storage; application
  availability is false and generation is absent.
- Old command: ordinary activation input is refused before dispatch.
- Current UX: `Application is no longer available` with refresh/application
  choice guidance; no internal IDs are shown.
- Fresh application appears: `gui2tui-v06b-fresh` with a fresh control.
- Recovery: refresh the already-open selector, explicitly choose its exact
  current row, open a new generation, then perform a fresh Toggle.
- Result: **PASS**.

## 9. Transport interruption

- Environment: isolated Ubuntu Linux OrbStack VM, Xvfb, session D-Bus, custom
  public `org.a11y.Bus` activation wrapper, AT-SPI registry, and PyQt6 fixture.
- Interruption: terminate the real accessibility bus launcher, registry, and
  accessibility bus while a public Toggle path is active; activation is
  deterministically refused by a marker.
- Retry: existing 100/200/400/800 ms sequence remains unchanged and bounded.
- Generation: G1 is invalidated and remains absent after reconnect.
- Old work: returns unverifiable/retired and is never replayed or attributed to
  a recovered view.
- Current state: safely unavailable after retry exhaustion.
- Transport restore: remove the activation refusal and explicitly press F5;
  successful connect/enumeration restores transport only.
- Fresh enumeration: current same-looking A2 is visible, with no implicit
  selection.
- Fresh generation: explicit selector choice opens G2.
- Fresh operation: `Fresh toggle` succeeds and authoritative GUI state is
  checked.
- Result: **PASS**.

## 10. Extended outage

- Duration relative to retry window: transport remained unavailable through
  the complete 1.5-second backoff window and for an additional controlled
  observation interval.
- Retry termination: one exhaustion record reports exactly four attempts;
  its count remains unchanged afterward.
- Runtime state: degraded, endpoint unavailable, and generation `null`.
- CPU/busy loop: no repeated reconnect/exhaustion activity was observed after
  the bounded loop ended; this was a bounded behavioral check, not a resource
  benchmark.
- Old authority: invalidated before reconnect attempts.
- Explicit recovery: after service restoration, F5 reconnects, then a separate
  explicit application choice establishes G2.
- Result: **PASS**.

## 11. Old binding after recovery

- Old binding: G1 binding for the first same-looking runtime fixture.
- Loss: full AT-SPI transport interruption and application replacement.
- Recovery: transport-only F5 leaves application unavailable.
- New generation: created only after exact current selection.
- Old command: input before that selection is refused.
- Mutation: replacement application's fresh checkbox remains unchecked.
- Fresh binding: newly compiled G2 binding toggles it successfully.
- Result: **PASS**.

Pre-loss command palette entries are cleared by invalidation and the recovered
view builds a new hierarchy; there is no dormant old command object to invoke.

## 12. Operation resurrection

- Old operation: public `Replaceable toggle` action started immediately before
  transport interruption.
- Loss: accessibility transport is terminated while the fixture action path
  is held at a deterministic marker.
- 0.6A retirement: the interrupted invocation returns `Unverifiable` as the
  transport fails and retires its ticket; application liveness then invalidates
  G1 and any operation still owned by it.
- Recovery: reconnect performs no command continuation.
- Late return: transport failure may finish underlying work internally as
  unverifiable; it is never treated as current success.
- New generation: G2 exists only after explicit selection.
- Confirmation: no G1 `Confirmed` result in G2.
- Mutation: no interrupted command is issued to A2.
- Result: **PASS**.

The deterministic 0.6A regression
`retired_generation_cannot_confirm_even_if_late_state_matches` independently
covers a result returning after generation replacement.

## 13. Exact surviving instance

- Was tested: no automatic same-process continuation was claimed.
- Exact backend evidence: a locator is exact only inside its live transport
  and generation contract. After a new accessibility bus starts, textual
  unique-owner/object-path values may be reused and cannot prove survival.
- Same application process: not required for the hard gates.
- Recovery behavior: every transport reset remains selection-required even if
  current applications look identical.
- Conclusion: **SAFE_SELECTION_REQUIRED**.

## 14. Application-generation policy

- Initial selection: fresh G1.
- A-to-B switch: retire G1, open B/G2.
- App restart: retire old generation; fresh explicit selection opens a new one.
- Transport reconnect: leaves generation absent; selection opens a fresh one.
- Same-window recreation: no application-generation change; exact locator and
  scope rules continue to handle target/surface replacement.
- Fresh generation rules: only a selected exact current application may open
  one; a connection or descriptive match cannot.

## 15. Cache/event/scope replacement

- Cache: freshly bootstrapped; never incrementally patched across generations.
- Event subscription: old subscription is closed; fresh exact application gets
  a new subscription before bootstrap.
- `InteractionScope`: recompiled from the fresh cache.
- Focus: initialized from fresh scene evidence; no old locator/name restore.
- `SceneBinding`: freshly compiled and generation-scoped.
- Command palette: cleared at invalidation and rebuilt from fresh commands.
- What is fresh after recovery: application cache, content semantics,
  subscription, scopes, focus, scene, bindings, commands, choices, and
  application generation.

General event producer JoinHandle/resource ownership remains 0.6C.

## 16. Recovery UX

- Unavailable: says the application or desktop accessibility service is
  unavailable and offers F5, application choice, diagnostics, or quit.
- Selection required: restored connection says to choose a current
  application; it never claims the previous app was restored.
- Ambiguous application: reports multiple current matches rather than choosing.
- Recovered transport: reports accessibility connection restored only.
- Fresh app selected: returns to the ordinary semantic scene.
- Internal IDs leaked: **NO** in normal TUI wording; exact details remain in
  diagnostics/product logs.

## 17. No replay

- Press: not replayed.
- Toggle: not replayed.
- Activate: not replayed.
- Selection: not replayed.
- Desired state: not migrated across generations.
- Interrupted operations: retired by 0.6A.
- Recovery: reconstructs current truth and waits for new user intent.
- Automatic semantic replay: **NO**.
- Expected: **NO**.

## 18. 0.6A reuse

- Generation invalidation: runs before app replacement or reconnect.
- Tickets: all G1-owned tickets are cancelled/drained.
- Late completion: final ticket gate rejects retired publication.
- External text: 0.6A live regression confirms stale candidate refusal across
  restart under the new explicit-selection UX.
- Publication authority: never follows a name, recovered connection, or fresh
  scene similarity.
- Fresh-generation usability: both recovered Toggle and external edit pass.

## 19. Genericity audit

- App name authority: **NO** after initial selection/loss.
- Window title authority: **NO**.
- PID authority: **NO**.
- Toolkit: no production branching.
- Geometry: **NO** authority.
- `RuntimeNodeId`: no cross-generation recovery.
- Old index: never retained; current selector index is display-only.
- Descriptive selector: filter/convenience, not recovery continuity.
- Blind retry: none.
- Private APIs: none.
- Input injection: none.
- Result: **PASS**.

## 20. Tests / quality

- New focused tests: 2 — duplicate exact application names are ambiguous; two
  same-name selector entries retain distinct exact current locators.
- Targeted: new tests plus retired-generation completion, cancellation,
  registry shutdown, exact Selection, PageTab, Action resolution, modal scope,
  and stale-focus tests passed.
- Linux live: `tests/live/v06b_run_linux.sh` passed application switching,
  duplicate names, no-app, transport interruption, extended outage, explicit
  recovery, old-binding refusal, operation retirement, and fresh operation.
  Updated `tests/live/v06a_run_linux.sh` also passed the representative 0.6A
  restart, locator, ticket, and external-text campaign with explicit recovery.
- macOS: final supported source-quality matrix passed on the development host.
- fmt: **PASS** — `cargo fmt --all -- --check`.
- check: **PASS** — `cargo check --all-targets`.
- test: **PASS** — `cargo test --all-targets`.
- clippy: **PASS** — `cargo clippy --all-targets -- -D warnings`.
- docs: **PASS** — `python3 scripts/check-docs.py`.
- diff: **PASS** — `git diff --check`.

The full source matrix ran once at final phase close after production and
documentation were complete.

## 21. Required result summary

```text
APP_SWITCH_OLD_BINDING_REFUSAL=PASS
APP_SWITCH_FRESH_BINDING_RECOVERY=PASS
DUPLICATE_NAME_RECOVERY_NO_GUESS=PASS
NO_APP_STATE_SAFE=PASS
NO_APP_FRESH_SELECTION_RECOVERY=PASS
TRANSPORT_INTERRUPTION_RECOVERY=PASS
EXTENDED_OUTAGE_SAFE_UNAVAILABLE=PASS
EXPLICIT_RECOVERY_AFTER_OUTAGE=PASS
RECONNECT_OLD_BINDINGS_REFUSED=PASS
RECOVERED_FRESH_OPERATION_USABLE=PASS
RECOVERY_DOES_NOT_RESURRECT_OPERATION=PASS
TRANSPORT_RECOVERY_REQUIRES_FRESH_GENERATION=PASS
TRANSPORT_RECOVERY_NOT_SEMANTIC_REAUTH=PASS
INITIAL_SELECTOR_RECOVERY_AUTHORITY_SEPARATED=PASS
EXACT_SURVIVING_INSTANCE_RECOVERY=SAFE_SELECTION_REQUIRED
NO_NEW_AUTHORITY_IDENTITY=PASS
NO_NONIDEMPOTENT_REPLAY=PASS
```

## 22. Existing capability regression

- 0.6A: representative Linux late generation, app exit, ticket retirement,
  same-generation locator replacement, fresh use, and external stale-write
  refusal all passed.
- Selection: exact-target resolution test passed.
- PageTab: exact current tab/no-index resolution test passed.
- Action: role-aware semantic preference test and live Toggles passed.
- v0.4 scope: modal scope confinement/restoration test passed.
- Fresh scene: A-to-B-to-A and post-transport fresh scenes worked.
- External text: stale return was refused; a fresh selected generation then
  completed a new edit.

No full historical milestone campaign was repeated.

## 23. Remaining issues

- P0: none.
- P1: none.
- P2: none specific to 0.6B.

Planned general event/resource/history bounds (0.6C) and external/terminal
lifecycle breadth (0.6D) are not 0.6B defects.

## 24. Architecture conclusion

- Did existing identity architecture remain sufficient? **YES**.
- Did recovery require a new identity? **NO**.
- Did recovery require a Recovery subsystem? **NO**.

Final recovery rule: transport recovery restores communication, not semantic
authority. Every recovered or reselected application view establishes a fresh
`ApplicationGenerationId` and fresh bindings around one exact current backend
application. Old authority never reactivates. Descriptive names may assist
selection but cannot prove continuity; ambiguity or unprovable survival
requires fresh user selection. Recovery reconstructs truth and never replays
old operations.

## 25. v0.6 roadmap

- Discovery: **COMPLETE**.
- 0.6A: **COMPLETE / VALIDATED**.
- 0.6B: **COMPLETE / VALIDATED**.
- 0.6C: **PLANNED / NOT AUTHORIZED**.
- 0.6D: **PLANNED / NOT AUTHORIZED**.
- 0.6E: **PLANNED / NOT AUTHORIZED**.

Only 0.6B changed status. No later phase is automatically authorized.

## 26. Recommended next direction

**A. Recommend 0.6C — Surface, Event, Cache, and Resource Continuity.**

Do not start it without explicit user authorization.

## 27. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`
- HEAD: the evidence/docs commit containing this handoff
- Worktree: clean at handoff
- Remote: synchronized at handoff
- Package version: `0.3.0` unchanged
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, and `v0.3.0` unchanged
- v0.6.0 tag/release: none

## 28. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone `HANDOFF.md`、v0.5
milestone `HANDOFF.md`、`docs/planning/v0.6-runtime-continuity.md`、
`docs/planning/v0.6-roadmap.md`、0.6A `HANDOFF.md`，以及本 0.6B
`HANDOFF.md`。分支为 `v0.6/runtime-continuity-multi-surface`；精确 HEAD
为本 handoff 所在 evidence/docs 提交（交接时以 `git rev-parse HEAD`
核验）。v0.4、v0.5 均为 COMPLETE / MILESTONE QUALIFIED；v0.6 Discovery
结论 A 仍成立；0.6A 与 0.6B 均 COMPLETE / VALIDATED。
`RuntimeSessionId + ApplicationGenerationId + BackendLocator` 仍是 authority
model；transport recovery 不等于 semantic reauthorization；每个 recovered
或 reselected application view 都使用 fresh generation 和 fresh bindings；
descriptive name 不能证明 continuity，duplicate-name recovery 绝不猜测，
old bindings 绝不重新激活，也不重放 old operation；无法证明 exact
continuity 时要求 fresh user selection 是安全且已验证的策略。建议下一方向仅为
0.6C Surface, Event, Cache, and Resource Continuity，0.6C **没有自动授权**。
v0.6 仍是 internal milestone，不做 RC、tag 或 release；v0.7 仍属后续；
v1.0.0 仍是下一计划公开版本。不要自行扩展范围。
