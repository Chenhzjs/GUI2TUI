# GUI2TUI v0.4 Phase 0.4B — Dynamic Surface and Scope Continuation

## 1. Status

- Phase: 0.4B
- Starting HEAD: `84e4650ab4238ed6025af49ab0993828620b4252`
- Production commit: `2056f7f243d7d0306a0847c25affe4733a3857b6`
- Branch: `v0.4/semantic-workflow-reconstruction`
- Production changes: bounded current-surface command qualification, exact
  palette binding revalidation, and exact temporary-surface disappearance
  observation
- New interaction families: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.4B DYNAMIC SURFACE AND SCOPE CONTINUATION VALIDATED**

## 2. Commits

1. `2056f7f` — `feat: continue across dynamic semantic surfaces`
2. Evidence and planning qualification are committed with this handoff.

## 3. Architecture implemented

The normal `CommandHierarchy` now retains both an already-supported structural
reveal action and its safe semantic descendants. Descendants inside a Menu are
actionable only while fresh Showing state exists on the Menu and command. The
ordinary scene strips Action bindings that are not current, visible, enabled,
and in the active scope.

Palette entries capture the exact `BackendLocator` and `UiIntent` alongside
their `RuntimeNodeId`. Execution revalidates all three against the rebuilt
current command hierarchy and active `InteractionScope`. A frozen entry cannot
follow presentation identity into a hidden, replaced, disappeared, or
background target.

The 0.4A observer gained one closed condition: an exact known temporary
surface is absent or no longer Showing. It is used for an action inside an
already Showing Menu, performs a fresh full semantic rebuild, and then returns
to the ordinary scene. Existing cache, scope analysis, focus history,
`RegionNavigator`, scene construction, operation tickets, event batching, and
0.4A authority remain the only mechanisms.

No workflow engine, menu framework, dialog subsystem, popup manager, surface
registry, automatic continuation runner, or persistent open-surface state was
added.

## 4. Surface authority model

- Visibility and Showing are semantic availability evidence, not authority by
  themselves.
- Focus is a usability hint only; it can select only an already qualified,
  present Window/Dialog/modal/popup scope.
- Structural containment places the Qt Menu subtree inside the existing
  Window scope without inventing a new popup scope.
- Public owner relations may qualify an existing popup rule. Their absence is
  never replaced by name, geometry, recency, or toolkit inference.
- A valid active modal remains the authoritative confined
  `InteractionScope`.
- Every next operation requires the fresh current scope, runtime node, exact
  locator, intent/capability, enabled state, and temporary-surface
  availability.

## 5. Same-scope temporary surface

The controlled Qt6 fixture exposes `Tools` with exact `ShowMenu`. Before the
operation, its Menu subtree and `Activate Demo` item exist at stable locators
but lack Showing. The existing action plus the 0.4A exact-node condition
confirmed Menu Showing. A full fresh rebuild kept the Window scope active and
made the exact item available in the ordinary command hierarchy.

The user then selected `Activate Demo` from the refreshed TUI. Exact `Press`
updated the authoritative fixture status counter and closed the Menu. The new
exact-surface-unavailable condition confirmed that the same Menu was no longer
Showing, rebuilt the ordinary scene, and removed the item from current
commands. No item was automatically selected.

## 6. Hidden/actionability behavior

- Before open: `Activate Demo` was absent from current and global actionable
  command search; the ordinary scene did not retain an Action binding for it.
- After open: fresh Menu and MenuItem Showing state made it available in the
  current Window scope.
- After close: fresh state removed it from actionable search again.
- Old binding: a palette entry frozen while the Menu was open was refused with
  `Command is no longer available in the current semantic surface` after an
  external exact Press closed the Menu. The authoritative counter remained
  unchanged. Reopening and selecting a fresh current entry worked normally.

## 7. Modal enter

The Qt `Open modal dialog` Button began in the main Window scope. 0.4A
confirmed a new active `ModalDialog` by fresh full read. Existing scope
analysis selected it, the ordinary scene was rebuilt for it, and Window
commands were absent from current command search. The dialog Close control was
then chosen by the user from the current TUI.

