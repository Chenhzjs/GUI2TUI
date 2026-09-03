# GUI2TUI v0.3 Phase 0.3A Validation Handoff

## 1. Status

- **Phase:** 0.3A — Capability Evidence & Operation Contract Qualification
- **Starting HEAD:** `554137b` (`docs: qualify v0.3 capability evidence contracts`)
- **Final HEAD:** documentation commit containing this handoff (reported by Git)
- **Worktree:** clean after the documentation commit
- **Roadmap:** `docs/planning/v0.3-roadmap.md`
- **Fresh Linux evidence:** collected in OrbStack Ubuntu 24.04 arm64 with the existing project setup, including a controlled reversible Value mutation
- **Production behavior changed:** no
- **Production source changed:** no
- **Tests added:** no
- **P0:** 0
- **P1:** 0
- **Overall:** **PHASE 0.3A CAPABILITY EVIDENCE & OPERATION CONTRACT QUALIFICATION VALIDATED**

Validation is evidence qualification, not feature exposure. No 0.3B–0.3D
implementation was started.

## 2. Persistent v0.3 roadmap

| Phase | Status |
| --- | --- |
| Discovery | COMPLETED |
| 0.3A Capability Evidence & Operation Contract Qualification | COMPLETED |
| 0.3B Native Bounded Capability Recovery | RECOMMENDED / AWAITING USER AUTHORIZATION |
| 0.3C Configurable Complex Text Interaction | PLANNED / NOT AUTHORIZED FOR IMPLEMENTATION |
| 0.3D Compound Interaction & Capability UX Qualification | PLANNED / NOT AUTHORIZED FOR IMPLEMENTATION |

Later phases remain not authorized; the roadmap has no release phase yet.

## 3. Evidence environment

- **Linux:** Ubuntu 24.04 arm64 guest (`gui2tui-live`, OrbStack kernel)
- **AT-SPI:** at-spi2-core 2.52, session D-Bus, registry activated; accessibility
  status enabled through `gdbus`
- **Display/session:** existing `dbus-run-session` + Xvfb (1280×800, X11),
  `NO_AT_BRIDGE=0`, `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`
- **Applications/fixtures:** GTK4 and Qt6 project live fixtures, a temporary
  validation-only Qt6 `QSlider` Value target, EOG 45.3,
  Google Chrome 152, Firefox 154 launch attempts. Existing v0.2 evidence for
  Mousepad, Qt Designer and LibreOffice Writer was used for document/partial
  realization context; no broad compatibility campaign was run.
- **Limitations:** Chrome exposed only an application root in this isolated
  file fixture. Firefox started a “Profile Missing” dialog in the probe
  profile, so no write was attempted; prior Firefox unchanged-readback
  evidence remains unverified rather than promoted. EOG exposed Value nodes
  (scroll bars) but their off-screen objects did not yield a safe reversible
  setter/read-back candidate.

Representative fresh excerpts:

```text
GTK TextInput "Username" value="alice" [editable,...]
  interfaces=[...,EditableText,Text]
GTK TextInput "GTK rich text article" [multi-line,...,read-only]
  interfaces=[...,EditableText,Text]
GTK List "Demo items" actions=[list.unselect-all,list.select-all]
  interfaces=[...,Selection]
EOG Slider [horizontal,...] interfaces=[...,Value]
Value target "Probe value" role=Slider current=4 min=0 max=10 increment=1
set_current_value(5) -> Ok; fresh CurrentValue -> 5.0
restore set_current_value(4) -> Ok; fresh CurrentValue -> 4.0
```

The dangerous Qt rich-text Text path was not re-invoked; existing generation
quarantine was inspected and retained.

## 4. Existing architecture reused

Evidence used the current `AtspiBackend` inspector/bootstrap, semantic cache,
`SemanticCapability` derivation, role-aware action resolver, Choice strategies,
`EditSession` authoritative read-back, `TextCapabilityStatus` quarantine,
`RuntimeSession`/`ApplicationGenerationId` tickets and refresh, bounded
`SemanticContentModel` (`PartialRealized`), event settling, and private artifact
ownership primitives. There is no `OperationRegistry` type or generalized
operation-trust table to reuse. `src/modality` remains resource viewing and
materialization infrastructure, not a mutation backend.

## 5. Single-line text reference contract

- **Preconditions:** current generation/node, plain TextInput role, Text and
  EditableText exposed, editable and non-password state, not `MultiLine`,
  active scope.
- **Invocation:** `EditableText.SetTextContents` through `EditSession`.
- **Verification:** settle events, independently read complete text, refresh
  node/presentation; local buffer is never authoritative.
- **Conflict:** starting value/fingerprint and generation are checked; external
  change rejects commit as stale.
