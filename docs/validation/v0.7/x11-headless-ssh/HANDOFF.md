# GUI2TUI v0.7C X11, Headless and Same-host SSH Qualification Handoff

## 1. Status

- Starting HEAD: `f072dce4c22ffc88393a3094f5d05b5c6744cdf4`.
- Final HEAD: the documentation/evidence commit containing this handoff.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree: clean at final handoff.
- Production changes: none.
- Validation changes: one bounded installed-Managed lifecycle harness.
- P0: none.
- P1: none observed in the available environment.
- Overall: **PARTIALLY VALIDATED / ENVIRONMENT EVIDENCE PENDING**. Managed
  Xvfb is qualified by current-source installed-binary evidence. A real local
  X11 desktop and a real SSH client/server PTY were unavailable and are not
  qualified.

## 2. Environment inventory

- Host: macOS 26.6.2 arm64; build/control host only, not a Linux GUI support
  environment.
- Linux guest: Ubuntu 24.04 aarch64 under OrbStack, unprivileged UID 501.
- Managed dependencies: Xvfb, session D-Bus, AT-SPI, GTK 3/4, Python 3,
  `pexpect`, `pyte` and a normal local PTY were available.
- Real local X11: unavailable. The guest reported a `tty` session, had no
  Xorg/display-manager/desktop-session process and no local X11 socket. A host
  XQuartz value inherited into the guest was not treated as a local Linux
  desktop.
- SSH: the client existed, but `sshd` was absent, the `ssh` unit was absent or
  inactive and no SSH listener was present. No SSH service or authentication
  policy was installed or changed.
- Editor/handler: `vim`/`vi` and existing controlled handler fixtures were
  available. Only the Managed local-PTY handler path was exercised; it is not
  SSH evidence.
- Isolation: private randomized HOME/XDG directories and a randomized
  mode-0700 `/tmp/gui2tui-v07c.*/user prefix` were used.

## 3. Commits

- Production: none; no reproduced product blocker required a fix.
- Validation/docs: `docs: qualify available v0.7 X11 headless SSH environments`
  (the commit containing the bounded harness, roadmap updates and this
  handoff).

## 4. Real local X11 qualification

**NOT TESTED.** No running normal Linux X11 desktop session existed. The
controlled Desktop side of the isolation run was Xvfb and is labelled as such.
It proves explicit session isolation only; it does not prove local desktop
login/session discovery, a desktop environment, display manager, inherited
desktop D-Bus, or terminal behavior in such a session.

Still required in a user-supplied or otherwise approved Ubuntu X11 desktop:
installed `--session desktop`, Doctor, current registry enumeration, explicit
application selection, scene/layout, one verified operation, dynamic refresh,
same-name cross-session isolation, normal TUI exit and terminal restoration.

## 5. Managed Headless qualification

**QUALIFIED** on the recorded Ubuntu 24.04 aarch64 topology.

- Installed a current-source release build into a private user prefix with a
  space; invoked it from `/tmp`, outside the checkout and Cargo target.
- Created a persistent Managed session through the installed helper, verified
  its private descriptor, selected it explicitly and passed Doctor's session
  D-Bus, Accessibility-bus and registry checks.
- Started an existing GTK4 fixture in the selected session, enumerated it and
  explicitly selected it with installed `--app` from the current registry.
- Used the installed TUI Command Palette to invoke `Reorder current items`,
  observed `Structure: reordered; selected Gamma` through a fresh inspector
  read, then invoked `Reset current items` from the refreshed scene and read
  back `Selected: Alpha`.
- Exited normally and verified canonical input and echo were restored.
- Started a second installed process and selected the application afresh while
  the Managed session remained alive.
- Stopped the session while that view existed; the old view did not redirect
  its operation to the same-named application in the controlled Desktop
  registry.
- Verified descriptor removal, supervisor and recorded Xvfb/session-D-Bus
  child exit, and exact fixture cleanup.
- Recreated the Managed session, enumerated a fresh fixture, performed and
  authoritatively read back another operation, then stopped it cleanly.
- Reused the existing qualified complex-text fixture and direct-argv handler
  with the installed binary for one terminal handoff, public AT-SPI writeback
  and authoritative readback.
- Uninstalled through the exact-file manifest; an unrelated prefix file
  remained.

## 6. Desktop/Managed isolation

