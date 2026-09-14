# GUI2TUI v0.6 Phase 0.6C — Surface, Event, Cache, and Resource Continuity

## 1. Status

- Phase: 0.6C — Surface, Event, Cache, and Resource Continuity
- Starting HEAD: `0940835ba93c70adc32bbd8a22ccb299a7d4b113`
- Final HEAD: the evidence/docs commit containing this handoff
- Branch: `v0.6/runtime-continuity-multi-surface`
- Worktree: clean at handoff
- Production changes: application-view ownership of the event producer,
  bounded retirement, current-structure pruning for focus/command history,
  resource diagnostics, and healthy transport reuse during fresh application
  selection
- New identity layers: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.6C SURFACE, EVENT, CACHE, AND RESOURCE CONTINUITY
  VALIDATED**

v0.6 remains internal. No terminal/external lifecycle work, 0.6E campaign,
version change, RC, tag, package, or release was started.

## 2. Commits

- Production/tests: `bf49d11` (`fix: bound runtime event and surface state`)
- Evidence/docs: `docs: validate v0.6 runtime resource continuity` (the commit
  containing this handoff)

## 3. Runtime ownership model

- Application view: `TuiApplication` owns one exact current generation and
  its cache, events, scopes, histories, scene, and runtime operation registry.
- Event producer: directly owned by its `EventSubscription` as one retained
  `JoinHandle`; retirement closes the receiver and observes producer exit.
- Event queue: owned by the subscription; bounded MPSC capacity is 2,048 for
  the product path, and every fresh application view gets a distinct queue.
- Semantic cache: owned by the application view and represents the current
  bootstrap/reconciled GUI structure; generation replacement rebuilds it.
- Content cache: application-view `ContentRuntime`, already bounded to 512 KiB
  and 256 ranges and safely re-fetchable.
- `InteractionScope`: freshly analyzed from the current cache and relational
  graph; only its current active scope contributes operation eligibility.
- Focus history: presentation-only exact `(scope, RuntimeNodeId, locator)`
  anchors; pruned to current reachable scopes and exact current locator
  membership and cleared on application retirement.
- Recent commands: presentation-only usage counters keyed by current
  `RuntimeNodeId`; pruned to sources in the freshly compiled current command
  hierarchy and cleared on application retirement.
- `SceneBinding`s: owned by the current `TuiScene`; dispatch still performs
  current generation, exact locator, scope, capability, and state checks.
- Operation registry: bounded to 32 by `RuntimeSession`; 0.6A invalidation
  cancels/drains it, and the live campaign observed zero active tickets at
  every settled sample.

History and presentation values have no independent authority.

## 4. Event producer lifecycle

- Creation: `subscribe_events` registers public AT-SPI event families, creates
  a fresh bounded queue, and spawns exactly one producer for the selected
  application's exact bus owner.
- Owner: the returned `EventSubscription` retains the producer handle.
- Cancellation/closure: application retirement closes the receiver, which is
  an independent wakeup through `sender.closed()`.
- Join/exit observation: `shutdown` waits up to 250 ms; a slow producer is
  aborted and awaited as a bounded fallback. `Drop` closes and aborts as a
  last-resort safety net.
- Application switch: old generation/tickets are invalidated first, then the
  old producer exits before a fresh selected view is installed.
- Application loss: application-owned histories/overlays are cleared and the
  producer is shut down after authority invalidation.
- Reconnect: 0.6B fresh selection constructs a new generation, subscription,
  queue, cache, scope, and scene. A healthy transport connection may be reused
  only for fresh public enumeration; it carries no application authority.
- Shutdown: the product loop explicitly awaits `TuiApplication::shutdown`.
- Old producer influence: none; it retains only its old sender and exact old
  bus filter, and receiver closure prevents delivery into any replacement
  view.

No task supervisor or event manager was introduced.

## 5. Event overflow contract

- Capacity: 2,048 product events.
- Overflow detection: nonblocking send failure sets the shared resync flag,
  counts dropped events, and wakes the consumer.
