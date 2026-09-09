# GUI2TUI v0.5 Phase 0.5D — Common Task Baseline Qualification

## 1. Status

- Phase: 0.5D — Common Task Baseline Qualification
- Starting HEAD: `e74e596c4be278a1638bf22c509f85dc8ea413f0`
- Final HEAD: the evidence/docs commit containing this handoff
- Branch: `v0.5/task-interaction-completeness`
- Worktree: clean at handoff
- Production changes: none
- New interaction families: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.5D COMMON TASK BASELINE QUALIFICATION VALIDATED**

The bounded baseline passed through existing generic capabilities. This closes
v0.5 functional development only; milestone qualification remains pending
separate user authorization.

## 2. Commits

- Production/tests: none; the qualified source is unchanged from the completed
  0.5C source.
- Evidence/docs: `docs: qualify v0.5 common task baseline`

## 3. Final v0.5 task model

```text
current semantic scene
  -> user chooses one current verified semantic operation
  -> authoritative public semantics establish its post-state
  -> ordinary fresh scene
  -> user manually chooses the next operation
  -> later authoritative application semantics establish task completion
```

There is no `TaskSession`, `WorkflowEngine`, task planner, or task-specific
runtime. The application remains authoritative and the current scene is the
user's task state.

## 4. 1.0 common-task baseline table

| Task | Status | Primary semantic capabilities | Dynamic continuation | Safe limitation | Result |
| --- | --- | --- | --- | --- | --- |
| Structured form | COMPLETE | Text, radio/Choice, Toggle, Value, PageTab, Button | v0.4 fresh page/result scene | password content unreadable | PASS |
| Common dialog | COMPLETE | exact Action, Toggle, modal scope | modal enter/exit and caller return | only publicly actionable controls | PASS |
| Named command/menu | COMPLETE | exact advertised Action | current popup/menu surfaces | pointer-only context menu unsupported | PASS |
| Single selection | COMPLETE | exact current parent Selection | fresh scene/readback | multiselect not reinterpreted | PASS |
| Table-row task | COMPLETE | current Table structure and row Select | fresh row/caller state | current realized rows only | PASS |
| PageTab | COMPLETE | exact PageTab Action or TabList Selection | v0.4 page refresh | insufficient target readback is unsupported | PASS |
| Hierarchy | COMPLETE WITH SAFE LIMITATION | exact expansion action and Expanded state | v0.4 realization | ambiguous Toggle is not Expand | PASS |
| Open File | COMPLETE WITH SAFE LIMITATION | Table row Select, exact Button Action | chooser exit and caller return | public current realized rows only | PASS |
| Choose Folder | COMPLETE WITH SAFE LIMITATION | exact cell Action, Button Action | fresh listing/location and caller return | no invented object type or canonical path | PASS |
| Reader/current collection | COMPLETE WITH SAFE LIMITATION | Reader, List/Table content models | ordinary current scene | partial content stays partial | PASS |

All claims remain conditional on sufficient public Accessibility semantics.

## 5. Structured form

- Fixture: temporary controlled Qt6 multi-page form.
- Steps: edit Username; choose the Dark radio option; toggle Enable; adjust a
  bounded Value; switch to Advanced; edit its current field; toggle its check;
  submit.
- Text: exact fresh value became `alice-v05d`.
- Choice/radio: Dark became authoritatively checked; existing bounded Choice
  remained available independently.
- Toggle: Enabled and Advanced confirmation were freshly checked.
- Value: the public slider value became 5.
- Page: exact Advanced PageTab became current; page-A-only command was absent.
- Submit: current exact Button action.
- Authoritative result: caller exposed the exact combined submitted values.
- Result: **PASS**; password content never appeared in normal TUI or Inspector.

## 6. Collection task

- Fixture: temporary GTK4 single-selection collection plus follow-up action.
- Target: exact current Beta semantic object.
- Selection: normal Choice/Select interaction used current parent Selection.
- Readback: fresh Selected state and caller label both identified exact Beta.
- Follow-up: user invoked `Use selected item` from the refreshed scene.
- Task result: `Result: collection Beta`.
- Result: **PASS**.