**PASS under two controlled Xvfb Accessibility universes.** Explicit Desktop
enumerated its unique fixture and one same-named GTK fixture. Explicit Managed
enumerated exactly its own same-named GTK fixture and never saw the Desktop-
unique fixture. Managed mutation left the Desktop copy at `Selected: Alpha`.
Stopping Managed did not cause its existing view to acquire or mutate the
Desktop copy. A new Managed session required fresh enumeration and selection.

This is session-isolation evidence, not real local-X11 qualification.

## 7. Same-host SSH qualification

- SSH -> Existing Desktop: **NOT TESTED**. Neither a real local Desktop session
  nor an SSH server/PTY route was available.
- SSH -> Managed Headless: **NOT TESTED**. Managed itself passed, but there was
  no real SSH server/client connection with an interactive PTY; a local PTY
  was not relabelled as SSH.

No `SSH_CONNECTION`, `SSH_TTY` or similar variable simulation was used. No
other user's bus address or credentials were copied.

## 8. SSH PTY and terminal

**NOT TESTED for SSH.** The client/server/interactive-PTY prerequisite was
absent. Therefore Tab, Shift-Tab, arrows, Enter, F6/Shift-F6,
Ctrl-Tab/Ctrl-Shift-Tab, enhanced keyboard reporting and SSH shell restoration
have no 0.7C SSH result.

For the available local Managed PTY, explicit `--app`, Command Palette input,
Enter execution, normal `q` exit, canonical mode and echo restoration passed.
This is retained only as Managed terminal evidence.

## 9. SSH external handler

**NOT TESTED.** No SSH PTY existed, so neither `vim`/`vi` nor any controlled
handler result is claimed as SSH editor handoff evidence. The Managed local-PTY
positive handler path passed separately; historical v0.6 terminal ownership
evidence was not relabelled as SSH qualification.

## 10. SSH disconnect behavior

- Normal GUI2TUI exit through SSH: **NOT TESTED**.
- Normal SSH session end: **NOT TESTED**.
- SSH SIGINT/SIGTERM: **NOT TESTED**.
- Abrupt SSH transport loss: **NOT TESTED**.

No guarantee is added for process destruction that prevents userspace cleanup,
and no remote process supervisor or reconnect mechanism was introduced.

## 11. Installation evidence

The live runs used `/tmp/gui2tui-v07c-target/release` only as the source of an
extracted-layout payload. `install-user.sh` installed that payload into a
randomized private prefix containing a space. All Doctor, inspector, TUI,
Managed helper, handler and uninstall invocations used the installed paths and
ran from ordinary directories outside the checkout. Version output was
`gui2tui 0.3.0`; no root access was used.

The existing 0.7B harness was also rerun against the same release build and
again passed installation, helper lookup, Desktop/Managed Doctor, semantic
operation, controlled PTY, fail-closed uninstall cases and safe removal.

## 12. Session / application authority

Session choice selected only one transport. Both initial and reconnected TUI
processes enumerated the Managed registry and resolved the explicit current
`--app` choice afresh. Descriptor, display, PID and application name did not
transfer a binding to another session. Stopping and recreating Managed retired
all prior application state; the recreated operation used a new process,
registry enumeration, cache and exact current locator.

The existing `RuntimeSessionId` + `ApplicationGenerationId` + exact
`BackendLocator` + current scope/capability/ticket/readback contract was not
changed.

## 13. Dynamic semantic continuation

The installed TUI invoked a structural reorder. A fresh public Accessibility
read observed the reordered state and selected item. The same TUI then opened
the ordinary current Command Palette and invoked Reset from the refreshed
scene; a fresh read observed the initial selection again. No event, method
return, delay or optimistic terminal state was treated as success.

## 14. Existing capability regression

- Explicit Desktop/Managed selection and Doctor topology: PASS in controlled
  Xvfb sessions.
- Current registry enumeration and unique current `--app` selection: PASS.
- Existing Action plus authoritative readback: PASS before and after Managed
  recreation.
- Dynamic fresh-scene continuation: PASS.
- Complex text positive handler/writeback/readback: PASS under the Managed
  local PTY.
- Normal terminal exit/restoration: PASS under the Managed local PTY.
- Safe user-prefix uninstall and unrelated-file preservation: PASS.

The broader v0.4-v0.6 campaigns, broad application matrix, real local X11,
SSH clients, Wayland/XWayland and package ABI were not rerun.

