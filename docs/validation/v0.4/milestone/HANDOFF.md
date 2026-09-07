# GUI2TUI v0.4 — Semantic Workflow Reconstruction Milestone Qualification

## 1. Status

- Starting HEAD: `485b2de98dd1cc4fa37a82b537e853102ec7423f`
- Final HEAD: the documentation/policy commit containing this handoff
- Branch: `v0.4/semantic-workflow-reconstruction`
- Worktree at handoff: clean
- Production changes: none
- Tests changed: none
- Release-policy change: v0.4–v0.7 are internal milestones; public release
  engineering resumes for the planned v1.0.0 release
- v0.4: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**
- Overall: **GUI2TUI v0.4 SEMANTIC WORKFLOW RECONSTRUCTION MILESTONE
  QUALIFIED**

## 2. Public release policy

- Last currently planned public pre-1.0 release: `v0.3.0`
- Next currently planned public release: `v1.0.0`
- v0.4: internal milestone
- v0.5: internal milestone
- v0.6: internal milestone
- v0.7: internal milestone
- v1.0.0: next planned public release

Milestone completion does not authorize a package-version bump, RC, tag,
artifacts, GitHub Release, publication, or the next milestone. A pre-1.0
public release remains possible only after explicit user authorization for a
documented exception.

## 3. What changed

Previous assumption: each completed architectural milestone would normally be
followed by release-candidate qualification and a public release.

New policy: v0.4–v0.7 close through internal Milestone Qualification. After
v0.7, the default path is a functional/architectural feature freeze, 1.0
Integration & Stabilization, a separately authorized v1.0.0 RC, and a
separately authorized public v1.0.0 release.

Why: dynamic continuation, task completeness, runtime continuity, and the
deployment contract are tightly coupled parts of one pre-1.0 product contract.
The project keeps continuous architectural qualification while concentrating
public packaging and release engineering in the integrated 1.0 effort.

What did not change: release engineering remains essential; every milestone
still requires bounded evidence and quality gates; the 1.0 product contract,
safety principles, immutable release history, and explicit-authorization rules
remain intact.

## 4. Milestone qualification policy

Every internal milestone still requires:

- a qualified architectural theme and satisfied phase exit criteria;
- P0 = 0 and P1 = 0;
- relevant Linux live semantic/task evidence;
- representative prior-layer regression;
- preserved genericity, authority, security, and privacy invariants;
- one normal full source-quality pass at milestone close when source changed;
- current architecture/documentation and a qualification handoff;
- a healthy base for the next milestone, followed by STOP and user review.

Milestone qualification intentionally skips public release artifacts,
public-package smoke, tags, GitHub Releases, published checksums/manifests, and
release-provenance/attestation campaigns. It never means deferring the
milestone's own testing until 1.0.

## 5. v0.4 final status

- Discovery: COMPLETE — conclusion B, narrower continuation model sufficient
- 0.4A: COMPLETE / VALIDATED
- 0.4B: COMPLETE / VALIDATED
- 0.4C: COMPLETE / VALIDATED
- 0.4D: COMPLETE / VALIDATED
- Functional development: COMPLETE
- Milestone qualification: QUALIFIED
- P0: 0
- P1: 0
- P2: 0
- Public v0.4.0 RC: NOT PLANNED UNDER CURRENT PRE-1.0 POLICY
- Public v0.4.0 release: NOT PLANNED UNDER CURRENT PRE-1.0 POLICY

Qualification reuses the already completed evidence and final 0.4D quality
pass. No Rust suite, Linux live campaign, package build, or release workflow
was rerun for this documentation-only policy close.

## 6. v0.4 architectural result

```text
user semantic action
  -> bounded authoritative transition
  -> fresh current scope and public structure
  -> ordinary TuiScene
  -> fresh current binding
  -> user manual continuation
```

Invocation return, event arrival, and elapsed time are not semantic truth.
Events wake observation; fresh authoritative GUI semantics establish truth,
and deadlines only terminate observation. `RuntimeNodeId` may preserve
presentation continuity but never transfers authority to a replacement
`BackendLocator`. Visibility and focus do not independently grant interaction
authority; current exact semantic evidence and `InteractionScope` do.
Realization changes current availability, not ownership. Only fresh public
structure establishes hierarchy; stale historical relation and focus locators
do not migrate. A fresh current `SceneBinding` authorizes the user's next
manual operation. No workflow engine or automatic continuation was required.

