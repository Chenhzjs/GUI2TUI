# GUI2TUI v0.5 Phase 0.5C — File and Folder Choice Tasks

## 1. Status

- Phase: 0.5C — File and Folder Choice Tasks
- Starting HEAD: `33e9e85904ea8b52444ec918cbf5fd402a13d03d`
- Final HEAD: the evidence/docs commit containing this handoff
- Branch: `v0.5/task-interaction-completeness`
- Worktree: clean at handoff
- Production changes: one bounded generic Table presentation/actionability
  correction required by live task evidence
- New interaction families: none; existing Table-row Select, exact generic
  Activate, Button action, and v0.4 continuation compose the tasks
- P0: 0
- P1: 0
- Overall: **PHASE 0.5C FILE AND FOLDER CHOICE TASKS VALIDATED**

No package version, release workflow, RC, tag, public artifact, Save As
overwrite, semantic scrolling, multi-selection, or later phase was started.

## 2. Commits

- Production/tests: `feat: expose exact current table cell actions`
- Evidence/docs: `docs: validate v0.5 file and folder tasks`

The production commit is limited to generic current TableCell mapping,
presentation, selection return-to-scene behavior, and exact current Cell
Activate dispatch. It contains no chooser-specific type, workflow, mode, or
filesystem logic.

## 3. Architecture result

- Existing capabilities composed the tasks:
  - v0.4: bounded transition observation, modal scope, authoritative refresh,
    stale-binding refusal, chooser exit, and ordinary current-scene return.
  - 0.5A: exact current Table-row selection and target-specific readback.
  - 0.5B: not required by these two chooser sequences; its hierarchy/page
    behavior remained unchanged.
  - Existing Action/Text/etc.: exact current TableCell `Activate`, generic
    Button actions, readable Table content, and ordinary focus/navigation.
- New FileChooser subsystem: **NO**.
- New TaskSession: **NO**.
- Filesystem semantic backend: **NO**.

The one discovered generic blocker was a flat public Table structure whose
direct TableCells exposed their authoritative row/column positions only
through the TableCell interface. GUI2TUI now asks that public interface for
fresh positions and revalidates each exact cell against `Table.GetAccessibleAt`
before using the position for presentation or operation addressing.

## 4. Open File task contract

- Chooser authority: current modal `InteractionScope`, current semantic cache,
  current public Table/Selection/Button state, and fresh bindings.
- Target authority: exact current accessible TableCell locator and current
  public Table membership; neither label nor row number is identity.
- Selection: 0.5A derives a temporary row number from fresh public structure,
  invokes Table row selection, and proves the selected current row maps back
  to the exact target.
- Confirmation: the user invokes the current exact generic confirmation
  Button action; its accessible name is presentation, not hidden task logic.
- Task completion proof: exact selected target before confirmation, chooser
  exit, and a fresh application-visible result matching the intended
  synthetic target.
- Chooser exit: v0.4 removes chooser scope and invalidates chooser bindings.
- Application result: the controlled caller exposes an ordinary accessible
  `Result: OPEN:<synthetic-root>/beta.txt` status.
- Manual continuation: after each verified operation, the user chooses the
  next operation from the ordinary refreshed scene.

## 5. Open File live evidence

- Fixture: controlled GTK3 caller and native GTK file chooser on Linux arm64.
- Synthetic tree: `alpha.txt`, `beta.txt`, `folder-a/nested.txt`, and
  `folder-b`; no personal content was accessed.
- Chooser: modal Open chooser exposing a public PartialRealized Files Table,
  Selection, exact cell actions, Cancel, and Open controls.
- Initial scene: caller opened the chooser; the chooser became the active
  scope and background caller commands were absent.
- Target: exact current accessible row whose displayed semantic content was
  `beta.txt`.
- Selection proof: normal TUI Table Select reported `Selected "beta.txt"`
  only after target-specific 0.5A Table readback.
