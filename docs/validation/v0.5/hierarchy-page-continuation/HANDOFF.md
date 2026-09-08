# GUI2TUI v0.5 Phase 0.5B — Hierarchy Reveal and Page Continuation

## 1. Status

- Phase: 0.5B — Hierarchy Reveal and Page Continuation
- Starting HEAD: `9217ef3e35dd4b3aef09bfd4e53aac96c8886d7b`
- Production commit: `5fd42744eb33f55d1ffc70bf05791bc708e96b09`
- Branch: `v0.5/task-interaction-completeness`
- Production changes: bounded semantic Expand/Collapse and exact current
  PageTab switching through the existing capability/operation pipeline
- New interaction families: qualified hierarchy reveal/hide and page switch
- P0: 0
- P1: 0
- Overall: **PHASE 0.5B HIERARCHY REVEAL AND PAGE CONTINUATION VALIDATED**

The worktree was clean and synchronized at handoff. No package version,
release, tag, file chooser, multi-selection, scrolling, or v0.6 work occurred.

## 2. Commits

- Production and focused tests:
  `5fd42744eb33f55d1ffc70bf05791bc708e96b09`
  (`feat: add verified hierarchy and page transitions`)
- Evidence/docs: `docs: validate v0.5 hierarchy and page continuation`

## 3. Architecture implemented

- Hierarchy capability: two desired-state intents, `Expand` and `Collapse`,
  qualified only from current Expandable state plus an explicit compatible
  public action.
- Page capability: `SwitchPage` targets an exact current `PageTab`; its backend
  strategy is either current `PageTabList` Selection or an exact compatible
  PageTab action.
- Existing pipeline reused: `SemanticCapability`/public state and actions,
  `SceneBinding`, `UiIntent`, `SemanticOperation`, `BackendOperation`, exact
  operation authority, command palette, and authoritative backend readback.
- v0.4 continuation reused: events wake fresh reads; the normal cache, scope,
  public structure, scene, command hierarchy, and focus are rebuilt.
- 0.5A Selection reused: GTK PageTabList selection resolves the target locator
  to a fresh temporary child position and verifies the exact selected object.
- New Tree subsystem: **NO**.
- New Page subsystem: **NO**.
- New Workflow subsystem: **NO**.

## 4. Hierarchy semantic contract

- Target: exact current semantic locator, session, generation, scope, role,
  state, and binding.
- Eligibility: current Enabled/Sensitive and Showing/Visible target with
  Expandable state and an exact compatible expansion action.
- Expandable: necessary but insufficient.
- Expanded: supplies current state and the desired postcondition.
- Public action: exact `expand`, `collapse`, or explicitly named
  `toggle-expand` variants; action index is never guessed.
- Desired state: Expand requires `Expanded=true`; Collapse requires
  `Expanded=false`.
- Invocation: only after fresh exact target/action revalidation.
- Authoritative readback: fresh exact target `Expanded` state.
- Confirmation: backend acceptance or an event cannot confirm; the desired
  state must be freshly true.
- Failure/refusal: changed locator, scope, operation metadata, target
  disappearance, ambiguous action, timeout, or unverifiable state refuses.

**Generic Toggle is not Expand.**

## 5. Expand action qualification

- Observed action metadata: GTK Demo disclosures advertised
  `listitem.expand`, `listitem.collapse`, and `listitem.toggle-expand`.
- What qualifies: exact full semantic action names with current Expandable and
  current desired-state evidence.
- What does not qualify: generic `Toggle`, substring matching, role alone,
  Expandable alone, action order/index, application/toolkit identity, or an
  action probed to discover its effect.
- Action-name rule: case-insensitive exact equality against the bounded
  semantic allow-list; no `contains("expand")` logic.
- Toolkit dependency: none in production.
- Ambiguous actions: remain their existing independent generic semantics or
  read-only; no Expand/Collapse label is manufactured.

## 6. GTK hierarchy evidence

- Fixture: installed GTK4 Demo, `Constraints` disclosure.
- Before: exact target was Expanded and its realized rows were current.
- Target: the exact current `Constraints` Button locator.
- Affordance: normal command palette showed `Collapse Constraints` or
  `Expand Constraints` according to fresh current state.
- Operation: exact public `listitem.collapse` / `listitem.expand`.
- Fresh Expanded: false after Collapse and true after Expand.
- Realization: `Simple Constraints` disappeared and later reappeared with a
  fresh locator.