## 7. Table task

- Fixture: the controlled GTK chooser Files Table from 0.5C.
- Target row: exact current `beta.txt` accessible cell/row.
- Fresh addressing: current TableCell/Table mapping derived the temporary row.
- Selection: public Table row selection, not flattened child selection.
- Readback: fresh selected row mapped back to the exact target.
- Follow-up: current confirmation action closed the chooser and the caller
  exposed the exact intended synthetic result.
- Result: **PASS**. The unchanged source and frozen evidence were re-audited
  together with the 0.5A Table contract.

## 8. Hierarchy task

- Target: current GTK disclosure with explicit expansion semantics.
- Expand/readback: exact public action followed by fresh `Expanded=true`.
- Realization: v0.4 rebuilt current public siblings/descendants.
- Actual structure: sibling-shaped content remained siblings; no inferred
  ownership.
- Manual continuation: user navigated the ordinary refreshed scene.
- Collapse: exact action followed by fresh `Expanded=false`.
- Stale binding: old realized binding refused.
- Result: **PASS** in the current-HEAD v0.5B Linux live rerun.

## 9. Menu/dialog task

- Menu/action: current nested Tools / More commands / Apply command sequence.
- Surface/modal: each menu surface and a controlled modal dialog became current.
- Scope: active modal scope confined available operations.
- Current controls: user toggled the dialog option and invoked current Apply.
- Background: background Submit command was absent; frozen background authority
  also refused in the current-HEAD v0.4B rerun.
- User continuation/return: every step was manual; caller scene returned with
  an exact result.
- Result: **PASS**.

## 10. Open File

- Target: exact current synthetic `beta.txt` Table row.
- Selection: 0.5A target-specific Table selection/readback.
- Confirm/exit: current exact confirmation; v0.4 chooser exit.
- Caller result: exact synthetic file result in fresh application semantics.
- Filesystem semantic inference: none in production.
- Result: **PASS**, inherited from the immediately preceding qualified 0.5C
  source/evidence with no intervening production change.

## 11. Choose Folder

- Navigation: exact current public TableCell Activate.
- Current semantics: fresh checked breadcrumb and current listing.
- Fresh listing: old root rows disappeared; current nested content appeared.
- Acceptance/exit: manual current confirmation and v0.4 modal return.
- Caller result: exact intended synthetic folder.
- Filetype guessing: none.
- Result: **PASS**, inherited from the same unchanged 0.5C source/evidence.

## 12. Reader / partial content

- Reader: existing semantic Reader remains available for current complete and
  partial content; full source regression passed.
- Current List/Table: readable current semantics remain useful even when a
  mutation is not qualified.
- Partial realization: current chooser/virtual collection evidence remains
  explicitly `PartialRealized`.
- Complete-content claim: none.
- Result: **PASS**.

## 13. Negative boundaries

- Multi-selection: no single-Select reinterpretation; existing independent
  Toggle behavior is not called Select.
- Context menu: no public opener means **SAFE UNSUPPORTED**.
- Virtualized offscreen target: unknown/unrealized object has no authority.
- Save As overwrite: deferred and unqualified.
- Drag/drop: not required; no pointer simulation.
- Other: exhaustive virtualization, selection ranges, general deselection,
  column selection, hover-only tasks, and universal compatibility remain out.

## 14. Primitive truth vs task truth

In Choose Folder, activating the folder row made the initiating object
disappear, so that primitive remained conservatively unconfirmed. Fresh public
chooser semantics then showed a new current listing/location, the user manually
confirmed, and the final caller semantics proved the intended folder result.
The later task result did not retroactively weaken the primitive operation's
exact readback requirement.

## 15. v0.4 reuse