- Confirmation: the user invoked the current exact Open Button action.
- Chooser result: chooser disappeared and its old semantic scope ceased to be
  current.
- Application-visible result: fresh caller semantics exposed exactly
  `Result: OPEN:<synthetic-root>/beta.txt`.
- Return: the ordinary caller scene was rebuilt and remained usable.
- Result: **PASS**.

## 6. Choose Folder task contract

- Current location evidence: exact current checked breadcrumb and fresh
  listing where publicly exposed; no canonical path is reconstructed.
- Navigation operation: exact current generic TableCell Activate, qualified
  from current exact public action metadata and Table membership.
- Fresh location/listing: fresh checked breadcrumb, disappearance of old
  rows, and appearance of new current rows after v0.4 refresh.
- Folder acceptance: the user invokes the current exact confirmation action
  after reaching the intended current chooser state.
- Completion proof: chooser exit plus exact fresh application-visible
  selected-folder result.
- Chooser exit: v0.4 modal/surface continuation restores the caller.
- Application result: `Result: FOLDER:<synthetic-root>/folder-a`.
- Manual continuation: traversal never selects another step automatically.

## 7. Choose Folder live evidence

- Fixture: the same controlled GTK3 caller in folder-selection mode.
- Synthetic tree: the bounded synthetic tree from section 5.
- Starting location: synthetic root, represented by current public chooser
  controls and listing.
- Navigation: user moved to the exact current `folder-a` row and chose the
  contextual `Activate cell` command.
- Fresh location evidence: after the disclosure target disappeared, v0.4
  truthfully reported that the initiating action was not confirmed and showed
  the current interface; the fresh checked breadcrumb was `folder-a`.
- Fresh listing: the old root rows disappeared and a fresh current row for
  `nested.txt` appeared.
- Acceptance: the user manually returned to current controls and invoked the
  exact current chooser confirmation action.
- Application-visible result: fresh caller semantics exposed exactly
  `Result: FOLDER:<synthetic-root>/folder-a`.
- Return: chooser closed and the ordinary caller scene resumed.
- Result: **PASS**.

The intermediate operation-level unconfirmed result is intentional: target
disappearance cannot be turned into semantic success. The fresh chooser state
and final caller result establish task-level completion without weakening
v0.4.

## 8. File/directory type semantics

- Public type evidence: the fixture exposed a textual type cell for ordinary
  files but no reliable explicit file/directory type for the folder row.
- What was unavailable: a generic public object-kind contract distinguishing
  file, directory, and special location.
- What GUI2TUI did NOT infer: type from icon, image, suffix, name, slash,
  geometry, row position, filesystem metadata, or toolkit behavior.
- Normal TUI behavior: the row remains readable, selectable, and activatable
  through independently qualified semantics; it receives no invented
  `[File]` or `[Folder]` label.
- Safe limitation: Open/Choose Folder works where current public chooser
  controls suffice, but GUI2TUI does not claim generic filesystem object-type
  knowledge.

## 9. Location semantics

- Breadcrumb/current-location evidence: the controlled chooser exposed
  current ToggleButton breadcrumbs with a public checked state.
- Exact locator: every breadcrumb remains its exact current semantic object;
  the evidence document intentionally does not retain private raw locators.
- Checked/current state: after traversal, `folder-a` was freshly checked and
  the former root breadcrumb was not.
- Names: descriptive user-facing cues only, never operation identity.
- Path reconstruction: none.
- Filesystem lookup: none in production.
- Conclusion: **PASS** for a current public location cue, not a claim that
  GUI2TUI knows a canonical filesystem path.

## 10. Table-row selection reuse

- 0.5A target: exact current accessible TableCell semantic object.
- Temporary row index: freshly derived through public TableCell/Table mapping.
- Fresh readback: selected rows are mapped back through fresh current Table
  structure to the intended exact target.
- Stale index: never retained as identity or carried across listing changes.
- Chooser-specific selection logic: **NO**.

## 11. Traversal continuation

