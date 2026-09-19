# GUI2TUI v0.7B Installation and Diagnostic Completeness Handoff

## 1. Status

- Starting HEAD: `b09e1f99be5767bc3dcd2ffeb186153a5e5782fa`.
- Final HEAD: the documentation commit containing this handoff.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree: clean at final handoff.
- Production changes: user-prefix install/uninstall, executable-aware helper
  discovery and bounded Doctor distinctions.
- P0: none.
- P1: none.
- Overall: **0.7B COMPLETE / VALIDATED**.

## 2. Commits

- `d9abea5` — `feat: complete user-prefix installation and diagnostics` — installer,
  exact-file uninstaller, shared helper lookup, Doctor additions and bounded
  Linux harness.
- `docs: validate v0.7 installation and diagnostics` — user contract,
  troubleshooting, Roadmap state and this evidence record.

## 3. Existing installation architecture

The existing archive already used `bin/gui2tui` plus
`libexec/gui2tui/{gui2tui-inspect,gui2tui-local,headless-session}`. The main
binary searched installed libexec, Cargo siblings and, for developer builds,
the checkout `scripts` directory. A correctly copied prefix therefore worked,
but lookup checked only file presence, Doctor did not inspect it, and the
documented install/remove process was manual. There was no ownership marker or
safe uninstall operation.

The archive has no required runtime data bundle. `gui2tui-local` is an optional
same-host External Modality endpoint; it is not a semantic backend or Remote
Companion. The Managed helper already checks its own optional setup tools and
prints the Ubuntu dependency command without installing anything.

## 4. Final installation contract

After `cargo build --release --locked --bins`, run:

```bash
./scripts/install-user.sh --prefix "$HOME/.local"
```

An extracted compatible bundle uses its root `install-user.sh`. The default is
`$HOME/.local`; another absolute current-user-owned prefix, including a path
with spaces, is accepted. Root/sudo, symlink target directories and every
existing managed target are refused.

The installer writes the main binary, three existing helpers, the uninstaller
and a private mode-0600 exact-file hash manifest. No system directory, shell
profile, XDG configuration or runtime state is modified.

## 5. Helper discovery

The existing lookup is now shared through `product::paths` and requires an
executable regular file. Installed components resolve relative to the running
main executable; developer builds retain sibling binaries and the bounded
source-script fallback. Installed CLI, Inspector, Doctor, endpoint and Managed
setup were exercised from `/` and `/tmp` with no checkout or Cargo target in
their runtime lookup.

## 6. First-run behavior

Installation creates no config and selects no application. The user runs
Doctor with `--session desktop` or `--session managed`, connects to that one
session and then chooses an application from its current registry. Omitting
the flag retains the 0.7A compatible descriptor behavior. Installation never
saves an application name, PID, display or locator as future authority.

## 7. Safe uninstall

The installed `libexec/gui2tui/uninstall-user` verifies its header, ownership,
mode, single-link manifest and the hash of every existing managed file before
removing only the five fixed installed executable/script paths and manifest.
It refuses a changed file, a managed-path symlink or any existing Managed
descriptor. It does not recursively delete the prefix, follow a symlink, kill
processes, or remove configuration, runtime data, private recovery candidates
or unrelated files. Users stop an owned Managed session explicitly through
`gui2tui setup stop` first.

## 8. Doctor architecture

Doctor still uses the existing `Check`/`Report`, PASS/WARN/FAIL/INFO levels,
JSON schema, private report writer and bounded probes. No diagnostic engine or
state database was added. New checks cover the running entry, helpers,
user-install marker/uninstaller, terminal basics and configured external
handler. Existing D-Bus/AT-SPI probing was split into explicit connection
layers.

## 9. Installation diagnostics

- Running entry: executable-file check.
- Required private inspector: FAIL when missing/non-executable.
- Managed helper: PASS when usable; FAIL for selected Managed topology when
  absent, otherwise a feature-local WARN.
- Optional same-host modality helper: WARN when absent; core semantic use
  remains available.
- Managed user-prefix marker/uninstaller: PASS, INFO for developer/manual
  layouts, or FAIL for unsafe/incomplete managed installation.