- **Known variance:** GTK and Qt fixture commits read back; Chrome lacks
  EditableText; Firefox has historical apparent-success/unchanged-readback
  behavior and is not qualified by method return alone.

## 6. Text range / multiline findings

- **GTK:** fresh TextView exposed Text + EditableText but was read-only; bounded
  reads work. No whole-document write contract exists.
- **Mousepad:** existing evidence is Reader/readable multiline content; no
  atomic complete replacement path.
- **Firefox:** fresh probe reached a profile-error dialog; no mutation was
  attempted. Historical EditableText success with unchanged independent
  read-back remains unverified/rejected for qualification.
- **Qt:** Qt6.4 rich QTextEdit Text read previously crashed the bridge. The
  existing `Quarantined` generation state is the safe result; no repeat probe.
- **Writer:** existing long-document evidence is `PartialRealized`; bounded
  content is not a whole-document target.
- **Browser:** Chrome fixture exposed no usable child tree in this session;
  historical plain fields without EditableText remain read-only.
- **Completeness:** readable range/caret data is insufficient unless the model
  proves a complete intended mutation target. `PartialRealized`, virtualized,
  or bounded-window content is never qualified for whole-write-back.

## 7. Value operation contract

- **Public evidence:** fresh EOG tree contained Slider/scroll-bar roles with
  AT-SPI `Value`; a temporary validation-only Qt6 `QSlider` exposed current,
  minimum, maximum, and increment.
- **Preconditions:** role Slider, current=4, bounds 0..10, increment=1,
  writable controlled fixture, current generation and active scope.
- **Safe probe:** candidate 5 was in range, differed from 4, and had no
  destructive side effect.
- **Invocation:** public `Value.set_current_value(5.0)` returned `Ok(())`.
- **Verification:** fresh `Value.current_value()` returned `5.0`; this is the
  authoritative outcome, not the setter return.
- **Restoration:** `set_current_value(4.0)` returned `Ok(())`; an independent
  fresh read returned `4.0`, confirming the fixture was restored before exit.
- **Normalization:** none observed; returned value exactly matched the
  requested bounded value.
- **Failure:** rejected/unavailable/unchanged/timeout/stale/unverified must
  remain non-capabilities.
- **Qualification:** **QUALIFIED FOR LATER IMPLEMENTATION** as a bounded native
  candidate. The live contract was completed on the temporary Qt6 slider;
  this phase still does not expose Value in the product.

## 8. Selection operation contract

- **Public evidence:** fresh GTK List exposed Selection and explicit
  `list.select-all`/`list.unselect-all` actions; selected child state was
  observable. GTK ComboBox exposed Selection. Existing Qt/Firefox evidence
  confirms named child-action and visible-parent Selection strategies.
- **Preconditions:** visible/realized parent, stable child identity, active
  scope, compatible explicit action or accepted Selection call.
- **Safe probe:** select a reversible fixture item, then restore the original
  selection; do not touch destructive application lists.
- **Invocation:** current product supports single selection/Choice only;
  deselect, clear, select-all and multi-select are not semantic operations.
- **Verification:** selected state/set and refreshed children are authoritative.
- **Normalization:** application may canonicalize one selected child; report
  actual selected set.
- **Failure:** hidden/rejecting parent, unavailable children, stale identity,
  timeout or unchanged read-back remain refused.
- **Qualification:** **PARTIALLY QUALIFIED**; a narrow generic extension may be
  considered after 0.3B evidence, not broad multi-select UI.

## 9. Expand/collapse operation contract

- **Public evidence:** state vocabulary includes Expanded/Collapsed and tree
  realization is observable in existing scenes, but no role-compatible named
  expansion action was found in the resolver.
- **Preconditions:** explicit compatible action or public setter, current
  generation, and a bounded realization/read-back condition.
- **Safe probe:** metadata/state observation only; no guessed action index.
- **Invocation:** none qualified. Existing operation machinery can represent a
  future bounded sequence without introducing an IR type yet.
- **Verification:** at minimum Expanded/Collapsed state; if children are the
  purpose, refreshed child realization must also be observed.
- **Normalization/failure:** application rejection, absent realization,
  timeout, stale node, or unchanged state are not success.
- **Qualification:** **DEFERRED** pending generic evidence of an explicit
  semantic expansion operation and bounded realization behavior.

## 10. Invocation vs authoritative outcome

The distinction is demonstrated by the existing Firefox contract: an
`EditableText.SetTextContents` call returned apparent success, yet an
independent text read returned the old value. The outcome is therefore not
confirmed. Conversely GTK/Qt fixture edits required event settling plus a
fresh read before confirmation. A backend boolean is an invocation result;
only authoritative GUI state establishes confirmed, normalized, unchanged,
rejected, stale, or unverified outcome.

