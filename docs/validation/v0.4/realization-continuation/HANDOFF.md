# GUI2TUI v0.4 Phase 0.4C — Realization and Hierarchical Continuation

## 1. Status

- Phase: 0.4C
- Starting HEAD: `ac5c25be00aa3660365c63d3b8f9a1d10097a284`
- Production commit: `a8f3c3e` (`fix: keep realization authority exact`)
- Branch: `v0.4/semantic-workflow-reconstruction`
- Worktree at handoff: clean
- Production changes: exact relation-target resolution and exact-locator focus
  restoration across realization/replacement
- New interaction families: none
- P0: 0
- P1: 0
- Overall: **PHASE 0.4C REALIZATION AND HIERARCHICAL CONTINUATION VALIDATED**

## 2. Commits

1. `a8f3c3e` — `fix: keep realization authority exact` (production and two
   focused invariant tests)
2. Evidence, validation fixtures, and planning qualification are committed
   with this handoff.

No hierarchy, ownership, workflow, Expand, or Selection production commit was
needed or created.

## 3. Architecture result

Existing cache replacement, event invalidation, full refresh, ordinary scene
construction, `SceneBinding`, Choice reconstruction, exact command palette,
`InteractionScope`, focus model, and `RegionNavigator` were sufficient for
manual continuation. Two concrete generic correctness gaps were hardened:

1. A cached relation target is now resolved only by the exact locator exposed
   by the public relation. Retaining a target `RuntimeNodeId` can no longer
   rewrite historical L1 relation evidence onto replacement locator L2. A
   fresh public relation read for L2 qualifies normally.
2. When focus restoration has an exact locator anchor, a missing/stale locator
   now chooses a safe current fallback. It does not follow a reconciled
   `RuntimeNodeId` onto a locator replacement. Runtime-only focus restoration
   remains available where presentation continuity is deliberately the only
   anchor.

New hierarchy subsystem: **NO**. New ownership subsystem: **NO**. New workflow
engine: **NO**. New Expand capability: **NO**. New Selection capability:
**NO**.

## 4. Realization model

- **Realization:** fresh semantics create or reveal current nodes while the
  relevant current application/scope remains valid.
- **Disappearance:** a prior locator is absent or unavailable in fresh
  authoritative semantics; its old binding has no operation authority.
- **Replacement:** L1 disappears and L2 appears. A presentation ID may be
  reconciled, but authority and historical structural facts do not migrate.
- **Fresh current binding:** a binding rebuilt from the current generation,
  exact locator, current scope, visibility, capability, and public structure.
- **Stale historical binding:** any earlier binding/locator whose exact target
  or current authority no longer validates.

GUI2TUI does not infer cause, ownership, hierarchy, identity, or a next target
from timing, names, geometry, sibling position, toolkit conventions, or event
order.

## 5. Sibling realization evidence

Fixture: GTK4 Demo, `Constraints` disclosure in `Demo list`.

Trigger: exact advertised `listitem.collapse` and `listitem.expand` actions,
used only by the validation inspector. Before collapse, `Constraints`, `Simple
Constraints`, and `Interactive Constraints` were current rows. A fresh read
confirmed Expanded absent/false and both related rows disappeared. Their old
exact locators became unavailable. A fresh read after exact expand confirmed
Expanded and new row locators.

Actual structure: the trigger Button and `Simple Constraints` Button live
under different `ListItem` parents; those parents are siblings beneath the
same `List`. Neither ControllerFor nor ControlledBy provided exclusive
trigger-to-row ownership. Fresh Choice discovery contained the re-realized
row. The validation client used the ordinary explicit semantic refresh,
navigated by Tab to the exact current `Demo list` Choice owner, and opened the
existing terminal-native Choice overlay; `Simple Constraints` was reachable.
The overlay was cancelled without selecting a demo, and the original expanded
state was restored authoritatively.

Conclusion: sibling realization supports truthful manual continuation without
invented ownership or a new tree/Expand capability.

## 6. Structural descendant evidence

Fixture: the bounded Qt6 validation fixture.

Parent/container: exact public `Descendant container` `QGroupBox`. Before the
trigger its child was absent. Exact existing Press on `Toggle descendant
realization` created `Realized descendant toggle`; a fresh authoritative walk
placed the CheckBox below that container in actual parent/child ancestry.

The ordinary rebuilt command model exposed a fresh current checkbox binding.
The user selected that binding through the existing command palette, and the
existing exact Toggle plus 0.4A `exact-node-state` condition confirmed Checked
in fresh GUI state. No descendant search or hierarchy-specific navigation was
added.

Conclusion: **PASS**. True hierarchy came only from current public containment.

## 7. Collapse/disappearance evidence