- Row/action: exact current TableCell with exact advertised `Activate`.
- Backend acceptance: not semantic success.
- Fresh GUI evidence: checked breadcrumb and current listing changed while the
  chooser remained active.
- Old listing: removed from current semantics.
- New listing: freshly rebuilt from current public structure.
- Current scene: ordinary v0.4 TuiScene, not a chooser workflow state.
- Automatic continuation: **NO**; user chose every next semantic action.

## 12. Stale row after traversal

- Old locator: exact locator captured for the pre-traversal `folder-a` row.
- Old row: belonged to the synthetic-root listing.
- Traversal/change: exact current Activate changed the chooser location and
  listing.
- New listing: contained the fresh `nested.txt` row.
- Old command: direct validation of the old exact object returned public
  `UnknownObject`; production additionally requires current Table mapping,
  operation authority, generation, and scope before invocation.
- GUI mutation: none from the stale command.
- Fresh current target: remained usable through a fresh binding.
- Result: **PASS**.

## 13. Closed chooser stale command

- Captured chooser command: current chooser-only confirmation action captured
  before successful completion.
- Completion: chooser accepted the intended folder and disappeared.
- Chooser scope: no longer current.
- Old command: exact old accessible object returned `UnknownObject` and could
  not pass production authority checks.
- Calling app: stayed current with the correct application-visible result.
- Result: **PASS**.

## 14. Cancel safety

- Selected target before cancel: one exact current chooser row was selected.
- Cancel: user invoked the current exact Cancel Button action.
- Chooser exit: chooser disappeared and current caller scope returned.
- Application result: fresh caller status was `Result: CANCELLED`.
- False completion: none; row selection was not reported as accepted file
  choice.
- Current scene: ordinary caller scene remained usable.
- Result: **PASS**.

## 15. Confirmation actionability

- Enabled: current generic actions require current Enabled/Sensitive semantics
  when these states are exposed.
- Visible/Showing: current exact target must remain visible/showing under the
  existing actionability rules.
- Scope: active modal scope must allow the target.
- Disabled case: **NOT TESTED**; the controlled GTK fixture kept its
  confirmation action enabled throughout the bounded scenarios.
- Normal command exposure: no disabled confirmation command was observed;
  production retains current enabled/showing/scope qualification.
- Result: **NOT TESTED**, with no production bypass or false claim.

## 16. Partial realization / virtualization

- Chooser completeness: public Files Table advertised `PartialRealized`.
- Realized rows: only exact currently accessible rows were shown and operable.
- Unrealized targets: neither synthesized nor addressable.
- Scrolling added: **NO**.
- Complete-directory claim: **NO**.
- Result: **PASS** — the normal Table view explicitly states that only
  realized cells are shown.

## 17. Save As boundary

- Basic no-overwrite evidence: **NOT TESTED**.
- Overwrite: not implemented or exercised.
- Destination authority: remains unqualified for overwrite tasks.
- Production change: none for Save As.
- v0.5 baseline: Open existing file and Choose Folder only.
- Conclusion: Save As overwrite remains an explicit planned boundary, not a
  0.5C defect.

## 18. Cross-implementation evidence

- GTK: Open and Choose Folder both passed end-to-end.
- Qt: **NOT TESTED** for a whole chooser task; no readily available Qt chooser
  harness justified expanding setup.
- Other: 0.5A and 0.5B already supplied GTK/Qt evidence for the underlying
  exact selection/action/continuation contracts.

Genericity remains supported because production contains no chooser, toolkit,
application, title, path, label, or filename branch. The added behavior is
qualified solely from public TableCell/Table structure and exact action
semantics.

## 19. v0.4 reuse

- Modal entry: active chooser became current confined scope.
- Scope confinement: caller background commands were unavailable.
- Events: wake/invalidate observation only.
- Fresh reads: establish selection, current listing, current location cue,
  chooser exit, and caller result truth.