## 11. Trust / quarantine

`TextCapabilityStatus::{Unsupported,Declared,Verified,Quarantined}` and
generation-scoped failed-probe quarantine were sufficient. Runtime generation
identity, cancellation and refresh reject stale/late results. No generalized
trust framework or application/toolkit trust key was necessary. Future Value,
Selection and expansion work should record operation observations and retain
generation-scoped quarantine where a probe is unsafe; do not create a global
“Qt bad” or “Firefox bad” table.

`PartialRealized` is a content-completeness state, not quarantine; it blocks
whole-target mutation because the intended target cannot be proven complete.

## 12. Password safety

PasswordText remained excluded before content reads, logging, export,
materialization and mutation. No secret value was captured or included in
evidence. This boundary is absolute and unchanged.

## 13. Probe safety and bounds

Probes used a small controlled corpus, existing bounded traversal/timeouts and
reversible fixture state. No save/delete/destructive command, real user
document mutation, password read, keyboard/mouse injection, backing-file edit,
or repeated Qt crash path was used. Chrome/Firefox failures were recorded as
evidence, not retried indefinitely.

## 14. Production changes

Expected and actual: **none**. No diagnostic source change was required.

## 15. Tests

No tests were added. Existing live scripts and inspector commands were used
only for bounded evidence; the full release qualification suite was not run.

## 16. Genericity audit

No application-, process-, browser-, editor-, toolkit-, or window-title-based
production branch was introduced. Application names in this document are
validation evidence only.

## 17. Candidate qualification matrix

| Candidate | Result | Reason |
| --- | --- | --- |
| Single-line Text | ALREADY IMPLEMENTED / VERIFIED REFERENCE CONTRACT | Existing verified EditSession contract; GTK/Qt read-back evidence |
| Multiline Text | PARTIALLY QUALIFIED | Public reads exist, but complete writable target/range contract is absent |
| Value | PARTIALLY QUALIFIED | Live Value exposure observed; setter/read-back fixture still required |
| Selection | PARTIALLY QUALIFIED | Select strategies verified; deselect/clear/multi-select not yet qualified |
| Expand/Collapse | DEFERRED | State is observable, explicit safe invocation/realization contract missing |
| Partial Document | NOT QUALIFIED FOR WHOLE-TARGET MUTATION | `PartialRealized` is incomplete representation, not interface quarantine |

## 18. P0 / P1 / open questions

P0: **0**. P1: **0**.

Open questions are bounded to later authorized work: identify a controlled
writable Value fixture; determine whether a narrow Selection extension gives
generic value; establish a safe complete text/range representation; and find a
generic explicit expansion action. None authorizes implementation in this
phase.

## 19. Architecture conclusion

An exposed capability becomes trustworthy only when its public semantic
evidence identifies the target and operation, preconditions are explicit, the
probe/invocation is bounded and safe, generation/scope/conflict checks hold,
and an independent authoritative read-back confirms the resulting state (with
known normalization). Interface presence or a successful method return alone
is insufficient. Missing, partial, secret, crashing, rejected, anonymous, or
unverifiable operations must remain safely degraded.

## 20. Roadmap conclusion

0.3B remains the recommended next phase, but narrowly: **Value** is the first
candidate family, contingent on a controlled setter/read-back contract; a
small Selection extension may follow only if its evidence remains generic.
Expand/collapse and multiline/document mutation stay out of 0.3B. 0.3C and
0.3D remain planned and not authorized.

## 21. Documentation

- Roadmap: [docs/planning/v0.3-roadmap.md](../../../planning/v0.3-roadmap.md)
- Validation handoff: this document
- Project guide: [docs/project-guide.md](../../../project-guide.md), unchanged
- AGENTS.md: unchanged and normative

## 22. Git status

HEAD and worktree state are reported in the final response after committing
the two documentation files. The immutable `v0.2.0` tag/source was not moved.

## 23. Next Codex context

新的会话必须先阅读 `AGENTS.md`，再阅读 `docs/project-guide.md`、
`docs/planning/v0.3-roadmap.md` 和本 HANDOFF。当前 0.3A（Capability Evidence
& Operation Contract Qualification）已完成；下一推荐阶段是 0.3B Native Bounded
Capability Recovery，且仅建议先做受限 Value（在受控夹具中证明 setter/read-back，
必要时再评估窄 Selection 扩展）。0.3C/0.3D 仍未授权，后续阶段不会自动获得授权。
不要实现超出用户明确下一次授权的功能，不要添加外部 handler、多行编辑、
Expand/Collapse 或新的抽象；继续坚持无 DOM/CDP、无注入、无应用适配器、
PasswordText 永不导出。