Live evidence changed the installed inspector to non-executable and observed a
blocking helper-specific FAIL, then restored it. No source path appeared in
the installed invocation.

## 10. Session / Accessibility diagnostics

Doctor now separates selected-session failure, session D-Bus, `org.a11y.Bus`
address, AT-SPI registry/enumeration, zero applications and application
semantic sufficiency. Zero applications is WARN after a registry PASS.
Application presence is PASS only for visibility/count; semantic sufficiency
is explicitly NOT CHECKED without traversal or mutation. Session and bus
failures do not become application-not-found claims.

## 11. Terminal diagnostics

Doctor checks stdin+stdout TTY/PTY attachment, nonempty/non-dumb `TERM`, a
declared UTF-8 locale and nonzero readable terminal dimensions. The responsive
renderer has no fixed global size cutoff, so the report does not invent one.
Raw mode, alternate screen and enhanced keyboard reporting are explicitly NOT
CHECKED because Doctor does not alter terminal state. Terminal-emulator and SSH
client qualification remain 0.7C work.

## 12. External handler diagnostics

A missing complex-text handler is valid INFO. Existing config parsing retains
the exact standalone `{file}` and shell-free argv rules. A configured program
is resolved through the existing executable validation: executable is PASS;
missing/non-executable is a feature-local WARN. Doctor never starts the editor,
creates a GUI target or writes a candidate. Live sentinel evidence confirmed
that an executable handler was not invoked.

## 13. Privacy and security

Doctor retains contents-free reporting and does not emit bus addresses,
descriptor bodies, GUI/application text, app names, passwords, candidate data,
payloads or credentials. Installation uses no root and overwrites no target.
The uninstall manifest is current-user mode 0600 and single-link; uninstall
uses fixed paths plus hashes and fails closed. Existing 0700/0600 XDG,
PasswordText and external-candidate inode/permission/size/lifetime rules are
unchanged.

## 14. Linux installation evidence

Ubuntu 24.04 arm64 OrbStack guest, UID 501, controlled Xvfb/session-D-Bus/
AT-SPI and a temporary prefix containing a space:

- fresh user-prefix install without root: PASS;
- installed layout and mode-0600 manifest: PASS;
- arbitrary working directory and all helper discovery: PASS;
- Desktop Doctor with zero/current applications: PASS;
- Managed Headless setup and Doctor: PASS;
- unavailable selected session bus diagnosis: PASS;
- AT-SPI address failure: NOT TESTED live;
- controlled interactive PTY terminal preflight: PASS;
- handler absent/executable/unavailable/not-executed: PASS;
- fresh application discovery and one semantic operation/readback: PASS;
- active Managed descriptor uninstall refusal: PASS;
- malicious managed-path symlink refusal: PASS;
- exact uninstall, unrelated-file/config preservation and process cleanup:
  PASS;
- real ordinary local X11 desktop: NOT TESTED.

## 15. Existing capability regression

The installed inspector enumerated the selected Desktop registry, selected a
fresh fixture application, invoked one advertised Action and confirmed the
result through fresh Accessibility readback. Managed selection, zero-app
distinction and explicit stop/cleanup also ran. Broader v0.4-v0.6 campaigns,
real local X11, SSH, Wayland and package ABI matrices were not repeated.

## 16. Genericity and forbidden audit

No application/toolkit branch, private API, DOM/CDP, UNO, OCR/vision, input
injection, coordinate operation, fuzzy identity, permission migration or
filesystem semantic bypass was added. No DeploymentManager,
EnvironmentRegistry, SessionManager, daemon, updater, package manager, Remote
Companion or new runtime identity exists.

## 17. Tests and quality

- Targeted product/config/path/Doctor unit tests: PASS.
- User CLI regression: PASS.
- Installer/uninstaller Bash syntax and live exact-file behavior: PASS.
- Ubuntu 24.04 arm64 controlled installation/diagnostic harness: PASS, with
  named NOT TESTED rows above.