- Realization: fresh public structure supplies current rows; no ownership is
  inferred from traversal.
- Exit: chooser disappearance returns to fresh caller semantics.
- Stale bindings: old listing and closed-chooser commands refuse.
- Current scene: ordinary rebuilt TuiScene is the task state.

## 20. 0.5A / 0.5B reuse

- Selection: 0.5A exact current target and authoritative readback reused.
- Table row: 0.5A public Table row operation and temporary addressing reused.
- Hierarchy: 0.5B Expand/Collapse was not needed.
- Page switching: 0.5B PageTab switching was not needed.

## 21. TUI UX

- Normal user steps: open chooser, inspect current scene, open current Table,
  Select or Activate an exact current cell, return to refreshed scene, invoke
  current confirmation or Cancel, then continue in the caller.
- Select: contextual Enter only where current Table-row selection qualifies.
- Activate: contextual `a Activate cell` only for an exact current Cell with
  the exact compatible public action.
- Confirm: existing current Button activation.
- Cancel: existing current Button activation.
- Type labels: no invented File/Folder category.
- Path labels: no reconstructed canonical path.
- Internal taxonomy: locators, row numbers, and transition conditions are not
  exposed in normal UX.
- Workflow UI: none.

## 22. Security / semantic authority

- Filesystem inspection: none in production.
- Backing-file bypass: none.
- Private chooser API: none.
- Input injection: none.
- Type guessing: none.
- Name identity: none.
- Icon inference: none.
- Coordinates: none.
- Result: **PASS**.

Validation knew only the bounded synthetic expected results. It did not turn
that fixture knowledge into production authority.

## 23. Existing capability regression

- Text: representative existing source tests passed.
- Toggle: representative existing source tests passed.
- Choice: representative existing source tests passed.
- Value: representative existing source tests passed.
- Complex text: representative existing source tests passed.
- Button/action: existing exact generic actions passed and drove both tasks.
- Selection: full existing tests passed; live exact file-row selection passed.
- Table row: live exact row selection and readback passed.
- Expand: existing tests passed; not modified.
- PageTab: existing tests passed; not modified.
- Reader: existing tests passed.
- Password: existing safety tests passed.
- Spatial/navigation: existing tests passed; normal Table navigation worked.

## 24. Genericity audit

- App branches: none.
- Toolkit branches: none.
- Window-title chooser detection: none.
- Button-name task detection: none.
- Filename extension inference: none.
- Filesystem stat: none.
- Name identity: none.
- Geometry: presentation only; not chooser semantics or identity.
- Input injection: none.
- Private API: none.
- Result: **PASS**.

## 25. Tests / quality

- Production/tests changed: yes; one focused generic Table regression test and
  one existing content-view test extension.
- New focused tests: flat Table public positions/nested text and contextual
  Table Activate/direct-close behavior.
- Targeted: affected Table/content-view tests and `cargo check --all-targets`
  passed during development.
- Linux live: **PASS** — controlled GTK Open, Choose Folder, stale, Cancel,
  partial-realization, and no-type-guess scenarios.
- macOS: full source quality matrix passed on the final source.
- fmt: **PASS**.
- check: **PASS**.
- test: **PASS**.
- clippy: **PASS** with warnings denied.
- docs: **PASS**.
- diff: **PASS**.

The normal full source-quality matrix was run once at phase close because
production source changed. Its first `cargo test --all-targets` attempt saw
one pre-existing artifact-lease test fail once; the exact test then passed
independently and in five consecutive targeted runs, and the complete
all-target suite passed on the qualification rerun. No unrelated runtime
source was changed.

## 26. Required result summary