Current realized node: the Qt `Realized descendant toggle`, plus the GTK
`Simple Constraints` row as independent locator evidence.

The Qt palette entry was frozen while the CheckBox existed. Another exact
current fixture action removed the node. Fresh semantics confirmed its absence
and status `descendant removed`. Enter on the frozen palette entry was refused
as unavailable; its old AT-SPI locator was dead, and current command discovery
contained no such action. The existing focus model remained usable and the
fixture could be re-realized and used again.

The GTK collapse likewise made the old row locator unavailable. No vanished
node remained operation-authoritative.

Conclusion: **PASS**.

## 8. Re-realization / replacement authority

- Old RuntimeNodeId: live reuse was **NOT OBSERVED** and was not forced;
  deterministic 0.4A coverage exercises preserved-ID replacement.
- Old locator: L1, the exact first Qt CheckBox object path; it became
  unavailable after deletion.
- New RuntimeNodeId: a fresh current runtime binding; no identity equivalence
  was claimed.
- New locator: L2, a different exact Qt CheckBox object path after re-creation.
- Presentation continuity: permitted in the cache model, not required by this
  toolkit run.
- Old authority: refused and could not act on L2.
- Fresh authority: the user selected the new current binding; its existing
  Toggle operation was confirmed from fresh Checked state.

Result: **PASS**. Presentation continuity never grants replacement authority.

## 9. Ambiguous ownership

Case: GTK disclosure-related rows appeared after `Constraints` expanded but
were public siblings, not descendants of the trigger. Public semantics
established only the two distinct `ListItem` parents, their common `List`
parent, row roles/states/locators, and the trigger's Expanded state. They did
not establish an exclusive owner, row range, or trigger-to-row relation.

GUI2TUI refused to infer ownership from adjacency or event timing. The rows
remained usable through their actual current List/Choice semantics. Result:
**PASS**.

## 10. Unrelated realization

With the first Qt descendant present, an independent exact action realized
`Unrelated realized action` under a different public `Unrelated container`.
Fresh ancestry placed it only under that container and not under `Descendant
container`; no ControllerFor/ControlledBy relation associated it with the
first realization. It appeared normally in current command discovery, but no
operation attribution or ownership edge was recorded. Both fixtures were then
restored to their initial absent state. Result: **PASS**.

## 11. Fresh realized binding

New current binding: the re-created Qt `Realized descendant toggle` at L2.
Existing capability: generic exact Toggle. User selection: ordinary current
command palette search. Operation: exact advertised action plus the existing
0.4A exact-node-state transition. Authoritative result: fresh AT-SPI state was
Checked. Result: **PASS**.

## 12. Focus and navigation

When an exact focused locator disappears, focus now refuses to follow a
retained runtime ID onto a replacement; it chooses the first safe current
focusable element. An exact current locator restores its current element, and
a runtime-only presentation anchor can still preserve deliberate presentation
continuity. The live stale entry refusal left the TUI usable for a fresh
command and successful operation.

Tab reached the freshly rebuilt GTK Choice owner. F6/Shift-F6,
Ctrl-Tab/Ctrl-Shift-Tab, Tab/Shift-Tab, and `RegionNavigator` continue to derive
only from the ordinary current scene and were not changed. New hierarchy
navigator: **NO**.

## 13. Public structure rule

Current exact parent/child containment and freshly exposed public relations
may establish current hierarchy. Role/state/capability may qualify the current
nodes inside that structure. Geometry may arrange presentation only. Names are
descriptive but cannot establish ownership or replacement identity. Event
timing can wake refresh but cannot attribute realization. Historical
parent/child or relation facts must be revalidated and cannot migrate through
`RuntimeNodeId` reconciliation.

## 14. 0.4A / 0.4B reuse

The 0.4A contract remains unchanged: events wake or invalidate; fresh reads
decide truth; exact locator/session/generation/scope authority is mandatory;
deadlines never create success. The preserved-runtime-ID replacement test ran
in the Linux phase runner.

The 0.4B rules also remain unchanged: visibility/focus alone do not grant
authority, current `InteractionScope` confines operations, hidden content is
not actionable, and ownerless surfaces receive no invented owner. Realized
nodes outside current scope would not become actionable merely by appearing.

## 15. v0.5 boundary

- Selection, deselection, multi-select, and row-selection breadth remain v0.5.
- Broad Expand/Collapse product actions and recursive expansion remain v0.5.
- Tree task completeness, automatic paths, and descendant targeting remain
  v0.5 or are explicit non-goals.
- File chooser navigation, listing selection, Open/Save, and overwrite flows
  remain v0.5.
- Virtualized/full-hierarchy completeness remains later task coverage; current
  `PartialRealized` semantics remain honest.
