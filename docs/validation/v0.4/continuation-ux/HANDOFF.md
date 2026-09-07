# GUI2TUI v0.4 Phase 0.4D — Continuation UX and Theme Qualification

## 1. Status

- Phase: 0.4D
- Starting HEAD: `66f87f34f4c578f54ed8d2daed6b4fdd33bbba40`
- Production commit: `d7e062a` (`fix: clarify semantic continuation status`)
- Final HEAD: the documentation/evidence commit containing this handoff
- Branch: `v0.4/semantic-workflow-reconstruction`
- Worktree at handoff: clean
- Production changes: concise task-oriented transition and stale-command
  status wording only
- New interaction families: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.4D CONTINUATION UX AND THEME QUALIFICATION VALIDATED**

## 2. Commits

1. `d7e062a` — `fix: clarify semantic continuation status` (minimal UX change
   and one focused status-language test)
2. Validation probes, evidence, project-guide refinement, and roadmap
   qualification are committed with this handoff.

No workflow, menu, dialog, hierarchy, focus, navigation, or interaction-family
production commit was created.

## 3. Integrated v0.4 user model

The user performs a semantic action. GUI2TUI observes only an explicit,
bounded semantic condition and rebuilds from fresh GUI semantics. The normal
scene and current-scope command palette then contain what is currently safe.
The user chooses the next fresh binding manually.

The user does not need to understand transition conditions, event wakeups,
`RuntimeNodeId`, `BackendLocator`, `ApplicationGenerationId`, or
`InteractionScope`. Those details remain in product logs, Inspector output,
and validation evidence.

## 4. UX changes

Previous behavior: normal transition statuses could say “via ShowMenu;
authoritative transition confirmed”, mention an “observation deadline”, call a
result “semantically ambiguous”, or refer to a “semantic runtime/current
binding”.

Problem: these messages exposed implementation taxonomy and invited the user
to reason about GUI2TUI internals rather than the current task. Timeout wording
also overemphasized its implementation bound.

New behavior: success, when it remains visible, is a short task action such as
`Opened menu "Tools"`; unconfirmed outcomes say the action was not confirmed
and that the current interface is shown; stale commands tell the user to
choose from the current interface. Ambiguous, cancelled, and unverifiable
outcomes use similarly bounded user language.

Why required: the live stale and timeout workflows were truthful but their
normal wording was unnecessarily diagnostic. Existing footer/status plumbing
was sufficient; no new UI was added.

## 5. Menu continuation UX

- Before: `Activate Demo` is absent from current commands while the Qt Menu is
  not Showing.
- Action: the user chooses the existing `Tools` ShowMenu action.
- After reveal: the Menu and item are freshly Showing inside the current
  Window; the current-scope palette lists the exact `Activate Demo` item.
- Available action: only the current fresh item binding is selectable.
- User continuation: the user explicitly selects and activates that item.
- After close: authoritative status changes, the Menu becomes non-Showing,
  and the item disappears from current commands.
- Internal lifecycle exposed: no workflow, owner, scope, locator, or
  transition taxonomy is shown.
- Result: **PASS**.

## 6. Modal continuation UX

- Before: the Qt main Window exposes its ordinary controls.
- Enter: the user chooses `Open modal dialog`; a fresh active modal is
  confirmed.
- Current context: the ordinary scene shows `Qt Fixture Dialog`, dialog
  content, and `Close`.
- Background: `Activate safely` is absent from current commands, and a frozen
  background command is refused.
- User action: the user chooses the current `Close` action.
- Exit: the modal disappears and its bindings become stale.
- Return: the Window scene and its controls return; the user can invoke a
  current checkbox action.
- Result: **PASS**.

## 7. Cross-implementation modal UX

- Qt: an active `QDialog` becomes the confined current scene and returns to
  its Window after `Close`.
- GTK: a GTK4 modal Window is created, presents `Dialog content` and `Close
  dialog`, is destroyed on close, and returns to the main Window.
- Toolkit-specific UX difference: none. Creation/destruction and event shapes
  stay below the normal scene.
- Result: **PASS**.

## 8. Realization UX

- Before: GTK Demo exposes the expanded `Constraints` row and related current
  rows.
- Semantic change: exact validation-only collapse removes the rows; exact
  expand re-realizes them with fresh locators.
- Fresh scene: after the explicit semantic refresh, the current Choice model
  contains the re-realized rows.
- Hierarchy presentation: the rows remain siblings under the public List;
  they are not grouped under the disclosure trigger.
- Manual navigation: ordinary Tab navigation reaches the current Choice and
  the user opens it to reach `Simple Constraints`.
- Stale entries: the pre-collapse locator is unavailable and cannot act.
- Result: **PASS**.

## 9. Public hierarchy UX

- True descendant: the Qt fixture realizes `Realized descendant toggle`
  beneath the public `Descendant container`; a fresh current palette binding
  performs the existing Toggle action.