- `cargo fmt --all -- --check`: PASS.
- `cargo check --all-targets --locked`: PASS.
- `cargo test --all-targets --locked`: PASS — 299 library, 2 inspector CLI and
  5 user CLI tests; binary targets with no tests also completed.
- `cargo clippy --all-targets --locked -- -D warnings`: PASS.
- `python3 scripts/check-docs.py`: PASS — 85 files / 283 local links.
- `git diff --check`: PASS.

## 18. Remaining issues

- 0.7B P0: none.
- 0.7B P1: none.
- Known P2: Doctor intentionally cannot prove raw/alternate-screen/enhanced-key
  behavior without side effects; it reports NOT CHECKED. A live isolated
  `org.a11y.Bus`-missing case was not constructed.
- Environment limitations: controlled Xvfb is not real local X11; no SSH PTY,
  Wayland/XWayland or x86_64 live run occurred.
- 0.7C–0.7E remain separately planned and unauthorized.

## 19. Architecture conclusion

- Existing installation/CLI/Doctor architecture sufficient: **YES**.
- DeploymentManager required: **NO**.
- Repeatable no-root installation implemented: **YES**.
- Deployment failure versus application semantic limitation distinguished:
  **YES**, with semantic sufficiency honestly NOT CHECKED by Doctor.
- 0.7A session contract and v0.6 runtime authority preserved: **YES**.

## 20. v0.7 roadmap

- Discovery: **COMPLETE**.
- 0.7A: **COMPLETE / VALIDATED**.
- 0.7B: **COMPLETE / VALIDATED**.
- 0.7C: **PLANNED / NOT AUTHORIZED**.
- 0.7D: **PLANNED / NOT AUTHORIZED**.
- 0.7E: **PLANNED / NOT AUTHORIZED**.

## 21. Git status

- Branch: `v0.7/deployment-environment-completeness`.
- HEAD: the documentation commit containing this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`.
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, `v0.3.0`.
- v0.7.0 tag/release: absent.

## 22. Next recommended direction

Recommend **0.7C — X11, Headless and Same-host SSH Qualification**. It is not
authorized. Wait for explicit user review.

## 23. Next Codex context

新会话先读 `AGENTS.md`，再依次读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.6 milestone HANDOFF、v0.7 Discovery、
`docs/planning/v0.7-roadmap.md`、0.7A HANDOFF 和本 HANDOFF。当前分支是
`v0.7/deployment-environment-completeness`；准确 HEAD 以本 HANDOFF 所在提交为
准。v0.4–v0.6 已完成内部 Milestone Qualification；0.7A 与 0.7B 已完成验证。
当前安装合同是 current-source `cargo build --release --locked --bins` 后运行
`scripts/install-user.sh --prefix ABSOLUTE_PATH`，或在兼容 bundle 根运行相同
installer；无需 root，固定 `bin`/`libexec` 布局及私有哈希 manifest。安装后的
`uninstall-user` 只移除经 manifest 验证的 GUI2TUI 文件，拒绝改动、symlink 与
仍存在的 Managed descriptor，保留配置、runtime/recovery 数据及无关文件。
Doctor 已验证入口/helper、handler executable、TTY/TERM/UTF-8/size、session
D-Bus、Accessibility bus、registry 与零应用区分；raw/alternate/enhanced keys 和
应用语义充分性明确 NOT CHECKED，不执行 handler 或 GUI mutation。
`--session desktop` 永不读 Managed descriptor；`--session managed` 只用现有
有效 descriptor 且不创建/回退；默认兼容策略不变。Session 只决定 transport，
fresh registry selection、generation、exact locator、scope/capability/ticket 和
authoritative readback 仍决定应用权限。Local X11 与 Managed Xvfb 是核心目标，
Wayland/XWayland 分别优先验证，same-host SSH TUI 争取纳入，Remote Companion
延期到 1.0 后。真实本地 X11、SSH、Wayland/XWayland 和当前源码双架构 package/
ABI 仍未资格验证。0.7C–0.7E 未授权；v0.7 仍为内部里程碑，package version 为
0.3.0，v1.0.0 仍是下一计划公开版本。不得自行扩展任务。