- No menu, submenu, or dialog task breadth was added.

## 16. Existing capability regression

Single-line EditSession, Value, complex text, conflict refusal, password
exclusion, Choice, Reader, External Modality, and spatial presentation were not
refactored. GTK live evidence specifically reused the existing Choice model;
Qt reused the existing generic Toggle/action path. Existing automated coverage
remained green.

## 17. Genericity audit

Production has no application or toolkit branch, geometry-derived hierarchy,
name ownership, event-time attribution, input injection, anonymous action,
fuzzy identity, window-title matching, private API, or backing-file bypass.
GTK/Qt names occur only in fixtures and validation evidence. Result: **PASS**.

## 18. Tests and quality

Two focused existing regression tests were strengthened:

1. `historical_relation_target_never_follows_locator_replacement` proves a
   retained runtime ID cannot move an old relation target from L1 to L2, while
   a fresh relation read for L2 resolves normally.
2. `exact_locator_restores_focus_but_stale_locator_does_not_follow_runtime_id`
   proves stale exact focus authority falls back safely, current exact focus
   works, and runtime-only presentation restoration remains available.

The pre-existing 0.4A
`preserved_runtime_id_never_transfers_old_locator_authority` test also ran.
Targeted macOS and Ubuntu tests passed. The live runner is
`tests/live/v04c_run_linux.sh`; the probe is
`tests/live/v04c_realization_continuation.py`. It ran on Ubuntu 24.04 arm64,
Xvfb/X11, session D-Bus, AT-SPI, GTK4, and Qt6. The phase-close full quality
pass completed with format, all-target check/test, clippy with warnings denied,
docs audit, and diff check all passing.

## 19. Required result summary

```text
SIBLING_REALIZATION_MANUAL_CONTINUATION=PASS
STRUCTURAL_DESCENDANT_CONTINUATION=PASS
COLLAPSED_NODE_STALE_BINDING_REFUSAL=PASS
REREALIZED_LOCATOR_AUTHORITY_SEPARATION=PASS
UNRELATED_REALIZATION_NO_ATTRIBUTION=PASS
AMBIGUOUS_REALIZATION_OWNERSHIP_REFUSAL=PASS
FRESH_REALIZED_BINDING_CURRENT_AUTHORITY=PASS
FOCUS_AFTER_REALIZATION_CHANGE=PASS
HIERARCHY_USES_PUBLIC_STRUCTURE_ONLY=PASS
```

## 20. Remaining issues

- P0: 0
- P1: 0
- P2: 0

Planned v0.5 task/interaction breadth is not a 0.4C defect.

## 21. Architecture conclusion

Realization did **not** require an ownership/hierarchy subsystem. Manual
continuation from the current scene remained sufficient.

Stable rule: realization changes current semantic availability; it does not
retroactively create ownership or operation identity. Only fresh public
structure establishes current hierarchy. Historical relations and stale
bindings never acquire authority over re-realized replacements, while a fresh
current binding can be used normally.

## 22. v0.4 roadmap

- Discovery: complete — conclusion B, narrower continuation model sufficient
- 0.4A: complete / validated
- 0.4B: complete / validated
- 0.4C: complete / validated
- 0.4D: planned / not authorized

## 23. Recommended next direction

**A. Recommend 0.4D — Continuation UX and Theme Qualification.** Do not start
it without explicit user authorization.

## 24. Git and release status

The work remains on `v0.4/semantic-workflow-reconstruction`; current HEAD is
the commit containing this handoff, based on production commit `a8f3c3e`. The
worktree is clean at handoff. The branch is ahead of its remote until this
phase is pushed normally. The immutable `v0.3.0` release source/tag remains
`efc704adf8a3ded3463ed8bb81670eddd08296c3`. No RC or release work occurred.

## 25. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.4 Discovery、v0.4 roadmap、0.4A
HANDOFF、0.4B HANDOFF 和本 0.4C HANDOFF。当前精确分支为
`v0.4/semantic-workflow-reconstruction`，HEAD 为包含本 HANDOFF 的提交；
0.4A/0.4B/0.4C 均已验证。事件只唤醒，fresh authoritative semantics 才决定
事实；`RuntimeNodeId` 可保留展示连续性，但不会授予替换对象 authority，旧
locator authority 永不迁移。可见性/focus 不产生交互 authority，realization
也不产生 ownership；只有 fresh public structure 能建立当前 hierarchy，旧
binding 不能操作 re-realized replacement。用户仍从普通刷新场景手动继续。
精确建议下一阶段仅为 0.4D Continuation UX and Theme Qualification，但 0.4D
未自动授权；v0.5/v0.6/v0.7 仍是后续路线层。不要自行扩展范围。