- Ambiguous sibling: GTK `Constraints` and its related rows occupy sibling
  `ListItem` parents with no exclusive ControllerFor/ControlledBy relation.
- How presentation differs: only the fresh public parent/child structure can
  produce hierarchy; the ambiguous GTK case remains the existing flat Choice.
- False hierarchy: none produced.
- Result: **PASS**.

## 10. Stale/refusal UX

- Case: `Activate Demo` is frozen in an open palette, then the Menu closes and
  the item acts through another exact validation client.
- User-visible result: `Command is no longer available; choose from the
  current interface`.
- GUI mutation: the stale submission causes no second activation.
- Scene recovery: a fresh Menu reveal creates a new current command that the
  user can select successfully.
- Internal detail leaked: none.
- Result: **PASS**.

## 11. Unconfirmed/timeout UX

- Condition: the ordinary `Activate safely` action cannot satisfy the
  expected new-active-modal condition before the bound.
- User-visible wording: `Action for "Activate safely" was not confirmed;
  current interface is shown`.
- Success claimed: no. The message does not claim that application-level
  mutation failed either.
- Runtime usable: yes; the fresh application status is visible and the user
  immediately performs a current checkbox action.
- Result: **PASS**.

## 12. Ambiguity UX

- Case: the Qt Menu has no public popup-owner relation, while the GTK
  disclosure-related rows have no exclusive trigger ownership.
- Unsafe affordance exposed: none. The contained Menu stays in its proven
  Window scope, and GTK rows remain siblings.
- What remains usable: the exact current Menu item and fresh Choice rows.
- Normal wording: no ownerless-popup or realization-ownership terminology.
- Inspector detail: containment, relations, locators, and scopes remain
  available there.
- Result: **PASS**.

## 13. Help/footer/navigation

- Contextual help: continues to derive from the focused current binding.
- Stale help: hidden Menu items, modal background commands, and disappeared
  realized nodes are absent after refresh.
- Tab: unchanged and used to reach fresh Choice/current controls.
- F6: unchanged.
- Ctrl+Tab: unchanged.
- RegionNavigator: unchanged and rebuilt only from the current scene.
- Second help/navigation system: **NO**.

## 14. False-affordance audit

- Hidden Menu: absent before reveal and after close.
- Modal background: absent from current commands and frozen entry refused.
- Stale realized binding: refused; replacement requires a fresh binding.
- Detached ownerless surface: receives no authority from focus/visibility.
- Unsupported Expand: no production affordance added.
- Selection: no breadth added.
- Result: **PASS**.

## 15. Structural-honesty audit

- Fresh parent/child: may establish current hierarchy.
- Relations: exact fresh public relations may contribute semantic structure.
- Ambiguous siblings: remain siblings.
- Geometry: presentation evidence only; never ownership.
- Names: descriptive only; never identity or ownership.
- Historical structure: revalidated by exact current locators before use.
- Result: **PASS**.

## 16. 0.4A reuse

- Events: wake or invalidate only.
- Fresh reads: establish transition truth.
- Deadlines: terminate observation and never create success.
- Exact locator: operation and observation authority remains exact.
- Result: **PASS**.

## 17. 0.4B reuse

- Visibility: qualifies availability but is not authority alone.
- Focus: usability evidence only.
- Scope: current exact `InteractionScope` confines operations.
- Ownerless popup: no owner or new scope is invented.
- Result: **PASS**.

## 18. 0.4C reuse

- Realization: changes current availability, not ownership.
- Replacement: old locator authority ends.
- Fresh structure: alone establishes current hierarchy.
- Fresh binding: supplies the next user operation authority.
- Historical relation: never migrates through presentation identity.
- Focus: exact stale locator falls back safely rather than following a
  retained runtime ID.
- Result: **PASS**.

## 19. Existing feature regression

- Single-line: existing EditSession tests pass; not refactored.
- Value: existing authoritative Value tests pass; not refactored.
- Complex text: existing ExternalTextSession tests pass; not refactored.
- Conflict: existing stale/conflict refusal tests pass.
- Password: exclusion/redaction tests pass and live fixture stays redacted.
- Choice: direct semantic Choice remains unchanged and supplies GTK
  realization continuation.
- Reader: Reader/content tests pass and GTK live scene remains usable.
- External Modality: tests pass; no lifecycle merge occurred.
- Spatial: all layout/navigation tests pass; no presentation redesign.

## 20. Required result summary

```text
CONTINUATION_UX_MENU=PASS
CONTINUATION_UX_MODAL=PASS
CONTINUATION_UX_MODAL_CROSS_IMPLEMENTATION=PASS
CONTINUATION_UX_REALIZATION=PASS
CONTINUATION_UX_PUBLIC_HIERARCHY=PASS
CONTINUATION_UX_STALE_REFUSAL=PASS
CONTINUATION_UX_AMBIGUITY=PASS
CONTINUATION_UX_UNCONFIRMED=PASS
CURRENT_SCENE_MANUAL_CONTINUATION=PASS
NO_WORKFLOW_UI=PASS
NO_FALSE_HIERARCHY=PASS
NO_FALSE_AFFORDANCE=PASS
```

