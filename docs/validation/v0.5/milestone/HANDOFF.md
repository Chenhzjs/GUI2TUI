# GUI2TUI v0.5 — Task & Interaction Completeness Milestone

## 1. Status

- Milestone: GUI2TUI v0.5 — Task & Interaction Completeness
- Starting HEAD: `0e7b4265bad0ee0e95d4f51dbb73144c005c1d93`
- Final HEAD: the documentation/status commit containing this handoff
- Branch: `v0.5/task-interaction-completeness`
- Production changes during close: none
- Tests changed during close: none
- Functional development: **COMPLETE**
- Milestone qualification: **QUALIFIED**
- P0: 0
- P1: 0
- Public release: **NOT PLANNED**
- Overall: **GUI2TUI v0.5 TASK & INTERACTION COMPLETENESS MILESTONE QUALIFIED**

This close relies on the final functional-source qualification already
performed by 0.5D. It adds no capability, production behavior, test, package
version, tag, artifact, release workflow, or later-milestone work.

## 2. Qualification basis

The qualification basis is:

- [v0.5 Discovery](../../../planning/v0.5-task-interaction-completeness.md):
  **COMPLETE**, conclusion A — Task-Completeness Milestone Justified;
- [0.5A current collection selection](../current-collection-selection/HANDOFF.md):
  **COMPLETE / VALIDATED**;
- [0.5B hierarchy/page continuation](../hierarchy-page-continuation/HANDOFF.md):
  **COMPLETE / VALIDATED**;
- [0.5C file/folder choice](../file-folder-choice/HANDOFF.md):
  **COMPLETE / VALIDATED**;
- [0.5D common-task baseline](../common-task-baseline/HANDOFF.md):
  **COMPLETE / VALIDATED** and v0.5 functional development complete.

The [v0.4 milestone](../../../validation/v0.4/milestone/HANDOFF.md) remains the
qualified continuation foundation. No evidence contradiction was found during
this documentation/status audit.

## 3. Final architecture

The qualified task model is:

```text
current semantic scene
  -> user chooses one verified current semantic operation
  -> fresh authoritative public semantics establish its result
  -> ordinary current scene is rebuilt
  -> user manually chooses the next current operation
  -> later authoritative application semantics establish task completion
```

The current scene remains the user's task state. v0.5 required no
`TaskSession`, `WorkflowEngine`, task planner, task graph, form session,
FileChooser session, or task-specific runtime. It also required no Collection,
Tree, Page, Dialog, or Menu subsystem.

## 4. Qualified common-task baseline

The following baseline is qualified when sufficient public Accessibility
semantics exist:

| Task family | v0.5 result | Honest boundary |
| --- | --- | --- |
| Bounded structured forms | COMPLETE | Secret content remains unavailable; unsupported contained controls degrade safely. |
| Supported common dialogs | COMPLETE | Only current scoped and publicly actionable controls operate. |
| Exact named Buttons, links, commands, and qualified menus | COMPLETE | Anonymous and pointer-only invocation remains unavailable. |
| Authoritative single-item selection | COMPLETE | Multi-selection is not reinterpreted as single Select. |
| Authoritative Table-row selection | COMPLETE | Only exact current realized rows are targets. |
| PageTab switching | COMPLETE | Insufficient target-specific current-page evidence is unsupported. |
| Qualified hierarchy reveal | COMPLETE WITH SAFE LIMITATION | Ambiguous generic Toggle is not Expand. |
| Open existing file | COMPLETE WITH SAFE LIMITATION | Only current public chooser semantics and realized rows are used. |
| Choose Folder | COMPLETE WITH SAFE LIMITATION | Object type and canonical path are not invented. |
| Reader/current collection interaction | COMPLETE WITH SAFE LIMITATION | Partial semantic content remains partial. |

This is a bounded common-task contract, not universal widget, toolkit,
application, or GUI compatibility.

## 5. Stable selection rules

