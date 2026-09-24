# GUI2TUI v1.0 Integration & Stabilization Discovery HANDOFF

## 1. Status

- Discovery: **COMPLETE**.
- Starting HEAD: `298076239326666dc3abc2eb9c53af216c8c6591`.
- Final HEAD: the documentation commit containing this handoff; obtain the
  exact close commit with `git rev-parse HEAD`.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree at Discovery start: clean.
- Remote at Discovery start: synchronized.
- Package version: `0.3.0`, unchanged.
- Production source changes: none.
- Test/fixture changes: none.
- Documentation changes: Discovery report, Integration Roadmap, long-term
  roadmap status, current x86_64 limitation wording, and this handoff.
- v1.0 Integration implementation: **NOT AUTHORIZED / NOT STARTED**.
- v1.0.0 RC/tag/release/public publication: **NOT AUTHORIZED / NOT STARTED**.

## 2. Answer to the product question

GUI2TUI is close to a useful 1.0 for a deliberately bounded Headless-first
Linux contract, but it is not yet a release-ready 1.0 product. The gap is the
integration proof and contract freeze between independently qualified layers:

- the final installed package has not rerun the complete v0.5 common-task
  baseline as one user journey;
- final terminal usability has not been observed across the declared narrow,
  normal and resized PTY cases in this Discovery;
- 1.0 performance/stability thresholds have not been frozen and measured on
  the final package; and
- support rows, architecture breadth, package artifacts, and public release
  metadata have not yet been frozen for v1.0.

The current architecture is sufficient. No new semantic, workflow, runtime,
deployment, Remote Companion, or application-adapter layer is required.

## 3. Evidence reviewed

The required context was read in order: `AGENTS.md`, `docs/project-guide.md`,
`docs/planning/roadmap-to-1.0.md`, v0.4-v0.7 milestone HANDOFFs,
`docs/planning/v0.7-headless-first-contract.md`, and
`docs/planning/v0.7-roadmap.md`. The review then inspected current source,
tests, installation/session/Doctor/launcher paths, TUI selector/help/status
paths, user documentation, v0.5 common-task evidence, v0.6 continuity
evidence, and v0.7 package/environment evidence.

Historical evidence retained as historical evidence includes:

- v0.4 dynamic continuation, modal scope, realization and stale-command
  refusal;
- v0.5 structured forms, selection/table rows, PageTab, hierarchy, menus/
  dialogs, Open File, Choose Folder, Reader and safe negative boundaries;
- v0.6 generation isolation, event overflow convergence, terminal/handler
  ownership, bounded artifacts and mixed lifecycle G1-G7; and
- v0.7 exact package install/Doctor/Managed, Docker interactive PTY, same-host
  SSH -> Managed, and bounded Weston Native Wayland/XWayland evidence.

These records were not relabelled as new final-source Linux evidence.

## 4. New checks performed in this Discovery

On the available macOS arm64 development host:

- Git branch/HEAD/worktree/remote/tag state: consistent with the v0.7
  HANDOFF; worktree clean at start, branch synchronized, public tags unchanged.
- `python3 scripts/check-docs.py`: PASS before documentation changes.
- `cargo fmt --all -- --check`: PASS.
- `cargo check --all-targets`: PASS.
- `cargo test --all-targets`: PASS — 300 library tests, 2 inspector CLI tests,
  and 5 user CLI tests.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- Product CLI help was inspected for `gui2tui`, `doctor`, `setup`, `app add`
  and `inspect`.
- `--session desktop doctor --json` produced bounded, contents-free failure
  guidance for the unavailable macOS session bus; `--session managed` refused
  the absent descriptor with `gui2tui setup persistent` guidance.

Linux AT-SPI, Managed Xvfb, Docker, SSH, Wayland, package, real terminal,
virtual-console, and long-running live campaigns were **NOT RUN in this
Discovery** because the available host is macOS and this task authorized only
bounded Discovery evidence. The existing v0.7 Linux evidence remains
attributable to its recorded package/source/topology identities.

## 5. Findings by severity

### P0

None found. No new permission, privacy, authority, password, data-integrity,
false-success, or unsafe fallback defect was reproduced.

### P1

No reproduced P1 product defect is open.

Two P1-level 1.0 release gates remain open as evidence/contract gates:

1. **G1 — final installed common-task proof:** v0.5D qualifies the baseline,
   but v0.7E's installed package rerun covered a representative subset rather
   than the full form/selection/table/PageTab/hierarchy/chooser/Reader path.
2. **G2 — final support contract:** Linux VT, native x86_64 full environment
   integration, ordinary desktops, Wayland over SSH and broader environment
   rows are not proven. They must be separately qualified or explicitly kept
   out of the 1.0 Supported matrix.

These are not claims that the current implementation fails those scenarios;
they are release blockers because the evidence is incomplete or the support
claim is not yet frozen.

### P2 / evidence and documentation

- Final-source terminal usability at narrow/wide/resized sizes is not newly
  live-observed in this Discovery.
- Final-package long-run performance/stability thresholds are not yet frozen;
  incomplete-cache large-tree startup remains a documented multi-second
  limitation.
- One current limitation sentence overstated x86_64 native runtime evidence;
  it was corrected to match the v0.7E emulation boundary.

### Known limitations / non-goals

Multi-selection completeness, Save As overwrite, exhaustive virtualization,
pointer-only context menus, drag/drop, rich-text lossless editing, universal
application/toolkit coverage, new-TTY attachment, cross-host Remote Companion,
and non-Linux semantic backends remain outside this Discovery and are not
converted into 1.0 implementation work.

## 6. Recommended next phase

Authorize **1.0A — Final-source product contract and end-to-end baseline**.
The first concrete output should be an explicit support matrix and evidence
manifest, followed by a bounded installed-package journey:

```text
install -> Doctor/session -> application selection or launcher
  -> one common task -> authoritative result -> safe exit/uninstall
```

Only after that run determines whether G1/G2 are merely missing evidence or
reproducible generic defects should 1.0B cross-milestone task integration
begin. The full recommended route is 1.0A, 1.0B task integration, 1.0C
terminal usability/recovery, 1.0D stability/performance/environment
qualification, and 1.0E product-contract/release qualification.

## 7. Git and release state

- Starting HEAD was exactly `298076239326666dc3abc2eb9c53af216c8c6591`.
- Public tags `v0.1.0`, `v0.1.1`, `v0.2.0`, and `v0.3.0` remain unchanged.
- Package version remains `0.3.0`.
- No production feature, package, RC, tag, release, publication, deployment,
  Remote Companion, or cross-host protocol was executed.
- Final documentation checks and final Git state are recorded in the closing
  response and by the documentation commit containing this handoff.

## 8. Next-session context

Read `AGENTS.md`, `docs/project-guide.md`,
`docs/planning/roadmap-to-1.0.md`,
`docs/planning/v1.0-integration-discovery.md`,
`docs/planning/v1.0-integration-roadmap.md`, and this HANDOFF first.

The next session is not authorized to implement v1.0 unless the user grants
1.0A explicitly. Preserve the current v0.7 environment matrix and its
limitations. Do not infer Linux VT, native x86_64 full integration, ordinary
desktop, Wayland-over-SSH, or cross-host support from other evidence. Preserve
the authority chain `RuntimeSessionId + ApplicationGenerationId + exact
BackendLocator + current InteractionScope + current capability + operation
ticket + authoritative readback`; preserve manual continuation and all
PasswordText/private-artifact/terminal single-owner invariants.

