# GUI2TUI v0.5 Phase 0.5A — Verified Current Collection Selection

## 1. Status

- Phase: 0.5A — Verified Current Collection Selection
- Starting HEAD: `f54b0545e9abc3517f5c1c41ccfcc9af70656c3d`
- Production commit: `4dee79f752f6ec65ea754fc71fe75b006a1e56d4`
- Branch: `v0.5/task-interaction-completeness`
- Production changes: bounded exact-current List-item and Table-row selection,
  target-specific readback, and a contextual normal-TUI affordance
- New interaction families: single current collection selection and bounded
  current Table-row selection
- P0: 0
- P1: 0
- Overall: **PHASE 0.5A VERIFIED CURRENT COLLECTION SELECTION VALIDATED**

Final production and evidence commit identities are recorded in the Git
history and final phase handoff. The worktree was clean at handoff.

## 2. Commits

- Production and focused tests: `4dee79f752f6ec65ea754fc71fe75b006a1e56d4`
  (`feat: add verified current collection selection`)
- Evidence/docs: `docs: validate v0.5 current selection`

The production commit contains no tab, Expand/Collapse, file chooser,
multi-selection, scrolling, workflow, package-version, or release work.

## 3. Architecture implemented

The existing operation pipeline remained sufficient. Two narrowly qualified
semantic capabilities distinguish an exact current direct child from an exact
current Table row. The user intent remains `Select`; `SemanticOperation`
targets the chosen runtime node, while `BackendOperation` carries exact
collection/Table and target locators—never a durable index. The backend
freshly resolves the locator to a child index or Table row immediately before
the public operation and freshly resolves the selected object/row afterward.

- `SemanticCapability`: adds `SelectCurrentChild` and
  `SelectCurrentTableRow`; the older interface-level `SelectChildren` remains
  descriptive and cannot alone create the new normal-TUI affordance.
- `UiIntent`: reuses `Select`.
- `SemanticOperation`: reuses `SelectNode`.
- `BackendOperation`: adds exact-locator `SelectCurrentItem` and
  `SelectCurrentTableRow` forms.
- Operation ticket: reuses `RuntimeSession`, application generation,
  cancellation, and transition-observation tickets.
- v0.4 transition: events only wake re-evaluation; fresh reads confirm.
- `SceneBinding`: carries the current locator, role, capability, actions, and
  Select intent. Frozen bindings are revalidated against current structure.
- New Collection subsystem: **NO**.

## 4. Selection semantic contract

- Semantic target: the exact currently accessible semantic object chosen by
  the user.
- Collection authority: current session/generation, exact collection locator,
  current scope, current public interface/state, and single-selection
  eligibility.
- Target authority: exact current locator, role, availability, scope, and
  direct membership or exact public TableCell mapping.
- Backend addressing: a freshly derived child index or Table row number used
  only for the immediate backend call.
- Invocation: exact advertised target action or public Selection/Table method.
- Authoritative readback: exact selected child locator, or selected row plus a
  fresh TableCell-to-row mapping.
- Confirmation: only the exact intended current target is proven selected.
- Failure/refusal: stale membership, replacement, multiple selection,
  unavailable state, null/different selected object, or unverifiable mapping
  cannot produce `Confirmed`.

**Index is not semantic identity.**

## 5. Parent Selection eligibility

- Interface: public AT-SPI Selection on the exact current parent.
- Single-selection requirement: parent and target must not advertise
  Multiselectable.
- Target membership: the exact locator must occur exactly once in a fresh
  direct-child list.
- Visibility/actionability: target must be Enabled/Sensitive and
  Showing/Visible; defunct/read-only or unavailable structures refuse.
- Scope: both parent and target must remain inside current
  `InteractionScope`.
- Readback: exactly one selected child must resolve to the target locator.
- Multiselect behavior: the new Select affordance is absent; the collection
  remains readable and any independently safe existing behavior is unchanged.

Existing Choice-only option selection remains supported through its bounded
Choice path, without widening normal List-item eligibility.

## 6. GTK single-selection evidence

- Fixture: `tests/fixtures/v05a_gtk_selection_fixture.py`, GTK4 `ListView`
  backed by `SingleSelection`.