- Current scene: ordinary v0.4 refresh removed and restored current content.
- Manual continuation: the user remained in the ordinary scene; no descendant
  was automatically selected or activated.
- Result: **PASS**.

## 7. Collapse evidence

- Before: `Constraints` was current and Expanded.
- Operation: normal TUI `Collapse Constraints` command.
- Fresh Expanded: false, proven on the exact target.
- Removed/hidden content: realized sibling rows disappeared.
- Old binding: the old `Simple Constraints` locator became unavailable.
- Fresh scene: current command/scene reconstruction exposed only current
  content and offered `Expand Constraints`.
- Result: **PASS**.

## 8. Already-desired hierarchy state

- Expand on Expanded: a frozen pre-transition Expand command was attempted
  after the exact target had already become Expanded; current-command
  validation refused it and did not toggle the target closed.
- Collapse on Collapsed: the equivalent frozen Collapse command was refused
  after the target had already become collapsed.
- Backend mutation: none from the stale already-satisfied command.
- State preserved: yes in both directions.
- Result: **PASS**.

The operation preflight also recognizes a freshly proven already-desired state
and returns without invoking a non-idempotent toggle-expansion action.

## 9. Ambiguous Qt Toggle

- Fixture: controlled Qt6 tree evidence from Discovery plus the current Qt6
  negative fixture.
- Expandable: Discovery observed an Expandable tree cell.
- Action: only generic `Toggle`; the current negative fixture likewise exposes
  a tree cell Toggle without explicit expansion semantics.
- Observed behavior: the Discovery Toggle changed selection/recreated a row
  but did not establish `Expanded` or reveal a child.
- Generic Expand exposed: **NO**.
- Safe behavior: tree semantics remain readable; existing independently safe
  actions are not relabelled as hierarchy reveal.
- Result: **PASS**.

The focused regression test additionally constructs Expandable + generic
Toggle and proves that it receives no Expand capability.

## 10. Stale hierarchy target

- Old locator: exact current disclosure or realized row locator captured in a
  command/binding.
- Replacement/removal: collapse removed the old realized sibling; a frozen
  desired-state command also ceased matching after external exact state change.
- Old command: current command validation refused it.
- GUI mutation: none from the stale command; already-desired state did not
  invert.
- Fresh target: after re-expansion, the newly realized row received a distinct
  locator and fresh ordinary binding.
- Result: **PASS**.

## 11. Realization / public hierarchy

- Sibling case: GTK `Simple Constraints` and `Interactive Constraints` remain
  siblings under the public List structure.
- True descendant case: the prior v0.4C controlled Qt fixture continues to use
  fresh public parent/child structure.
- Ownership inference: none from the triggering action, names, event order,
  geometry, or visual indentation.
- v0.4 reuse: exact structure refresh, relation/focus authority, stale binding
  removal, and manual continuation.
- New hierarchy system: **NO**.

## 12. Page semantic contract

- Target: exact current `PageTab` locator and binding.
- PageTab structure: exact current direct membership in a current
  single-selection `PageTabList`.
- Eligibility: current generation/scope, enabled/available state, and either
  parent Selection or an exact compatible PageTab action.
- Backend operation: parent Selection for GTK; exact PageTab action for Qt.
- Temporary index if any: derived fresh only for immediate parent Selection.
- Authoritative readback: exact selected child for Selection; otherwise a
  constrained public current-page proof on the same exact locator.
- Current-page proof: public Selected state when exposed, or for the observed
  Qt action shape the exact target must be focused, its own nonempty label must
  equal the PageTabList current-page label, and that label must be unique among
  fresh direct PageTab children. Focus or a name alone cannot confirm, and
  names never locate or rebind a target.
- Content refresh: ordinary v0.4 rebuild determines current page content.
- Failure/refusal: changed membership/locator/action/scope, multi-select,
  duplicate/empty ambiguous page evidence, or failed readback cannot confirm.

**Tab index is not semantic identity.**

## 13. GTK PageTab evidence

- Fixture: controlled GTK4 Notebook.
- Before page: General selected; General-only action current.
- Target: exact current Advanced PageTab.
- Fresh position: resolved from the current TabList immediately before public
  Selection; no position is retained in the semantic operation.
- Operation: public parent Selection.
- Fresh selected/current PageTab: selected child locator exactly equalled
  Advanced, and Advanced exposed Selected.
- Content: Advanced-only checkbox became current; General-only Button ceased
  to be current/actionable.
- Manual continuation: the fresh current Advanced control remained usable.
- Result: **PASS**.

## 14. Qt PageTab evidence