Selection targets exact current semantic objects, not indices. Child indices
and Table row numbers are temporary backend addressing derived from fresh
current public structure for one immediate invocation. Confirmation requires
fresh target-specific selected-object or selected-row readback. Focus, backend
acceptance, event arrival, and integer membership are not selection truth.

Reorder and replacement never transfer old authority. A fresh current binding
may operate; the stale binding cannot select the object that inherited an old
position. Unknown or unrealized virtual targets are not selectable. General
multi-selection, ranges, deselection, and Select All remain outside the
qualified baseline.

## 6. Stable hierarchy and page rules

Hierarchy reveal requires an exact current target, an unambiguous compatible
public operation, and fresh readback of the requested `Expanded` state.
Generic Toggle is not Expand. Realization changes availability; it does not
establish ownership, reparent siblings, or justify inferred descendants.

Page switching targets an exact current semantic `PageTab`, never a retained
tab index. Different public backend mechanisms may implement switching, but
fresh target-specific current-page evidence is always required. Hidden or
stale old-page authority does not migrate. No TreeNavigator, hierarchy engine,
Page manager, or page runtime is required.

## 7. Stable task-composition rules

Open File and Choose Folder compose exact current Table selection/actions,
ordinary Button actions, public chooser state, v0.4 modal continuation, and a
fresh application-visible result. There is no FileChooser subsystem or task
session, and the user chooses every continuation step.

The backing filesystem never supplies missing GUI semantics. Production does
not infer file/directory type from names, suffixes, icons, geometry, or
filesystem metadata; it does not reconstruct a canonical path from
breadcrumbs. A PartialRealized chooser listing remains partial.

## 8. Primitive truth versus task truth

Primitive operation truth remains conservative and cannot be manufactured by
later progress. A multi-step task may nevertheless complete after an
unconfirmed primitive when fresh public GUI state presents a valid next scene,
the user manually continues, and later authoritative application semantics
prove the overall result.

Choose Folder is the canonical evidence: activating the folder row caused the
initiating object to disappear, so that primitive remained unconfirmed. Fresh
chooser location/listing semantics then became current; the user manually
confirmed; and the caller's fresh semantics proved the exact folder result.
The later task result did not retroactively confirm the earlier primitive.

## 9. Safe degradation and deliberate boundaries

Completeness includes `COMPLETE`, `COMPLETE WITH SAFE LIMITATION`, `READ-ONLY
BY DESIGN`, `PARTIAL`, `SAFE UNSUPPORTED`, and `EXTERNAL MODALITY` outcomes
when public semantics are insufficient. False capability is never required.

The following remain deliberate non-baseline boundaries, not v0.5 defects:

- arbitrary multi-selection, ranges, general deselection, and Select All;
- Save As overwrite and broad destination authority;
- exhaustive virtualized traversal and arbitrary offscreen targeting;
- pointer-only context menus, drag/drop, and hover-only interaction;
- broad table editing and column selection;
- universal widget, toolkit, chooser, editor, and application compatibility.

## 10. v0.4 continuation and authority

v0.4 remains the single continuation layer: events wake/invalidate; fresh
reads establish truth; bounded observation terminates without time-based
success; `InteractionScope` confines operations; current surfaces and
realization rebuild ordinary `TuiScene`; stale bindings refuse; and the user
continues manually.

Operation authority remains current application generation, exact
`BackendLocator`, current scope, current capability, and fresh public
structure/state. `RuntimeNodeId` is snapshot-local. Index, name, geometry,
focus, event timing, and filesystem state do not establish identity,
ownership, current page, selection, or task success.

## 11. Genericity and safety

- Application-specific production branches: none.
- Toolkit-specific production branches: none.
- Private toolkit/application APIs: none.
- Keyboard/mouse injection or coordinates: none.
- Anonymous action or action-index guessing: none.
- Filesystem semantic bypass or backing-file mutation: none.
- Name/index/fuzzy identity: none.
- Inferred hierarchy or false completeness: none.
- Task-specific runtime infrastructure: none.
- Result: **PASS**.