- Collection: `Current single-selection items`.
- Before: Alpha selected; Beta and Gamma current and accessible.
- Target: exact current Beta object.
- Fresh index: 1 in the initial structure.
- Operation: parent Selection `SelectChild(1)`, where 1 was derived from the
  fresh exact Beta membership immediately before invocation.
- Backend result: accepted.
- Fresh selected target: Selection `GetSelectedChild(0)` resolved exactly to
  Beta; Beta also advertised Selected.
- After: the normal refreshed scene showed Beta selected.
- Normal TUI affordance: Tab reached Beta and Enter performed Select.
- Result: **PASS**.

## 7. Reorder safety

- Target: Beta from a captured pre-reorder scene.
- Old index: 1.
- Mutation/reorder: current order became Beta, Alpha, Gamma; Beta moved to
  index 0 and GTK recreated its locator.
- Old binding behavior: the old Beta locator returned
  `SelectionTargetNotCurrent`; no invocation used index 1.
- Fresh revalidation: the freshly chosen current Beta resolved to index 0.
- Selected result: exact fresh Beta confirmed selected.
- Wrong-target mutation: none; Alpha was never selected through the stale
  integer.
- Result: **PASS**.

This live toolkit recreated the row locator during reorder, so the accepted
safe outcome was refusal of the old binding followed by successful use of a
fresh binding.

## 8. Replacement safety

- Old locator: L1 for the original Beta row.
- Replacement locator: L2 for a newly created row with the same name and
  position.
- RuntimeNodeId: cache reconciliation may retain presentation continuity, but
  the live GTK run did not require or infer identity reuse.
- Old binding: refused because L1 was not a current direct member.
- GUI mutation: none; Gamma remained selected after the old attempt.
- Fresh binding: L2 was selected normally and exact L2 readback confirmed it.
- Result: **PASS**.

Name, position, index, geometry, and retained presentation identity transferred
no authority.

## 9. Virtualization refusal

- Logical collection size: 200.
- Realized accessible children: six (`Virtual 000` through `Virtual 005`) in
  the bounded live scene.
- Unsafe requested index: 100, used only through the validation inspector to
  reproduce backend behavior.
- Backend return: the low-level Selection method reported acceptance.
- Authoritative readback: no exact current selected object corresponding to
  logical item 100; current realized children remained unselected.
- Production exposure: only the six exact realized objects received current
  bindings. There is no logical-index input or synthesized target.
- Result: **PASS**.

Backend acceptance without an exact selected object is never success.

## 10. Already-selected behavior

- Target: current Beta after the first confirmed selection.
- Initial state: exact Beta was authoritatively selected.
- User Select: the user navigated to the fresh current Beta binding and chose
  Select again.
- Backend operation: detected exact current Selected state and skipped the
  potentially non-idempotent action/mutation.
- Final state: Beta remained the exact selected target.
- Accidental deselect: none.
- Result: **PASS**.

## 11. Qt cross-implementation evidence

- Fixture: existing controlled Qt6 single-select list fixture.
- Public operation shape: exact current child advertised the named `Toggle`
  action; the operation was qualified as the user task Select, not exposed as
  arbitrary toggle-membership semantics.
- Selection readback: fresh exact Beta state reported Selected after the named
  action.
- Task result: exact current Beta became selected.
- New toolkit-specific branch: **NO**.
- Result: **PASS**.

GTK and Qt are evidence carriers. They deliberately use different public
mechanisms beneath the same exact-target/readback contract.

## 12. Table-row semantic contract

- Table authority: exact current Table locator, public Table interface,
  single-selection state, availability, generation, and scope.
- Semantic row target: an exact current realized TableCell chosen in the
  ordinary Table view.
- Fresh mapping: public `TableCell.Table`, `TableCell.Position`, and
  `Table.GetAccessibleAt(row, column)` must all resolve back to the exact cell.
- Temporary row index: derived only after that fresh mapping.
- Backend operation: public `Table.AddRowSelection(row)`; flattened parent
  Selection is not used.
- Selected-row readback: public `Table.GetSelectedRows()`.
- Exact-target verification: the sole selected current row plus a fresh exact
  cell-to-row mapping must correspond to the intended cell.
- Partial realization behavior: only current accessible cells are shown and
  selectable. Partial presentation omits inferred row/column labels; backend
  Table semantics remain operation authority.

## 13. Table-row live evidence