## 8. Modal exit

Exact Close used the existing scope-inactive condition. A fresh read confirmed
the dialog scope absent and restored the Window as current. Dialog bindings
were no longer in the current scene. Existing exact focus reconciliation chose
a safe current binding; a subsequent user Toggle in the restored Window was
authoritatively confirmed. No historical dialog operation authority survived.

## 9. Ownerless popup

The Qt Tools Menu exposed no `PopupFor` owner relation. It was nevertheless an
exact semantic descendant of the current Window and could be used in that same
scope while Showing. Production created neither an owner edge nor a
`MenuPopup` scope. The safe conclusion was containment plus current
availability, not graphical ownership.

## 10. Ambiguous surface

A deterministic controlled semantic tree contained one ownerless Menu under
the current Window and one focused ownerless Menu detached directly under the
Application. Focus on the detached Menu did not change authority, no popup
scope was manufactured, and its node was denied by `allows_node`; the
contained Menu remained usable in the Window. This refusal test ran in the
same Linux phase runner as the live workflows.

A proposed detached Qt `QMenu` probe was discarded because Qt did not place it
in the selected application's public AT-SPI root. Treating an inaccessible
toolkit object as semantic evidence would have been invalid.

## 11. Disappeared surface

The Qt Menu was opened and a TUI palette entry for its current item was frozen.
An independent exact AT-SPI Press closed the surface. After event-driven
refresh, Enter on the old palette entry was refused; it did not increment the
fixture's authoritative counter. A fresh ShowMenu followed by a fresh current
item selection incremented the counter, proving the surface was not made
permanently unusable.

## 12. Cross-implementation evidence

Qt exposed a permanently present Menu subtree whose Showing states toggled.
GTK4 exposed a created modal Window/Dialog subtree and destroyed it on Close.
Both used the same generic role/state/containment/scope and 0.4A observation
rules. Event and cache shapes differed: GTK's AT-SPI Cache service could retain
a destroyed dialog record briefly, while a fresh recursive walk showed the
current 47-node main surface after the 52-node modal surface. Events remained
wakeups; fresh semantics decided authority.

## 13. File chooser lifecycle

Status: **NOT TESTED**. The Discovery-only temporary GTK file chooser was not a
retained repository fixture. Recreating it would add task-adjacent setup after
the required Qt and GTK surface families had already qualified scope entry,
confinement, destruction, and return. No path navigation, directory listing,
selection, Open/Save, or other file task support was added. Those remain v0.5
task/primitive coverage.

## 14. 0.4A reuse

Events only wake/invalidate and remain coalesced by the existing batch path.
Fresh exact-node or full-application reads establish truth. Exact
session/generation/locator/scope authority remains mandatory. The observer's
deadline can only stop waiting. Missing-event and unrelated-event semantics
from 0.4A are unchanged. Exact temporary disappearance is semantic success;
elapsed time and an event are not.

## 15. Focus and navigation

Existing per-scope exact focus anchors and `reconcile_identity` were reused.
Modal entry initialized safe current focus; exit restored a valid Window
anchor or a safe current fallback. No fuzzy restoration and no second focus
system were added. F6, Shift-F6, Ctrl-Tab, Ctrl-Shift-Tab, Tab, Shift-Tab,
Reader/command navigation, and `RegionNavigator` were unchanged and remain
driven only by the current scene.

## 16. Existing capability regression

Single-line EditSession, Value, complex text, conflict refusal, password
exclusion, Choice, Reader, External Modality, and spatial presentation were not
refactored. Choice remains a direct semantic task rather than a forced popup
lifecycle. Existing full tests cover these paths; representative Qt/GTK live
fixtures retained password redaction, current Text/Choice/Value content, and
normal scene construction.

## 17. Genericity audit

Production contains no application/toolkit branch, window-title or
name-based ownership, geometry authority, input injection, fixed-delay
success, anonymous action guess, fuzzy identity, private API, or backing-file
bypass. Toolkit names occur only in fixtures/evidence. The result is **PASS**.