- Fixture: controlled Qt6 `QTabWidget`.
- Before: General was the current page.
- Target: exact current Advanced PageTab locator.
- Public action: exact `Press` advertised by that PageTab.
- Backend result: accepted, but acceptance was not treated as success.
- Fresh target-specific page state: the exact target satisfied the constrained
  public composite described above; Advanced-only content became current.
- Content: General-only action lost Showing/Visible; Advanced-only setting
  gained it.
- Manual continuation: the ordinary refreshed scene exposed current controls.
- Toolkit branch: **NO**.
- Result: **PASS**.

## 15. Cross-implementation page contract

- GTK mechanism: exact current PageTabList Selection and exact selected-object
  readback.
- Qt mechanism: exact current PageTab public action and constrained fresh
  target-specific current-page readback.
- Shared semantic contract: exact current target + compatible public operation
  + fresh target-specific page truth + ordinary v0.4 continuation.
- What was not unified: backend Selection and Action mechanisms, nor their
  distinct readback evidence, were forced into a single backend operation.

## 16. Already-current page

- Target: exact Advanced PageTab after the first confirmed GTK switch.
- Initial current state: parent Selection returned exact Advanced.
- User request: `Switch to Advanced` again.
- Backend mutation: skipped because fresh exact current state already
  satisfied the intent.
- Final state: Advanced remained selected/current.
- Result: **PASS**.

## 17. Old-page authority

- Old page: GTK General.
- Captured control: frozen `General page action` palette entry.
- Switch: a concurrent normal GUI2TUI session switched to Advanced through the
  qualified current PageTab command.
- Current page: Advanced, proven by exact selected PageTab and current content.
- Old command result: concise current-interface refusal.
- GUI mutation: none; the General result action did not execute.
- Fresh current controls: Advanced setting remained available and General
  action was absent from the reconstructed command hierarchy.
- Result: **PASS**.

## 18. Tab reorder/replacement

- Case: controlled GTK fixture removed and recreated Advanced with the same
  descriptive label.
- Old target: pre-replacement exact Advanced locator.
- Old index: not retained by the command or semantic operation.
- New structure: fresh TabList contained a different Advanced locator.
- Old authority: frozen command refused; label equality transferred no
  authority.
- Fresh target: fresh `Switch to Advanced` resolved the replacement's current
  membership and succeeded.
- Wrong page switch: none through old authority.
- Result: **PASS**.

## 19. Focus / visibility

- Focus: may contribute to the constrained Qt action readback but never
  confirms current page by itself.
- Selected/current: exact target Selection/Selected state is the strongest
  current-page proof when exposed.
- Showing: determines current page-control availability and stale old-page
  command removal; it does not alone prove which tab caused the page.
- Scope: both PageTabList and target must remain in current interaction scope.
- Which establishes page truth: exact target-specific public postcondition.
- Which does not: backend acceptance, event, elapsed time, focus alone, name
  alone, visibility alone, RuntimeNodeId, or a tab position.

## 20. TUI UX

- Hierarchy affordance: contextual `Expand` or `Collapse` only for a currently
  qualified disclosure.
- Page affordance: contextual `Switch page`; command labels use `Switch to …`.
- Help/footer: existing focused-control presentation advertises only the
  current qualified intent.
- Command palette: retains exact locator, scope, and intent and validates them
  against the fresh hierarchy before execution.
- Raw action name leaked: no in normal UI.
- Index leaked: no.
- Stale wording: task-oriented current-interface refusal.
- False affordance: none for generic Toggle, hidden old-page content, stale
  tabs, or unsupported hierarchy targets.

## 21. Existing capability regression

- Selection: PASS; exact current List target tests remain green.
- Table row: PASS; exact current Table-row tests remain green.
- Text: PASS.
- Toggle: PASS; generic Toggle behavior remains distinct.
- Choice: PASS.
- Value: PASS.
- Complex text: PASS.
- Button/action: PASS.
- Reader: PASS.
- Password: PASS.
- Spatial/navigation: PASS; no key or RegionNavigator redesign.

## 22. Genericity audit

- App branches: none added.
- Toolkit branches: none added.
- Generic Toggle guessing: absent.
- Action index: never selected as semantic fallback.
- Name identity: absent; exact locators remain identity, and duplicate Qt tab
  labels make the constrained readback ambiguous rather than rebinding.
- Geometry: presentation only.
- Input injection: absent.
- Fixed-time success: absent; deadlines only terminate observation.
- Fuzzy identity: absent.
- Result: **PASS**.