- Events: wake/invalidate only.
- Fresh reads: establish truth.
- Scope/surface: current modal and menu surfaces control authority.
- Realization: changes availability, never inferred ownership.
- Stale bindings: refuse after replacement, hiding, collapse, or exit.
- Current scene/manual continuation: ordinary rebuilt `TuiScene`; user chooses
  every next operation.

## 16. 0.5A reuse

- Selection/Table rows: exact current semantic targets with target-specific
  readback.
- Index: temporary backend addressing only.
- Reorder/replacement: old authority refused; fresh binding remained usable.
- Virtualization: unrealized target unavailable.
- Multiselect: excluded from single Select.

## 17. 0.5B reuse

- Expand: unambiguous current public action plus desired Expanded readback.
- Ambiguous Toggle: never Expand.
- PageTab: exact current semantic tab, never retained tab index.
- Old-page authority: hidden page command unavailable/refused.

## 18. 0.5C reuse

- File/folder composition: current generic operations, no chooser runtime.
- Filesystem/file type: never supplies or guesses missing GUI semantics.
- Partial listing: remains partial.
- Cancel: no false completion.
- Chooser stale commands: refuse after traversal and exit.

## 19. Authority audit

Exact current locator, generation, scope, capability, and fresh public
structure/readback establish authority. `RuntimeNodeId` is snapshot-local;
indices are temporary; names, geometry, focus, events, time, and filesystem
state do not establish identity, ownership, selection, current page, or task
success. Result: **PASS**.

## 20. False-affordance audit

Stale targets, hidden page controls, modal background commands, ambiguous
Expand, unknown virtual targets, pointer-only context menus, multiselect items
as single Select, and invented chooser types were not advertised. Result:
**PASS**.

## 21. Genericity audit

- App/toolkit branches: none.
- Private APIs/input injection/coordinates: none.
- Name or index identity/filesystem inference: none.
- Task-specific runtime: none.
- Result: **PASS**.

## 22. Existing feature regression

Text, Toggle, Choice, Value, complex text, exact Button/action, Selection,
Table row, Expand, PageTab, Reader, password safety, spatial/navigation, and
External Modality passed through the integrated live evidence, the current
v0.5B/v0.4B live reruns, and the final full source suite. No feature family was
changed in 0.5D.

## 23. Required result summary

```text
COMMON_TASK_STRUCTURED_FORM=PASS
COMMON_TASK_COLLECTION_SELECTION=PASS
COMMON_TASK_TABLE_ROW=PASS
COMMON_TASK_HIERARCHY=PASS
COMMON_TASK_MENU_DIALOG=PASS
COMMON_TASK_MODAL_CONFINEMENT=PASS
COMMON_TASK_OPEN_FILE=PASS
COMMON_TASK_CHOOSE_FOLDER=PASS
COMMON_TASK_READER=PASS
COMMON_TASK_NO_INDEX_IDENTITY=PASS
COMMON_TASK_NO_FALSE_HIERARCHY=PASS
COMMON_TASK_CHOOSER_TYPE_HONESTY=PASS
COMMON_TASK_PARTIAL_REALIZATION_HONEST=PASS
COMMON_TASK_VIRTUAL_TARGET_REFUSAL=PASS
COMMON_TASK_CONTEXT_MENU_SAFE_UNSUPPORTED=PASS
COMMON_TASK_MULTISELECT_BOUNDARY=PASS
COMMON_TASK_MANUAL_CONTINUATION=PASS
COMMON_TASK_CURRENT_SCENE_MODEL=PASS
COMMON_TASK_NO_FALSE_SUCCESS=PASS
COMMON_TASK_NO_FALSE_AFFORDANCE=PASS
COMMON_TASK_STALE_AUTHORITY_REFUSAL=PASS
COMMON_TASK_V04_CONTINUATION_REUSED=PASS
COMMON_TASK_NO_WORKFLOW_UI=PASS
```

## 24. Tests / quality

- Production changed: no.
- New focused tests: none.
- Integrated live tasks: structured form, collection/follow-up, nested menu,
  modal, stale reorder, virtual/multiselect/context negatives; all PASS.