```text
OPEN_FILE_TASK_COMPLETION=PASS
OPEN_FILE_EXACT_TARGET_COMPLETION=PASS
CHOOSE_FOLDER_TASK_COMPLETION=PASS
CHOOSE_FOLDER_EXACT_TARGET_COMPLETION=PASS
CHOOSER_MANUAL_CONTINUATION=PASS
CHOOSER_STALE_ROW_AFTER_TRAVERSAL_REFUSAL=PASS
CLOSED_CHOOSER_STALE_COMMAND_REFUSAL=PASS
CHOOSER_CANCEL_NO_FALSE_COMPLETION=PASS
CHOOSER_NO_FILETYPE_GUESSING=PASS
CHOOSER_PARTIAL_REALIZATION_HONEST=PASS
CHOOSER_CONFIRM_ACTIONABILITY=NOT_TESTED
CHOOSER_CURRENT_LOCATION_SEMANTICS=PASS
CROSS_IMPLEMENTATION_FILE_CHOICE=NOT_TESTED
BASIC_SAVE_AS_NO_OVERWRITE=NOT_TESTED
NO_FILE_CHOOSER_SUBSYSTEM=PASS
NO_FILESYSTEM_SEMANTIC_BYPASS=PASS
```

## 27. Remaining issues

- P0: 0.
- P1: 0.
- P2: 0 within 0.5C.

Save As overwrite, exhaustive virtualization/scrolling, and pointer-only
context menus are explicit baseline boundaries, not 0.5C defects. A disabled
confirm case and whole Qt chooser task were not exercised; existing generic
qualification safely refuses unavailable controls and contains no
chooser/toolkit special case.

## 28. Architecture conclusion

- Did existing generic semantic primitives compose into Open File? **YES**.
- Did they compose into Choose Folder? **YES**.
- Was a FileChooser subsystem required? **NO**.
- Was filesystem semantic inference required? **NO**.

Stable task-composition rule: task completion is composed from verified
current semantic operations and ordinary refreshed scenes; it does not require
a task-specific runtime. File/folder chooser truth comes only from public
Accessibility semantics and fresh application-visible results. The backing
filesystem never supplies missing GUI semantics.

## 29. v0.5 roadmap

- Discovery: **COMPLETE**.
- 0.5A: **COMPLETE / VALIDATED**.
- 0.5B: **COMPLETE / VALIDATED**.
- 0.5C: **COMPLETE / VALIDATED**.
- 0.5D: **PLANNED / NOT AUTHORIZED**.

Only 0.5C became complete in this phase. 0.5D did not start.

## 30. Recommended next direction

**A. Recommend 0.5D — Common Task Baseline Qualification.**

It remains a separately authorized qualification phase and must not add new
interaction families. Do not start it without explicit user authorization.

## 31. Git status

- Branch: `v0.5/task-interaction-completeness`.
- HEAD: evidence/docs commit containing this handoff.
- Worktree: clean at handoff.
- Remote: synchronized with origin at handoff.
- Public releases/tags: unchanged; v0.1.0, v0.2.0, and v0.3.0 remain
  immutable.
- v0.5.0 tag/release: absent and not planned.

## 32. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 milestone HANDOFF、v0.5 Discovery、
v0.5 roadmap、0.5A HANDOFF、0.5B HANDOFF 与本 0.5C HANDOFF。当前分支为
`v0.5/task-interaction-completeness`，精确 HEAD 以本次文档提交及最终交接
为准。v0.4 已 COMPLETE / MILESTONE QUALIFIED；0.5A、0.5B、0.5C 均已
VALIDATED。Selection 的目标是语义对象而非索引；generic Toggle 不等于
Expand；PageTab 的目标是精确当前语义 tab。文件/文件夹任务由当前通用语义
能力组合，而不是 FileChooser 子系统；文件系统状态永不补足缺失的 GUI 语义，
缺失的文件/目录类型不得猜测，部分 chooser listing 始终保持部分。用户控制每
一步继续，Save As overwrite 不在初始基线。下一项精确建议为 0.5D Common
Task Baseline Qualification，但未自动授权。v0.5 仍是内部 milestone，无
RC/tag/release；v0.6/v0.7 仍是后续 milestone，v1.0.0 仍是下一计划公开
版本。不得自行扩展。