- Discard policy: the incomplete queued prefix/suffix is drained rather than
  interpreted as a complete history.
- Dirty/resync: `ResyncRequired` coalesces overflow, including one overlapping
  burst, into authoritative full snapshots.
- Fresh read: `refresh_from_backend(Full, ...)` re-reads current public AT-SPI
  semantics.
- Current scene: rebuilt from the fresh semantic/cache baseline and current
  scope, never from event replay.
- Operation after overflow: closing the current modal and a fresh `Activate
  safely` action both succeeded.
- Result: **PASS**.

Events remain bounded wakeups; fresh reads establish truth.

## 6. Old-event isolation

- Old app/generation: Qt fixture under G1.
- Queued event: modal open/close activity was emitted after switching away.
- Switch/replacement: explicit selection installed the GTK fixture as a fresh
  generation with a distinct receiver/producer pair.
- Fresh app: GTK event statistics were sampled before and after old Qt
  activity.
- Old event effect: current receiver `received` count did not change, cache and
  scope did not change, and no old command became current.
- Result: **PASS**.

## 7. Focus-history contract

- Purpose: restore usability when returning between current reachable scopes.
- Key: exact current `InteractionScopeId`; value includes `RuntimeNodeId` and
  `BackendLocator`.
- Authority: none. Focus reconciliation still requires exact current scene and
  locator evidence; operation dispatch independently revalidates authority.
- Previous growth behavior: entries could survive every vanished scope for a
  long-lived application generation.
- New bound/pruning: after every full/view rebuild, retain only current scopes
  whose remembered node still belongs to that scope and whose cached locator
  exactly equals the anchor. Application loss/reselection clears all entries.
- Eviction: vanished scope, moved node, or replacement locator is removed.
- Restoration: exact eligible current anchor, otherwise existing safe fallback.
- Stale locator: cannot follow a reused `RuntimeNodeId` to a replacement.
- Rationale: useful history is bounded by current reachable scopes, rather
  than an arbitrary numerical cap or the number of historical surfaces.

## 8. Recent-command contract

- Purpose: rank current command navigation by recent successful use.
- Stored data: `RuntimeNodeId -> saturating u32 use count`.
- Authority: none; command entries retain exact locators and invocation still
  performs current binding/scope/capability revalidation.
- Previous growth: unique vanished command sources could accumulate throughout
  one long-lived generation.
- New bound/pruning: retain only sources present in the freshly compiled
  current command hierarchy; prune on rebuild and after each success.
- Eviction: disappearance from the current hierarchy removes the counter.
- Revalidation: unchanged exact current command validation.
- Generation replacement: all recency is cleared.
- Rationale: recency is useful only for commands the current GUI exposes, so
  current hierarchy cardinality is the semantic bound; no magic cap is needed.

## 9. Semantic/cache lifecycle

- Arena: current semantic nodes only; full refresh replaces the baseline and
  incremental updates remove vanished subtrees.
- Locator map: remains one-to-one with live cache nodes in evidence.
- Content cache: retained within the current view under its existing 512 KiB /
  256-range budget.
- Scene bindings: regenerated from the current cache; old frozen values remain
  harmless because dispatch revalidates them.
- Subtree replacement: old exact locator is removed; same-looking replacement
  receives only a fresh locator/binding.
- Generation replacement: fresh cache/content/scopes/commands/choices/scene;
  histories clear and old event work retires.
- Presentation reconciliation: may preserve snapshot-scoped presentation
  identity only; it cannot transfer locator authority.
- Stale authority: none observed under modal, window, overflow, or app-view
  replacement.
- Growth: logical cache size tracked the current fixture (8 Qt nodes or 52 GTK
  nodes), not the number of historical surfaces/generations.

## 10. Surface churn evidence

- Fixture: public PyQt6 accessibility fixture
  `gui2tui-v06c-resource`.