## 15. Genericity and forbidden audit

No production code changed. No application/toolkit branch, private API,
DOM/CDP, UNO, OCR/vision, GUI input injection, coordinate authority, fuzzy
identity, backing-file bypass or anonymous action was added. No SessionManager,
EnvironmentRegistry, DeploymentManager, ProcessSupervisor, remote protocol,
Remote Companion or runtime identity was introduced. The validation fixtures
are evidence inputs only.

## 16. Tests and quality

- `cargo build --release --locked --bins` in Ubuntu 24.04 aarch64: PASS.
- Existing `tests/live/v07b_install_diagnostics.sh` against that build: PASS,
  including its named `AT-SPI bus failure=NOT_TESTED` and
  `real local X11=NOT_TESTED` rows.
- New `tests/live/v07c_managed_qualification.sh`: PASS for every Managed and
  controlled-isolation row; real X11 and all SSH rows explicitly NOT TESTED.
- Harness Bash syntax: PASS.
- Documentation audit: PASS at final handoff.
- `git diff --check`: PASS at final handoff.

No Rust production source changed, so the full Rust test/clippy matrix was not
repeated. The final source-quality matrix from 0.7B remains the current source
baseline; this phase performed only its authorized environment evidence.

## 17. Remaining issues

- Product P0: none observed.
- Product P1: none observed in the available Managed environment.
- P2: enhanced-key combinations were not separately classified in the local
  PTY because the selected task was reachable through the existing Command
  Palette; this is not SSH keyboard evidence.
- Environment evidence gap: real local Linux X11 is required before that core
  target can be qualified.
- Environment evidence gap: a safely available same-host SSH server/client
  PTY plus an appropriate Desktop and/or Managed session is required before
  either SSH row can be qualified.
- 0.7D remains the planned native Wayland/XWayland evidence phase.
- 0.7E remains the planned package/ABI and integrated environment-contract
  phase. Neither is authorized by this handoff.

## 18. Environment qualification matrix

| Environment | Qualification | Actual evidence | Missing evidence |
| --- | --- | --- | --- |
| Real local X11 | **NOT TESTED** | Environment inventory only; controlled Xvfb explicitly excluded | Normal Linux X11 desktop install-to-operation-to-exit run |
| Managed Xvfb | **QUALIFIED** on Ubuntu 24.04 aarch64 | Installed binary, create/reuse/stop/recreate, isolation, fresh selection, operation/readback, dynamic continuation, handler, PTY exit, cleanup/uninstall | 0.7E package/architecture integration remains separate |
| SSH -> Desktop | **NOT TESTED** | No SSH server or real Desktop session | Real SSH PTY into same-user existing Desktop plus terminal/handler/disconnect evidence |
| SSH -> Managed | **NOT TESTED** | Managed qualified locally; SSH prerequisite absent | Real SSH client/server PTY running installed binary against Managed, including exit and handler lifecycle |

## 19. Architecture conclusion

- Current AT-SPI/session/runtime architecture sufficient for the environment
  actually verified: **YES**.
- New environment manager required: **NO**.
- Real terminal compatibility defect found: **NO** in the available Managed
  local PTY; SSH terminal compatibility remains unknown.
- Environment qualified by 0.7C: **Managed Xvfb on the recorded Ubuntu 24.04
  aarch64 topology**.
- Environments still lacking necessary evidence: **real local X11, SSH ->
  Desktop and SSH -> Managed**.

## 20. v0.7 roadmap

- Discovery: **COMPLETE**.
- 0.7A: **COMPLETE / VALIDATED**.
- 0.7B: **COMPLETE / VALIDATED**.
- 0.7C: **PARTIALLY VALIDATED / ENVIRONMENT EVIDENCE PENDING**.
- 0.7D: **PLANNED / NOT AUTHORIZED**.
- 0.7E: **PLANNED / NOT AUTHORIZED**.

0.7C cannot be marked fully qualified until the missing real-X11 and SSH
evidence is supplied and reviewed.

## 21. Git status

- Branch: `v0.7/deployment-environment-completeness`.
- HEAD: the documentation/evidence commit containing this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, `v0.3.0`, unchanged.
- v0.7.0 tag/release: absent.

## 22. Next recommended direction

First supply an approved real local Ubuntu X11 desktop and a safe same-host SSH
client/server PTY environment, then complete the missing 0.7C rows. Do not mark
0.7C fully closed and do not start 0.7D automatically.