The detailed basis remains indexed by:

- [v0.4 Discovery](../../../planning/v0.4-workflow-reconstruction.md)
- [0.4A transition observation](../transition-observation/HANDOFF.md)
- [0.4B surface/scope continuation](../surface-scope-continuation/HANDOFF.md)
- [0.4C realization continuation](../realization-continuation/HANDOFF.md)
- [0.4D continuation UX](../continuation-ux/HANDOFF.md)

## 7. Roadmap to 1.0

- v0.5: Task & Interaction Completeness — next Discovery recommended, not
  authorized
- v0.6: Runtime Continuity & Multi-Surface Robustness — planned, not authorized
- v0.7: Deployment & Environment Completeness — planned, not authorized
- 1.0 integration: cross-milestone integration and stabilization after v0.7
  feature freeze, not authorized
- v1.0.0 RC: planned only after integration, separately authorized
- v1.0.0 release: next planned public release, separately authorized

v0.8 and v0.9 are not automatically planned. Inserting another architectural
milestone requires evidence of a distinct missing layer and explicit user
approval.

## 8. Roadmap milestone vs package version

v0.4, v0.5, v0.6, and v0.7 are roadmap milestone names. They do not imply
public `0.4.0`, `0.5.0`, `0.6.0`, or `0.7.0` packages or tags. Their identity
is retained through planning, validation evidence, branch history, and commits
until the 1.0 public release process resumes.

- Package version changed: NO

## 9. Existing release integrity

- v0.1.0: unchanged at
  `b4f4c530326cf5623bc75c9a16d54dfd55e6e81a`
- v0.2.0: unchanged at
  `578fcb2dcdfc07954587cc019caff2ba11982659`
- v0.3.0: unchanged at
  `efc704adf8a3ded3463ed8bb81670eddd08296c3`
- v0.4.0 tag: ABSENT
- v0.4.0 GitHub Release: ABSENT

An actual maintenance release for the public v0.3 line would require a new
version such as v0.3.1 and separate explicit authorization.

## 10. Files changed

- `docs/planning/roadmap-to-1.0.md`
- `docs/planning/v0.4-roadmap.md`
- `docs/project-guide.md`
- `docs/validation/v0.4/milestone/HANDOFF.md`

`AGENTS.md`, production source, tests, fixtures, package metadata, and release
workflows remain unchanged.

## 11. Checks

- `git diff --check`: PASS
- `python3 scripts/check-docs.py`: PASS
- Rust tests: NOT RUN — documentation/policy-only task; 0.4D already performed
  the final full v0.4 quality qualification

## 12. Git status

- Branch: `v0.4/semantic-workflow-reconstruction`
- HEAD: the documentation/policy commit containing this handoff
- Worktree: clean at handoff
- Remote: current branch is pushed normally at milestone close

## 13. Next recommended technical direction

**v0.5 — Task & Interaction Completeness Discovery**

Authorization: **NOT YET AUTHORIZED**. Do not create its branch or begin the
Discovery from this milestone close.

## 14. Next Codex context

先读 `AGENTS.md`，再读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、`docs/planning/v0.4-roadmap.md` 和
`docs/validation/v0.4/milestone/HANDOFF.md`。`v0.3.0` 是当前计划中最后一个
公开的 pre-1.0 release；`v0.1.0`、`v0.2.0`、`v0.3.0` 均保持不可变。v0.4
已 COMPLETE / MILESTONE QUALIFIED，不计划 v0.4.0 RC、tag 或 release。
v0.4–v0.7 是内部里程碑，但每个里程碑仍必须独立完成证据与 qualification；
下一个计划公开版本是 v1.0.0。当前精确分支为
`v0.4/semantic-workflow-reconstruction`，HEAD 为包含本 HANDOFF 的政策文档
提交。下一项建议工作是 v0.5 Task & Interaction Completeness Discovery，但
尚未授权；内部里程碑完成不会自动启动下一里程碑。没有新证据和用户批准时
不得自动增加 v0.8/v0.9。不要自行扩展范围。