- Fixture: `tests/fixtures/v05a_gtk_table_fixture.py`, GTK3 `TreeView` in
  SINGLE selection mode.
- Table: `Current semantic rows` with Alpha/10, Beta/20, Gamma/30.
- Before: Alpha selected.
- Target row: the row containing exact current Beta cell.
- Current row index: 1, derived from public TableCell position and verified by
  `GetAccessibleAt`.
- Operation: `Table.AddRowSelection(1)`.
- Fresh selected rows: `[1]`.
- Fresh target verification: Beta remained the exact current TableCell at the
  selected row/column mapping.
- After: application label read `Selected row: Beta`; the refreshed Table view
  marked the current selected content.
- Result: **PASS**.

The ordinary TUI entered the current Table through its existing content-view
model, navigated current accessible cells, and used Enter only when the fresh
Table selection capability was qualified.

## 14. Table reorder/replacement

- Case: controlled GTK table row reorder.
- Old row index: 1 for the captured Beta cell.
- New structure: Beta moved to row 0; GTK recreated the cell locator; Gamma
  was selected independently before the stale attempt.
- Old authority: exact old Beta locator returned unavailable and no Table row
  operation was issued from the old integer.
- Fresh target: current Beta mapped to `TableRow(0)` and selected normally.
- Wrong row selected: none; the stale row integer did not select Alpha.
- Result: **PASS**.

## 15. Multi-selection boundary

- GTK: an explicit `MultiSelection` List exposed Multiselectable and public
  Selection. Its Multi Alpha/Beta/Gamma elements remained readable but had no
  Select binding.
- Qt: Discovery showed a multiselect item `Toggle` can add and remove
  membership; that shape is not mapped to semantic Select.
- Why Select != Toggle: Select means make this exact target selected, whereas
  Toggle may deselect an already selected target or add to a selected set.
- What production exposes: bounded single-target selection only when the
  collection contract is single-select.
- What remains deferred: add/remove membership, deselect, clear, select-all,
  ranges, and ordering.
- Result: **PASS**.

## 16. Focus / visibility

- Focus: navigation/presentation state only; never selection truth.
- Selected: public target state contributes, but parent Selection or Table
  readback is cross-checked where available.
- Showing/Visible: required current-availability evidence, not confirmation.
- Enabled/Sensitive: required actionability evidence, not confirmation.
- Scope: determines whether current interaction is permitted.

Only fresh exact selected-object/Table-row semantics establish selection
truth.

## 17. v0.4 reuse

- Transition observer: reused the bounded operation ticket/deadline loop.
- Events: wake and invalidate only.
- Fresh reads: determine selected truth on every check.
- Scope: current `InteractionScope` validates parent/Table and target.
- Realization: normal cache/scene refresh exposes current post-selection
  content; no SelectionContinuation exists.
- Stale bindings: exact old locator/generation authority refuses.
- Current scene: the user continues manually from the ordinary rebuilt scene.

## 18. TUI UX

- User affordance: qualified List items use the existing focused Select
  interaction; a current qualified Table uses its existing content view and
  contextual Enter Select-row action.
- Help/footer: Select appears only for a current qualified target/Table.
- Command palette: exact current bindings carry locators, never raw durable
  indices; stale entries follow existing refusal behavior.
- Stale command wording: “Item is no longer available; choose from the current
  interface.”
- Index leaked to user: **NO**.
- Internal taxonomy leaked: **NO**.

## 19. Existing capability regression

- Text: PASS through existing automated coverage.
- Toggle: PASS; Select did not replace ordinary Toggle.
- Choice: PASS through targeted Choice discovery/resolution tests.
- Value: PASS through existing automated coverage.
- Complex text: PASS through existing conflict/readback coverage.
- Button/action: PASS through existing action-resolution/operation coverage.
- Reader: PASS; Table entry extends the existing content-view path only.
- Password: PASS through the existing exclusion/readback suite.
- Spatial/navigation: PASS; no new global key or navigation subsystem.

## 20. Genericity audit

- App branches: none.
- Toolkit branches: none.
- Name identity: absent.
- Geometry: presentation-only; absent from selection authority.
- Input injection: absent.
- Anonymous action: absent; exact advertised names only.
- Stale index: never carried in `SceneBinding`, `SemanticOperation`, or
  `BackendOperation`.