- Cycles: 50 same-looking modal open/operate/close replacements, followed by
  modal stale-command and secondary-window replacement cases.
- Surface types: main window, modal dialog, simultaneous secondary top-level
  window, and same-looking replacement surfaces.
- Initial logical counts: 8 nodes, 8 locators, 3 bindings, 2 scopes, focus
  history 0, recency 0, event depth 0, tickets 0.
- Peak: transient surfaces were refreshed during each cycle; settled midpoint
  and cycle 50 remained 8 nodes/8 locators, focus history 1, recency 1.
- Final: 8 nodes, 8 locators, 3 bindings, 2 scopes, focus history 1, recency 2,
  event depth 0, tickets 0.
- Scope: every modal confined current interaction and returned to the current
  main window scope after close.
- Focus: exact current anchors restored; replacement anchors did not inherit.
- Bindings: frozen modal and window commands were refused after exact
  replacement.
- Commands: recency stayed bounded by current hierarchy.
- Events: queue returned to depth zero and current producer remained owned.
- Tickets: zero at every recorded settled point.
- Fresh operation: final fresh Qt Toggle succeeded.
- Result: **PASS**.

## 11. Multi-window churn

- W1: persistent main `Continuity Window`.
- W2: simultaneous secondary top-level window deliberately using the same
  title and labels on recreation.
- Current authority: current scope plus exact locator/capability evidence.
- Switch: public focus/scope evidence moved authority to W2.
- Close: W2 was closed externally while its palette command was frozen.
- Replacement: a same-looking W2 with a new exact locator was opened.
- Old command: refused and did not close or mutate replacement W2.
- Fresh command: replacement Toggle worked, then fresh close returned safely
  to W1.
- New identity layer: **NO**.
- Result: **PASS**.

## 12. Event overflow + surface convergence

- Burst: existing GTK public fixture emitted a real high-volume property and
  selection storm against the normal 2,048-capacity queue.
- Overflow: product diagnostics observed at least one real resync request.
- Surface change: a modal dialog was opened during the burst.
- Fresh resync: incomplete event history was drained and the application was
  reread through a full semantic snapshot.
- Final scope: `ModalDialog` was the authoritative active scope.
- Final scene: cache nodes equaled exact locator mappings and included the
  authoritative `Storm complete: 2000` state.
- Fresh action: current dialog close and subsequent `Activate safely`
  succeeded.
- Result: **PASS**.

## 13. Application-view churn

- Generations/switches: seven further explicit Qt/GTK selections after the
  initial app switch, reaching generation 9.
- Event task count: one owned/active current producer at every sample; every
  invalidation recorded graceful old-producer exit.
- Cache: alternated between the current Qt shape (8 nodes) and current GTK
  shape (52 nodes), without accumulation.
- Focus history: zero immediately after every generation replacement.
- Recent commands: zero immediately after every generation replacement.
- Old bindings: prior-view commands remained retired under 0.6B semantics.
- Fresh operation: final Qt operation succeeded.
- Result: **PASS**.

Transport reuse in the selector is intentionally transport-only: the exact
application still comes from a new public enumeration and explicit choice,
and every choice creates a fresh generation and bindings.

## 14. Resource observations

Before / peak / final where available:

| Resource | Before | Peak/current alternate | Final |
| --- | ---: | ---: | ---: |
| semantic nodes | 8 | 52 on current GTK view | 8 |
| locator mappings | 8 | 52 on current GTK view | 8 |
| event tasks | 1 owned current | 1 owned current | 1 owned current |
| event queue | 0/2,048 | overflow -> full resync | 0/2,048 |
| focus history | 0 | 1 | 0 after final fresh view |
| recent commands | 0 | 2 | 0 after final fresh view |
| tickets | 0 | 0 at settled samples | 0 |
| FDs | 18 | 18 | 18 |
| threads | 13 | 13 | 13 |
| RSS | 19,592 KiB | 22,648 KiB | 22,648 KiB |