## 23. Next Codex context

新会话先读 `AGENTS.md`，再依次读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、v0.6 milestone HANDOFF、v0.7 Discovery、
`docs/planning/v0.7-roadmap.md`、0.7A HANDOFF、0.7B HANDOFF 和本 HANDOFF。
当前分支是 `v0.7/deployment-environment-completeness`，准确 HEAD 是包含本
HANDOFF 的 `docs: qualify available v0.7 X11 headless SSH environments` 提交，
工作树和远端应保持同步。v0.4-v0.6 已完成内部 Milestone Qualification；0.7A
和 0.7B 已完成。0.7C 当前为 PARTIALLY VALIDATED / ENVIRONMENT EVIDENCE
PENDING：Ubuntu 24.04 aarch64 Managed Xvfb 已用真实 user-prefix installed
binary 验证安装、create/reuse/stop/recreate、Doctor、双 registry 隔离、fresh
`--app` 选择、动态操作、authoritative readback、Command Palette、handler
handoff、终端恢复、精确清理和安全卸载；未修改生产代码。真实本地 Linux X11
没有环境，必须保持 NOT TESTED；客体没有 sshd/listener，所以 SSH -> Desktop、
SSH -> Managed、SSH handler 与断开行为均为 NOT TESTED，不能用本地 PTY 或环境
变量替代。安装合同仍是 current-source 或兼容 bundle 通过
`install-user.sh --prefix ABSOLUTE_PATH` 无 root 安装，manifest uninstaller
fail closed；Doctor 的安装、终端、session D-Bus、Accessibility bus、registry、
zero-app 与应用语义边界不变。Desktop 永不读 Managed descriptor；Managed 只用
现有有效 descriptor 且不创建/回退。Session 只选择 transport；应用权限仍要求
fresh registry enumeration、明确 current selection、fresh generation、exact
locator、current scope/capability/ticket 和 authoritative readback。PasswordText、
private artifact 0700/0600、external work 不延长权限、terminal single owner/reader
等不变量不变。既定路线仍是 Local X11 与 Managed Xvfb 为核心目标，
Wayland/XWayland 后续分别验证，same-host SSH 争取纳入，Remote Companion 延期到
1.0 后。下一步应先补真实 X11 与 SSH 环境证据；0.7D/0.7E 未授权，v0.7 仍为
内部里程碑，package version 仍为 0.3.0，v1.0.0 仍是下一计划公开版本。不得
自行扩展任务。

## 24. Post-handoff Headless-first contract revision

This section was appended after the recorded 0.7C validation. It changes
future product priority and exit criteria only; sections 1-23 remain the
historical evidence record.

- Managed Xvfb remains **QUALIFIED on Ubuntu 24.04 aarch64** on the exact
  recorded topology. No rerun or broader claim is implied.
- Real local X11, SSH -> existing Desktop and SSH -> Managed remain **NOT
  TESTED**. The local PTY and controlled Xvfb evidence were not relabelled.
- GUI2TUI is now Headless-first. Real SSH -> Managed is the highest-priority
  0.7C evidence gap, and Local Linux TTY -> Managed requires a real virtual-
  console run if that exact topology will be Supported.
- Ordinary local X11 and SSH -> existing Desktop are optional compatibility
  rows rather than Headless-core 0.7C exit gates. Their NOT TESTED status is
  unchanged.
- Docker/OCI Headless is an important but NOT TESTED candidate requiring a
  separately authorized bounded qualification before the 0.7E matrix is
  frozen if it remains in the proposed 1.0 contract.
- Native Wayland, XWayland and Headless Wayland remain separately NOT TESTED
  candidates for the planned, unauthorized 0.7D phase.

The authoritative revised classification and exits are in the
[Headless-first contract](../../../planning/v0.7-headless-first-contract.md)
and [v0.7 roadmap](../../../planning/v0.7-roadmap.md). This documentation-only
revision did not complete 0.7C or run new environment qualification.

## 25. Subsequent Headless qualification evidence

On 2026-09-20 a separately authorized supplement obtained the real Docker and
SSH evidence that was unavailable during this historical run. It does not
change any original `NOT TESTED` statement above: those statements accurately
describe the earlier environment. The new evidence and revised current 0.7C
status are recorded in the
[Headless-first qualification handoff](../headless-first-qualification/HANDOFF.md).