## 23. Tests / quality

- New focused tests: 2 — explicit expansion semantics reject generic Toggle;
  PageTab switching carries exact locators and never a durable tab index.
- Targeted: both new tests PASS; all-target check PASS.
- Linux live: PASS on Ubuntu 24.04 arm64 under Xvfb/AT-SPI using
  `tests/live/v05b_run_linux.sh`; current v0.4C realization workflow also PASS.
- macOS: PASS for source quality on the supported host.
- fmt: PASS.
- check: PASS.
- test: PASS — 289 library, 2 inspector, and 4 CLI integration tests; 0
  failures.
- clippy: PASS with warnings denied.
- docs: PASS.
- diff: PASS.

## 24. Required result summary

```text
HIERARCHY_EXPAND_CONFIRMED=PASS
HIERARCHY_COLLAPSE_CONFIRMED=PASS
HIERARCHY_MANUAL_CONTINUATION=PASS
AMBIGUOUS_TOGGLE_NOT_EXPAND=PASS
EXPANSION_ALREADY_DESIRED_STATE_SAFE=PASS
EXPANSION_STALE_TARGET_AUTHORITY_REFUSAL=PASS
PAGETAB_GTK_SWITCH=PASS
PAGETAB_QT_SWITCH=PASS
PAGETAB_TARGET_SPECIFIC_READBACK=PASS
PAGETAB_ALREADY_CURRENT_SAFE=PASS
OLD_PAGE_COMMAND_AUTHORITY_REFUSAL=PASS
PAGETAB_STALE_TARGET_REFUSAL=PASS
NO_INFERRED_REALIZATION_OWNERSHIP=PASS
NO_TOOLKIT_BRANCH=PASS
```

## 25. Remaining issues

- P0: 0.
- P1: 0.
- P2: 0 within 0.5B.

File/folder choice composition and integrated milestone qualification belong
to 0.5C and 0.5D; they are not 0.5B defects.

## 26. Architecture conclusion

- Did existing capability/operation/v0.4 architecture remain sufficient?
  **YES**.
- Did hierarchy reveal require TreeNavigator/ownership system? **NO**.
- Did PageTab require a Page subsystem? **NO**.

Stable rules: hierarchy reveal requires an exact unambiguous current public
operation and fresh desired Expanded state; generic Toggle is not Expand.
Page switching targets an exact current PageTab rather than an index and
requires fresh target-specific page truth. Both return to ordinary v0.4 scene
reconstruction and manual continuation.

## 27. v0.5 roadmap

- Discovery: **COMPLETE**.
- 0.5A: **COMPLETE / VALIDATED**.
- 0.5B: **COMPLETE / VALIDATED**.
- 0.5C: **PLANNED / NOT AUTHORIZED**.
- 0.5D: **PLANNED / NOT AUTHORIZED**.

No later phase is automatically authorized.

## 28. Recommended next direction

**A. Recommend 0.5C — File and Folder Choice Tasks.**

Do not start it without explicit user authorization.

## 29. Git status

- Branch: `v0.5/task-interaction-completeness`.
- Production source: `5fd42744eb33f55d1ffc70bf05791bc708e96b09`.
- Evidence HEAD: the subsequent documentation-only handoff commit.
- Worktree: clean at handoff.
- Remote: synchronized with origin at handoff.
- Public tags: unchanged; `v0.1.0`, `v0.2.0`, and `v0.3.0` immutable.
- v0.5.0 tag/release: absent and not planned.

## 30. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone HANDOFF、v0.5 Discovery、
v0.5 roadmap、0.5A HANDOFF 与本 0.5B HANDOFF。当前分支为
`v0.5/task-interaction-completeness`，精确 HEAD 以本次文档提交及最终交接
为准。v0.4 已 COMPLETE / MILESTONE QUALIFIED；0.5A 与 0.5B 均已
VALIDATED。Selection 的目标是语义对象而非索引；generic Toggle 不是
Expand；层级成功必须由新鲜的目标 `Expanded` 期望状态证明，实现不产生
所有权。PageTab 的目标是精确当前语义 tab 而非 tab index，成功需要新鲜
的目标特定当前页读回。页面/层级动态内容继续复用 v0.4，隐藏或陈旧权限不
迁移。下一项精确建议为 0.5C File and Folder Choice Tasks，但未自动授权。
v0.5 是内部 milestone，无 RC/tag/release；v0.6/v0.7 仍是后续 milestone，
v1.0.0 仍是下一计划公开版本。不得自行扩展。