RSS rose modestly during initial lazy runtime/cache activity and then remained
flat across the seven final generation replacements. Logical ownership,
FDs, and threads—not byte-perfect allocator return—are the correctness basis.

## 15. RegionNavigator / presentation state

- Region lifecycle: `RegionNavigator` is derived on demand from the current
  layout plan and current scene; it owns no historical semantic objects.
- Scene rebuild: fresh layout/scene derives a fresh navigator projection.
- Vanished regions: absent from current focus order and therefore absent from
  navigation.
- `RuntimeNodeId`: presentation/snapshot identity only.
- Focus: reconciled by exact current locator or safe fallback.
- Authority: current `SceneBinding`, generation, locator, scope, capability,
  and backend state only.
- Result: **PASS**.

## 16. 0.6A reuse

- Ticket retirement: generation invalidation cancels/drains old tickets.
- Late work: cannot publish into a current generation.
- Cancellation: removes publication authority even if backend work returns.
- External text: its final ticket/locator gate remains unchanged.
- Publication authority: still requires the coherent final ticket plus
  operation-specific exact verification.

Representative retired-generation and explicit-cancellation regressions
passed; active ticket count remained zero throughout settled churn samples.

## 17. 0.6B reuse

- App switch: explicit selector changes application generation.
- Generation replacement: old view is invalidated before fresh construction.
- Transport recovery: communication and application authority remain
  separate.
- Fresh event subscription: each selected application view gets its own queue
  and producer, even when the healthy transport connection is reused.
- Old binding refusal: unchanged and exercised live across Qt/GTK views.
- Duplicate names: exact duplicate selection remains ambiguous; both focused
  duplicate-name regressions passed.

## 18. Genericity audit

- Toolkit: no production branching; Qt/GTK are evidence only.
- App name: no authority.
- Window title: no authority.
- PID: no authority.
- Geometry: presentation evidence only.
- `RuntimeNodeId` authority: none across locator/snapshot replacement.
- History authority: none.
- Name matching: never used to migrate surface/application authority.
- Timing: deterministic state/generation/log checks establish ordering; waits
  only bound completion or allow resource settling.
- Private APIs: none.
- Input injection: none; validation uses GUI2TUI commands and public AT-SPI
  inspection/activation.
- Result: **PASS**.

## 19. Required result summary

```text
EVENT_TASK_RETIREMENT=PASS
OLD_APP_EVENT_ISOLATION=PASS
EVENT_OVERFLOW_FRESH_RESYNC=PASS
SURFACE_CHURN_RUNTIME_CONTINUITY=PASS
FOCUS_HISTORY_BOUNDED=PASS
RECENT_COMMANDS_BOUNDED=PASS
MULTI_WINDOW_CHURN_AUTHORITY=PASS
OVERFLOW_SURFACE_CONVERGENCE=PASS
APPLICATION_VIEW_RESOURCE_RETIREMENT=PASS
CACHE_LOGICAL_GROWTH=BOUNDED
EVENT_TASK_GROWTH=BOUNDED
FOCUS_HISTORY_GROWTH=BOUNDED
RECENT_COMMAND_GROWTH=BOUNDED
FD_GROWTH=BOUNDED
THREAD_GROWTH=BOUNDED
RSS_GROWTH=OBSERVED_BOUNDED
NO_STALE_HISTORY_AUTHORITY=PASS
NO_NEW_SURFACE_IDENTITY=PASS
```

## 20. Existing capability regression

- 0.6A: retired-generation confirmation and explicit ticket-cancellation
  regressions passed; live tickets settled to zero.
- 0.6B: fresh A-to-B selection, generation replacement, old binding/event
  refusal, and fresh operation passed; duplicate-name focused tests passed.
- Selection: exact target/no-child-index regression passed.
- PageTab: exact current tab/no-retained-index regression passed.
- Action: fresh GTK action succeeded after real overflow.
- Expand: unchanged and covered by the full source suite.
- v0.4 scope: modal and simultaneous-window scope stayed current; exact stale
  focus regression passed.