## 12. Milestone qualification gates

| Gate | Result | Evidence |
| --- | --- | --- |
| v0.5 Discovery completed | PASS | Discovery conclusion A |
| 0.5A exact current Selection validated | PASS | 0.5A handoff |
| 0.5B hierarchy reveal and PageTab switching validated | PASS | 0.5B handoff |
| 0.5C Open File and Choose Folder composition validated | PASS | 0.5C handoff |
| 0.5D integrated common-task baseline validated | PASS | 0.5D handoff |
| P0 = 0 | PASS | 0.5D status |
| P1 = 0 | PASS | 0.5D status |
| Final integrated Linux task qualification | PASS | 0.5D quality evidence |
| Final complete source-quality matrix | PASS | 0.5D quality evidence |
| Genericity audit | PASS | 0.5D audit |
| No private API, input injection, or filesystem semantic bypass | PASS | 0.5C/0.5D audits |
| No task-specific runtime subsystem | PASS | 0.5D architecture conclusion |
| Deliberate non-baseline boundaries explicit | PASS | 0.5D negative boundaries |
| Healthy source base for v0.6 Discovery | PASS | all prior gates; no open P0/P1 |

All milestone qualification gates pass.

## 13. Quality basis and close checks

0.5D ran the final integrated Linux qualification and the complete final-source
matrix: macOS source quality, `cargo fmt`, `cargo check`, `cargo test`,
`cargo clippy -D warnings`, documentation audit, and diff check all passed.

No Rust suite or Linux live campaign was rerun during this milestone close
because the final functional source was already qualified by 0.5D and this
task changes documentation/status only. The close runs only the repository
documentation audit and `git diff --check`.

## 14. Release integrity

- Package version: remains `0.3.0`.
- Existing public tags: unchanged.
- `v0.3.0^{}`: remains
  `efc704adf8a3ded3463ed8bb81670eddd08296c3`.
- `v0.5.0` tag: absent.
- v0.5 RC, artifacts, release, and publication: not planned and not created.

v0.3.0 remains the last currently planned public pre-1.0 release. v0.4-v0.7
remain internal milestones, and v1.0.0 remains the next planned public
release.

## 15. Roadmap and next direction

- v0.4: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.5: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.6: **PLANNED / NEXT DISCOVERY RECOMMENDED / NOT AUTHORIZED**.
- v0.7: **PLANNED / NOT AUTHORIZED**.
- 1.0: **PLANNED / NOT AUTHORIZED**.

The exact next recommendation is **v0.6 — Runtime Continuity & Multi-Surface
Robustness Discovery**. It is **NOT YET AUTHORIZED**. This close does not
create a v0.6 branch or planning record and does not begin runtime work.

## 16. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone HANDOFF 和本 v0.5 milestone
HANDOFF。v0.3.0 仍是最后一个计划中的 1.0 前公开版本；v0.4 与 v0.5 均已
COMPLETE / MILESTONE QUALIFIED。当前分支为
`v0.5/task-interaction-completeness`，精确 HEAD 为包含本 HANDOFF 的文档提交。
Selection 面向语义对象而非索引；child/Table/tab index 仅是从 fresh public
structure 派生的临时后端寻址。generic Toggle 不等于 Expand；realization 不
建立 ownership；PageTab 面向精确当前语义 tab。任务通过已验证的当前语义原语
组合，而不是 task-specific runtime；即使后续权威应用语义证明总体任务成功，
原语真值仍保持保守。文件系统永不补足 GUI 缺失语义，partial semantic content
始终保持 partial。multi-selection、Save As overwrite、穷尽虚拟化、pointer-only
context menu 与 drag/drop 是有意的非基线边界；用户手动决定每一步。精确下一
建议是 v0.6 Runtime Continuity & Multi-Surface Robustness Discovery，但尚未
自动授权；v0.6 仍是内部 milestone，无 v0.6.0 RC/tag/release；v0.7 留待以后，
v1.0.0 仍是下一计划公开版本。不得自行扩展。