## 18. Tests and quality

Three focused regression tests cover:

1. hidden Menu descendants require fresh Showing while their structural
   trigger remains available;
2. contained ownerless Menu stays in the Window while detached ownerless Menu
   receives no authority from focus;
3. exact temporary-surface hidden/removal state confirms disappearance without
   requiring the initiating item to survive.

The live runner is `tests/live/v04b_run_linux.sh`; its bounded pexpect/Inspector
probe is `tests/live/v04b_surface_scope_continuation.py`. It ran in the
established Ubuntu 24.04 arm64 Xvfb/X11, session D-Bus, and AT-SPI environment.
Targeted Rust checks, Python compilation, shell syntax, and live workflows
passed. The phase-close full suite passed: `cargo fmt --all -- --check`,
`cargo check --all-targets`, `cargo test --all-targets`, and `cargo clippy
--all-targets -- -D warnings`. The docs audit reported
`DOCUMENT_AUDIT=PASS files=63 local_links=200`; `git diff --check` also passed.

## 19. Required result summary

```text
SAME_SCOPE_MENU_CONTINUATION=PASS
TEMPORARY_VISIBILITY_ACTIONABILITY=PASS
MODAL_SCOPE_CONTINUATION_ENTER=PASS
MODAL_SCOPE_CONTINUATION_EXIT=PASS
MODAL_BACKGROUND_AUTHORITY_REFUSAL=PASS
OWNERLESS_POPUP_NO_SCOPE_INFERENCE=PASS
AMBIGUOUS_SURFACE_AUTHORITY_REFUSAL=PASS
DISAPPEARED_SURFACE_STALE_BINDING_REFUSAL=PASS
CROSS_IMPLEMENTATION_SURFACE_CONTINUATION=PASS
FILE_CHOOSER_SURFACE_LIFECYCLE=NOT_TESTED
```

## 20. Remaining issues

- P0: 0
- P1: 0
- P2: 0

Planned v0.5 interaction breadth is not a 0.4B defect.

## 21. Architecture conclusion

**YES**: existing `InteractionScope`, cache, ordinary scene, exact focus, and
0.4A transition architecture remained sufficient. 0.4B did not require a new
menu, dialog, popup, surface, or workflow subsystem.

Stable rule: visibility and focus alone do not create interaction authority.
Exact current semantic scope evidence decides authority. A contained temporary
surface may remain in the current scope, and ownerless surfaces never receive
invented ownership.

## 22. v0.4 roadmap

- Discovery: complete — conclusion B, narrower continuation model sufficient
- 0.4A: complete / validated
- 0.4B: complete / validated
- 0.4C: planned / not authorized
- 0.4D: planned / not authorized

## 23. Recommended next direction

**A. Recommend 0.4C — Realization and Hierarchical Continuation.** Do not start
it without explicit user authorization.

## 24. Git and release status

The work remains on `v0.4/semantic-workflow-reconstruction`. The public
`v0.3.0` tag remains immutable at
`efc704adf8a3ded3463ed8bb81670eddd08296c3`. No RC or release work occurred.

## 25. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 Discovery、v0.4 roadmap、0.4A HANDOFF
和本 HANDOFF。当前分支是 `v0.4/semantic-workflow-reconstruction`；以本文件
所在提交为当前 HEAD。0.4A 与 0.4B 均已验证。事件只负责唤醒，fresh
semantics 才确定事实；`RuntimeNodeId` 可维持展示连续性但不是 authority，
旧 locator 永不转移到替换对象。可见性或 focus 本身不产生交互 authority，
必须使用当前精确 scope/语义证据；无主 popup 永不获得虚构 owner。后续动作
仍由用户手动选择。建议的下一阶段仅为 0.4C Realization and Hierarchical
Continuation，但 0.4C 未自动授权；v0.5/v0.6/v0.7 仍是后续路线层。不要自行
扩展范围。