- Fresh scene: authoritative full resync and fresh generation construction
  both passed.
- External text: 0.6A final-ticket path unchanged and full suite passed.

No full historical live milestone campaign was repeated.

## 21. Tests / quality

- New focused tests: 3 — owned event producer exits with its application view;
  command recency prunes to the current hierarchy; focus history retains only
  exact current scope members.
- Targeted: the three new tests plus retired generation, cancelled ticket,
  duplicate application names/selector rows, exact stale focus, Selection,
  and PageTab regressions passed.
- Linux live: `tests/live/v06c_run_linux.sh` passed 50 modal cycles,
  same-looking window replacement, current-scope operation, application-view
  churn, old producer/event isolation, real 2,048-capacity overflow/full
  resync, fresh post-overflow action, and bounded logical/FD/thread/RSS
  observations.
- macOS: final supported source-quality matrix passed on the development host;
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
- P2: none within 0.6C. Planned 0.6D external/terminal lifecycle and 0.6E
  integrated qualification are not 0.6C defects.

## 23. Architecture conclusion

- Did existing identity architecture remain sufficient? **YES**.
- Was `SurfaceGenerationId` required? **NO**.
- Was an Event/Resource manager framework required? **NO**.

Stable rules: events are bounded wakeups and overflow converges through fresh
semantic reads. Event work retires with its current application view. Focus
and command history is bounded by current reachable semantic/presentation
structure, remains non-authoritative, and clears across generation
replacement. Long-lived logical state is bounded by the current GUI plus
explicitly bounded non-authoritative caches/history.

## 24. v0.6 roadmap

- Discovery: **COMPLETE**.
- 0.6A: **COMPLETE / VALIDATED**.
- 0.6B: **COMPLETE / VALIDATED**.
- 0.6C: **COMPLETE / VALIDATED**.
- 0.6D: **PLANNED / NOT AUTHORIZED**.
- 0.6E: **PLANNED / NOT AUTHORIZED**.

Only 0.6C became complete. No later phase was started.

## 25. Recommended next direction

**A. Recommend 0.6D — External and Terminal Lifecycle.** Do not start it
without explicit user authorization.

## 26. Git status

- Branch: `v0.6/runtime-continuity-multi-surface`
- HEAD: the evidence/docs commit containing this handoff
- Worktree: clean
- Remote: synchronized at handoff
- Package version: `0.3.0` (unchanged)
- Public tags: unchanged; `v0.3.0` remains the latest planned pre-1.0 public
  release
- v0.6.0 tag/release: none

## 27. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4/v0.5 milestone HANDOFF、
`docs/planning/v0.6-runtime-continuity.md`、`docs/planning/v0.6-roadmap.md`、
0.6A/0.6B HANDOFF，以及最新 0.6C HANDOFF。分支为
`v0.6/runtime-continuity-multi-surface`，HEAD 为包含本 HANDOFF 的 evidence
commit；v0.4/v0.5 均为 COMPLETE / MILESTONE QUALIFIED。v0.6 Discovery
结论 A 仍成立，0.6A、0.6B、0.6C 均为 COMPLETE / VALIDATED；authority model
仍是 `RuntimeSessionId + ApplicationGenerationId + BackendLocator`。Events
始终只是 bounded wakeups，fresh reads 才建立 truth；event producer 生命周期
属于当前 application view，overflow 通过 fresh full resync 收敛。Focus/command
history 有明确边界且不具 authority；surface churn 依靠 exact locator + current
scope，不增加 generation ID。资源正确性按 logical ownership/bounds 判断，不要求
RSS byte-for-byte 回落。建议下一方向仅为 0.6D External and Terminal Lifecycle；
0.6D 未自动授权。v0.6 仍为内部 milestone，不做 RC/tag/release；v0.7 留待以后，
v1.0.0 仍是下一计划 public release。不要自行扩展范围。