## 21. Tests and quality

- New focused tests: one status-language contract test covering confirmed,
  timeout, stale, and frozen-command wording without internal taxonomy.
- Live workflows: `tests/live/v04d_run_linux.sh` composes the existing 0.4B
  and 0.4C probes with `v04d_continuation_ux.py`.
- Screenshots/evidence: no image campaign; seven privacy-safe rendered text
  frames were reviewed in `/tmp/gui2tui-v04d-frames.txt` and were not committed
  as brittle goldens.
- macOS: full Rust source-quality matrix passed.
- Linux: Ubuntu 24.04 arm64, Xvfb/X11, session D-Bus, AT-SPI, Qt6 and GTK4
  integrated live pass succeeded.
- fmt: PASS.
- check: PASS.
- test: PASS.
- clippy: PASS with warnings denied.
- docs: PASS.
- diff: PASS.

## 22. P0 / P1 / P2

- P0: 0
- P1: 0
- P2: 0

Planned v0.5 Selection, broad Expand, tree/menu breadth, and file-chooser task
coverage are not v0.4 defects.

## 23. v0.4 theme qualification

Can verified operations now be followed through semantic GUI state changes
into a truthful current TUI scene from which the user can safely continue?
**YES**.

Does v0.4 require workflow automation? **NO**.

Does v0.4 require a `WorkflowEngine`? **NO**.

## 24. v0.4 exit definition

1. Bounded authoritative transition after a verified operation: **PASS**
   through 0.4A.
2. Dynamic surface authority without invented ownership: **PASS** through
   0.4B and integrated Menu/Modal UX.
3. Realization without stale identity transfer or invented hierarchy:
   **PASS** through 0.4C and integrated realization UX.
4. Ordinary refreshed `TuiScene` presents current state: **PASS**.
5. User continues with fresh current bindings: **PASS**.
6. Ambiguous, stale, and unconfirmed cases refuse safely: **PASS**.
7. State, dynamic surface, and realization/hierarchy share one continuation
   model: **PASS**.
8. No workflow automation engine required: **PASS**.

**v0.4 FUNCTIONAL DEVELOPMENT COMPLETE**.

## 25. Architecture conclusion

```text
user semantic action
  -> bounded authoritative transition
  -> current scope and public structure rebuild
  -> ordinary TuiScene / current command palette
  -> fresh current binding
  -> user manual continuation
```

Events wake observation; fresh GUI semantics decide truth. Visibility and
focus are evidence, not authority. Runtime presentation continuity does not
transfer locator authority. Realization changes availability without creating
ownership. Failures remain truthful and return to a usable current scene.

## 26. v0.4 roadmap

- Discovery: complete — conclusion B, narrower continuation model sufficient
- 0.4A: complete / validated
- 0.4B: complete / validated
- 0.4C: complete / validated
- 0.4D: complete / validated
- Functional development: **COMPLETE**

No RC, release, or later milestone work was started.

## 27. Roadmap-to-1.0 boundary

- v0.5: Task & Interaction Completeness — unchanged
- v0.6: Runtime Continuity — unchanged
- v0.7: Deployment — unchanged
- No automatic v0.8/v0.9.

## 28. Recommended next direction

**A. Recommend v0.4.0 RC qualification, awaiting explicit user
authorization.** Do not start RC, tag, release, or v0.5 work automatically.

## 29. Git status

- Branch: `v0.4/semantic-workflow-reconstruction`
- HEAD: the documentation/evidence commit containing this handoff, based on
  production commit `d7e062a`
- Worktree: clean at handoff
- Remote: branch pushed normally through the production commit; the evidence
  commit is pushed normally at phase close
- v0.3.0 immutable tag: `efc704adf8a3ded3463ed8bb81670eddd08296c3`

## 30. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 Discovery、v0.4 roadmap、0.4A、
0.4B、0.4C HANDOFF 和最新 0.4D HANDOFF。当前精确分支为
`v0.4/semantic-workflow-reconstruction`，HEAD 为包含本 HANDOFF 的提交；
0.4A/0.4B/0.4C/0.4D 均已验证，v0.4 functional development 已完成。事件只
唤醒，fresh authoritative semantics 才决定事实；`RuntimeNodeId` 仅可提供
展示连续性，不授予 replacement authority，旧 locator authority 永不迁移。
可见性/focus 不授予交互 authority；fresh scope 决定用户可在哪里交互。
realization 不产生 ownership，只有 fresh public structure 定义当前
hierarchy；fresh current binding 是用户下一次操作的 authority，continuation
始终由用户手动控制。精确建议下一步是 v0.4.0 RC qualification，但 RC 未被
本阶段自动授权；v0.5/v0.6/v0.7 仍是后续路线层。不要自行扩展范围。