- Linux: current-HEAD 0.5B and v0.4B suites PASS; 0.5A/0.5C frozen-source
  evidence remains applicable because production is unchanged.
- macOS: final complete source matrix PASS.
- fmt/check/test/clippy/docs/diff: **PASS**.

The full matrix ran once on the final 0.5D source.

## 25. Remaining issues

- P0: 0.
- P1: 0.
- P2: 0.

Deliberate non-baseline boundaries are not v0.5 defects.

## 26. v0.5 exit definition

Every agreed task either completes through generic public semantics or has an
explicit safe degradation; exact target and task results remain authoritative;
indices never become identity; stale authority refuses; hierarchy and PageTab
operations remain exact; file/folder tasks compose existing primitives without
filesystem inference; partial/unsupported breadth remains honest; v0.4 owns
continuation; user steps remain manual; no task-specific subsystem exists; P0
and P1 are zero.

**v0.5 FUNCTIONAL DEVELOPMENT COMPLETE**

## 27. Architecture conclusion

- TaskSession required: **NO**.
- WorkflowEngine required: **NO**.
- FileChooser/Tree/Page subsystems required: **NO**.

Stable rule: task completeness is composition of verified current semantic
operations. Primitive truth remains conservative even when later authoritative
application state establishes overall task completion.

## 28. v0.5 roadmap

- Discovery: **COMPLETE**
- 0.5A: **COMPLETE / VALIDATED**
- 0.5B: **COMPLETE / VALIDATED**
- 0.5C: **COMPLETE / VALIDATED**
- 0.5D: **COMPLETE / VALIDATED**
- Functional development: **COMPLETE**
- Milestone qualification: **PENDING USER AUTHORIZATION**

This handoff does not mark v0.5 milestone-qualified.

## 29. Roadmap boundary

- v0.6: unchanged / not authorized.
- v0.7: unchanged.
- v1.0.0: next planned public release.
- v0.5 RC/tag/release: not planned.

## 30. Recommended next direction

**A. Recommend v0.5 Milestone Qualification / Close.** It is not started or
authorized by this phase.

## 31. Git status

- Branch: `v0.5/task-interaction-completeness`
- HEAD: the evidence/docs commit containing this handoff
- Worktree: clean
- Remote: synchronized at handoff
- Package version: unchanged
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, and `v0.3.0` unchanged;
  `v0.3.0^{}` remains `efc704adf8a3ded3463ed8bb81670eddd08296c3`
- v0.5.0 tag/release: absent / not planned

## 32. Next Codex context

先读 `AGENTS.md`、`docs/project-guide.md`、`docs/planning/roadmap-to-1.0.md`、
v0.4 milestone HANDOFF、v0.5 Discovery、v0.5 roadmap、0.5A/0.5B/0.5C
HANDOFF，以及本 0.5D HANDOFF。当前分支为
`v0.5/task-interaction-completeness`，HEAD 为包含本 HANDOFF 的证据提交。
v0.4 已 COMPLETE / MILESTONE QUALIFIED；0.5A、0.5B、0.5C、0.5D 均已
VALIDATED，v0.5 FUNCTIONAL DEVELOPMENT COMPLETE，但 Milestone
Qualification 尚未获授权。Selection 面向语义对象而非索引；generic Toggle
不等于 Expand；PageTab 面向当前精确语义页签；文件/文件夹任务由当前通用
语义原语组合，不存在 FileChooser runtime，文件系统不能补足 GUI 语义，未知
类型不得猜测，partial 仍是 partial。即使后续应用状态证明整个任务成功，原语
操作真值仍须保守；用户手动决定每一步。多选范围、Save As overwrite、穷尽
虚拟化、pointer-only context menu、drag/drop 等仍是有意非基线边界。下一建议
仅为 v0.5 Milestone Qualification / Close；它不会自动授权。v0.5 仍是内部
里程碑，无 RC/tag/release；v0.6/v0.7 仍为未来里程碑，v1.0.0 仍是下一计划
公开版本。不得自行扩展。