- Offscreen guessing: absent.
- Fuzzy identity: absent.
- Result: **PASS**.

## 21. Tests / quality

- New focused tests: exact locator/no-index operation resolution;
  multiselect Toggle refusal; null/different exact selected-object mismatch
  refusal. Existing focused Table-view coverage now checks the contextual
  Select-row command.
- Targeted checks: capability, Choice, operation, Table content/view, and
  authority tests PASS.
- Linux live: GTK4 single/reorder/replacement/virtualized/already-selected and
  multiselect-negative cases; Qt6 action equivalent; GTK3 Table selection and
  reorder PASS.
- macOS: final full source matrix PASS.
- fmt: PASS.
- check: PASS.
- test: PASS.
- clippy: PASS with `-D warnings`.
- docs: PASS.
- diff: PASS.

## 22. Required result summary

```text
CURRENT_SINGLE_SELECTION=PASS
SELECTION_TARGET_SPECIFIC_READBACK=PASS
SELECTION_REORDER_WRONG_TARGET_REFUSAL=PASS
SELECTION_REPLACEMENT_AUTHORITY_REFUSAL=PASS
VIRTUALIZED_SELECTION_FALSE_SUCCESS_REFUSAL=PASS
SELECTION_ALREADY_SELECTED_SAFE=PASS
CROSS_IMPLEMENTATION_SINGLE_SELECTION=PASS
CURRENT_TABLE_ROW_SELECTION=PASS
TABLE_ROW_TARGET_SPECIFIC_READBACK=PASS
TABLE_ROW_REORDER_WRONG_TARGET_REFUSAL=PASS
MULTISELECT_NOT_EXPOSED_AS_SINGLE_SELECT=PASS
FOCUS_NOT_USED_AS_SELECTION_TRUTH=PASS
INDEX_NOT_SEMANTIC_IDENTITY=PASS
```

## 23. Remaining issues

- P0: 0.
- P1: 0.
- P2: 0.

The separately planned 0.5B hierarchy/page, 0.5C chooser, and 0.5D integrated
task scope are not 0.5A defects.

## 24. Architecture conclusion

The existing capability/operation pipeline remained sufficient: **YES**.

Selection required a Collection subsystem: **NO**.

Stable rule: selection targets semantic objects, not indices. Child indices
and Table row numbers are temporary backend addressing derived from fresh
current public structure. Success requires fresh authoritative readback that
proves the exact intended current target is selected.

## 25. v0.5 roadmap

- Discovery: **COMPLETE**.
- 0.5A: **COMPLETE / VALIDATED**.
- 0.5B: **PLANNED / NOT AUTHORIZED**.
- 0.5C: **PLANNED / NOT AUTHORIZED**.
- 0.5D: **PLANNED / NOT AUTHORIZED**.

Only 0.5A became complete in this phase. No later phase is automatically
authorized.

## 26. Recommended next direction

**A. Recommend 0.5B — Hierarchy Reveal and Page Continuation.**

Do not start it without explicit user authorization.

## 27. Git status

- Branch: `v0.5/task-interaction-completeness`.
- HEAD: exact final evidence HEAD is recorded in the final phase response.
- Worktree: clean at handoff.
- Remote: synchronized after the two phase commits.
- Public tags: unchanged.
- v0.5.0 tag/release: absent and not planned.

## 28. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone HANDOFF、
`docs/planning/v0.5-task-interaction-completeness.md`、
`docs/planning/v0.5-roadmap.md` 和最新 0.5A HANDOFF。当前分支为
`v0.5/task-interaction-completeness`；精确 HEAD 见最终阶段交接。v0.4 已
COMPLETE / MILESTONE QUALIFIED，0.5A 已 VALIDATED。Selection 的目标是
语义对象而不是 index；child index/Table row number 只是在 fresh public
structure 上临时解析的后端寻址；成功必须由 fresh、exact、target-specific
selection readback 证明。stale reorder/replacement authority 不迁移；未实现的
virtual target 不可选；multi-selection 仍排除；动态变化继续复用 v0.4。
推荐下一阶段为 0.5B Hierarchy Reveal and Page Continuation，但尚未自动授权。
v0.5 仍是内部里程碑，无 RC/tag/release；v0.6/v0.7 仍为后续里程碑，下一次
计划公开发布仍为 v1.0.0。不得自行扩展。
