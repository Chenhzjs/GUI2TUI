# GUI2TUI v0.7C Headless-first Environment Qualification Handoff

## 1. Status

- Starting HEAD: `d0686e44c2e265ede257009301910e2ad77df63b`.
- Validated source/helper/harness commit:
  `bd43199d737a61c67f33a8087ccf100b33e576ed`.
- Final HEAD: the documentation commit containing this handoff.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree: clean at final handoff.
- Production changes: one bounded Managed Headless helper recovery fix; no Rust
  semantic, runtime, interaction or terminal production code changed.
- P0: none.
- P1: none.
- Overall: **0.7C COMPLETE / VALIDATED WITH EXPLICIT LIMITATIONS** under the
  approved Headless-first exit contract.

This does not qualify v0.7 as a milestone, authorize 0.7D/0.7E or make a 1.0
release claim.

## 2. Headless-first contract basis

The normative classification is the
[Headless-first contract](../../../planning/v0.7-headless-first-contract.md).
Managed Xvfb remains the concrete core target. Same-host SSH -> Managed is a
core candidate. Docker/OCI is an independently qualified important candidate.
Linux native VT is a separate topology. Ordinary local X11 and SSH -> existing
Desktop are optional compatibility rows, not Headless-core gates.

Session selection chooses transport only. Application authority still requires
fresh registry enumeration, explicit selection, a fresh
`ApplicationGenerationId`, exact `BackendLocator`, current scope/capability and
operation ticket, followed by authoritative public Accessibility readback.

## 3. Environment inventory

- Host: macOS 26.6.2 arm64, used only as the terminal and Docker host.
- Container engine: Docker client/server 29.4.0 under OrbStack.
- Container: Ubuntu 24.04 arm64, glibc 2.39.
- Build toolchain: Rust/Cargo 1.88.0; locked current-source release binaries.
- SSH: host OpenSSH 10.2p1 client to Ubuntu OpenSSH 9.6p1 server.
- Product user: unprivileged `gui2tui` user, UID 1001.
- Install prefix: `/home/gui2tui/.local` inside the disposable container.
- Graphical runtime: Managed Xvfb, private session D-Bus and AT-SPI registry;
  no desktop environment, VNC/RDP, host GUI, host X11 socket or graphics device.
- Controlled applications: existing GTK selection and GTK live fixtures.
- Native Linux virtual-console TTY: unavailable, **NOT TESTED**.
- Ordinary local X11 desktop: unavailable, **NOT TESTED** optional
  compatibility.
- Native Wayland/XWayland: outside this task, **NOT TESTED**.

## 4. Reproducible validation topology

The committed validation slice consists of:

- `tests/live/Dockerfile.v07c-headless`;
- a constrained sshd configuration and disposable-key entrypoint;
- an exact-process container control helper;
- one standard-library PTY driver; and
- `tests/live/v07c_headless_qualification.sh` as the single orchestrator.

The container was not privileged, did not share the host PID namespace and
mounted only a disposable public key read-only. Port 22 was published to an
ephemeral host port explicitly bound to `127.0.0.1`. Password, keyboard-
interactive, root, agent forwarding, TCP forwarding, tunnels and X11
forwarding were disabled. No host private key, personal file, D-Bus credential,
GUI socket or device was mounted.

The Dockerfile is live-test infrastructure, not a production image, installer
or generalized OCI support claim.

## 5. Installation, setup and Doctor

The current source built release binaries in the container and installed the
complete user-prefix layout through the existing 0.7B installer. Tests invoked
the installed program and helpers, not Cargo target binaries. Product
processes, fixtures, Xvfb, session D-Bus and AT-SPI ran as the non-root user.

Docker and real SSH paths both ran structured Doctor checks. Installation
entry, Inspector helper, Managed helper, session D-Bus, Accessibility bus,
registry, accessible applications and configured direct-argv handler were all
`PASS`. Doctor did not start the editor or read application content.

## 6. Docker Headless interactive-TTY result

**QUALIFIED WITH EXPLICIT LIMITATIONS** on the exact recorded topology.

Through a real `docker exec -it` PTY the installed TUI:

1. explicitly selected the Managed session;
2. enumerated the current AT-SPI registry;
3. explicitly selected the controlled application;
4. rendered the current spatial semantic scene;
5. received Tab, Shift+Tab, Left, Right, Enter, F6, Shift+F6, Ctrl+Tab and
   Ctrl+Shift+Tab sequences with observable terminal responses;
6. used the Command Palette to reorder and then reset a structured selection;
7. confirmed both changes through fresh Inspector/AT-SPI reads;
8. continued from the refreshed scene; and
9. exited with canonical input and echo restored.

The result does not qualify another container engine, distribution,
architecture, target application or production image.

## 7. Real same-host SSH PTY result

**QUALIFIED** on the recorded OpenSSH topology.

The host OpenSSH client connected to the loopback-only container sshd with a
real interactive PTY. The remote process, GUI2TUI, Managed session and GUI
applications were all in the same Linux container and user context. No SSH
environment-variable simulation, X11 forwarding, VNC/RDP or cross-host
semantic backend was used.

The SSH TUI passed application enumeration and explicit selection, semantic
scene rendering, base navigation, Command Palette use, verified reorder/reset,
fresh authoritative readback, dynamic continuation, normal `q` exit, SSH shell
exit and terminal canonical/echo restoration. F6, Shift+F6, Ctrl+Tab and
Ctrl+Shift+Tab were actually delivered and produced observable TUI output in
this client path; this is not a claim for all clients or enhanced-key modes.

## 8. SSH external-handler lifecycle

**QUALIFIED** on the recorded topology using Vim through the existing generic
direct-argv configuration. No editor-specific production branch was added.

The TUI selected a current complete, bounded, non-password multiline target,
released the real SSH PTY, and Vim edited only the GUI2TUI-owned private
candidate. Vim exited normally, the TUI reacquired the terminal and its reader,
the current generation/locator/scope/ticket/conflict gates ran, public
EditableText performed the write, and a fresh complete AT-SPI read confirmed
`SSH qualified edit`. The in-memory fixture had no backing document for the
handler to modify. The successful path left zero candidate artifacts.

Default Vim atomic replacement was initially rejected by the existing inode
safety gate, as designed. The validation configuration used Vim's ordinary
in-place write mode so the original 0600 candidate inode remained authoritative;
production policy was not weakened.

## 9. SSH disconnect and recovery boundary

The harness separately destroyed the client transport while the TUI owned the
PTY and while Vim owned it. In both cases the remote terminal-owned `gui2tui`
and `vim` processes exited within the bound; the independently Managed Xvfb,
session D-Bus and controlled GUI application remained alive. The handler-
disconnect path left zero private artifact namespaces/files; the metadata-only
audit would have rejected a foreign owner, symlink, non-regular file, extra
hard link or group/other permissions.

A new SSH connection could start a new TUI, enumerate the current registry,
explicitly select the application and complete a new verified operation. No
old TUI process, application generation, locator, binding or operation ticket
was migrated. GUI2TUI does not promise to restore a destroyed remote PTY,
transparently reconnect the old process or replay a non-idempotent operation.

Normal TUI exit and normal SSH shell exit passed. The abrupt case used a
controlled client-process `SIGKILL` to model transport disappearance; it is not
reported as an ordinary GUI2TUI SIGINT/SIGTERM path. Existing v0.6 signal
evidence was not rerun or relabelled as SSH evidence.

## 10. Container restart and fresh authority

After `docker restart`, the engine assigned a new loopback port. The harness
rediscovered that exact mapping and refreshed only the disposable known-hosts
entry. The old Managed descriptor was rejected, the old fixture was absent and
no old application authority was usable. A newly created Managed session,
fresh fixture registration, Doctor, explicit application selection and a new
verified semantic workflow all passed.

One real Managed lifecycle defect was found: the persistent state directory
could retain the prior display-number file across an abrupt container stop,
allowing the asynchronous Xvfb readiness loop to observe stale data before the
child redirection truncated it. Its early cleanup also referenced function-
local PID variables after their scope had ended. The minimal helper fix removes
stale startup files before launch/stop and makes early cleanup safe before
D-Bus initialization. The complete restart path passed afterward.

## 11. Local Linux TTY

**NOT TESTED.** Docker `exec -it`, a local PTY and SSH PTY are not Linux
virtual consoles. The macOS/OrbStack environment could not provide a real
Linux VT without expanding host/VM scope. No desktop, display manager, boot
configuration, privileged container or host service was installed for this
row.

The existing installer, Managed helper, Doctor, TUI and terminal restoration
are reusable, but Local Linux VT must remain unclaimed until a separately
authorized real-console run supplies evidence.

## 12. Qualification matrix

| Environment | Result | Evidence boundary |
| --- | --- | --- |
| Existing Managed Xvfb on Ubuntu 24.04 arm64 | **QUALIFIED** | Retained original installed lifecycle evidence |
| Docker Headless + interactive TTY | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Unprivileged Ubuntu 24.04 arm64, Docker 29.4, current-source installed binary |
| Docker Headless + real SSH PTY | **QUALIFIED** | Loopback OpenSSH client/server, same container/user/Managed session |
| SSH external handler lifecycle | **QUALIFIED** | Real Vim PTY ownership, public AT-SPI writeback and full readback |
| SSH disconnect and recovery | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Terminal-owned processes exit; Managed/app persist; reconnect is fresh, not transparent |
| Linux native VT/TTY | **NOT TESTED** | No real Linux virtual console available |
| Ordinary local X11 Desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | No real desktop; controlled Xvfb is not substituted |
| SSH -> existing Desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | No real desktop session in scope |
| Native Wayland / XWayland | **NOT TESTED / OUT OF SCOPE** | Planned 0.7D only if authorized |

## 13. Installation, uninstall and cleanup

The isolated prefix installed without root and resolved all helpers without a
checkout or target-directory dependency. Final stop removed the active
descriptor and exact owned fixture/session processes. The exact-file
uninstaller removed installed GUI2TUI files while retaining the user
configuration. No unrelated prefix, system configuration or host service was
modified.

The orchestrator removed its exact container, image, ephemeral loopback port,
scratch directory and disposable key on success and failure. A post-run label
query found no remaining validation container or image. Pre-existing unrelated
Docker containers were not touched.

## 14. Authority, privacy and genericity audit

- Session choice never selected or authorized an application.
- Every new process/session used current registry enumeration and explicit
  application selection; restart/reconnect did not revive stale authority.
- Operations used current scope/capability/ticket and fresh authoritative
  public AT-SPI readback.
- No name, PID, display number, geometry or historical locator granted
  authority; fixture names existed only in the validation harness.
- No PasswordText, application text, bus address, candidate payload, token,
  private key or descriptor content was retained in committed evidence.
- The artifact audit inspected metadata only. 0700/0600, owner, regular-file,
  no-symlink and single-link boundaries remained intact.
- No DOM/CDP, UNO, private toolkit API, OCR, screenshot semantics, GUI input
  injection, coordinate click, fuzzy matching or backing-file mutation was
  added.
- No SessionManager, DeploymentManager, Remote Companion, runtime identity or
  container-specific semantic backend was added.

## 15. Tests and quality

Live validation:

- `RESULT_DIR=/tmp/gui2tui-v07c-headless-bd43199
  tests/live/v07c_headless_qualification.sh`: **PASS**.
- Embedded source identity:
  `bd43199d737a61c67f33a8087ccf100b33e576ed`.
- Script syntax and Python bytecode compilation: **PASS**.

Phase-close source quality:

- `cargo fmt --all -- --check`: **PASS**.
- `cargo check --all-targets --locked`: **PASS**.
- `cargo test --all-targets --locked`: **PASS**.
- `cargo clippy --all-targets --locked -- -D warnings`: **PASS**.
- `python3 scripts/check-docs.py`: **PASS**.
- `git diff --check`: **PASS**.

The historical v0.4-v0.6 campaigns, broad application matrix, real Linux VT,
ordinary desktop, Wayland, x86_64 and release/package campaign were not rerun.

## 16. Commits and files

- `bd43199` — `fix: recover managed headless after abrupt restart` — bounded
  helper fix plus the Docker/SSH live harness.
- `docs: record headless-first v0.7C qualification` — current support
  classifications, Roadmap updates, usage note and this handoff.

No Cargo/package version, release workflow, tag or public release changed.

## 17. Remaining limitations and risks

- P0: none.
- P1: none for the approved, claimed topology.
- Linux native virtual-console TTY: evidence gap, not a product defect; NOT
  TESTED and not claimed Supported.
- SSH client breadth: only the recorded OpenSSH path is qualified.
- Docker breadth: no x86_64, alternate engine/distribution, production image,
  archive ABI or broad application matrix is qualified.
- Abrupt disconnect: no original PTY/TUI restoration or transparent semantic
  continuation is promised after transport destruction.
- Validation-infrastructure P2: changing the embedded source commit currently
  invalidates the Docker apt layer and makes a clean identity rerun slower; it
  does not alter runtime correctness or qualification results.
- Ordinary X11/SSH -> Desktop: optional compatibility, NOT TESTED.
- Native Wayland/XWayland/Headless Wayland: 0.7D scope, NOT TESTED.
- x86_64 and final package/environment integration: 0.7E scope.

## 18. Architecture conclusion

The existing installer, explicit session selection, Doctor, Managed helper,
AT-SPI backend, RuntimeSession/application-generation model and terminal guard
were sufficient. No new environment manager, semantic backend or authority
model was needed. Docker and SSH changed transport/deployment only; they did
not create a second interaction architecture.

The helper defect was a stale startup-file race and early cleanup-scope issue,
not an authority-model failure. The fix remained inside the existing Managed
lifecycle boundary.

## 19. 0.7C exit decision

The revised hard exits are satisfied for every topology now claimed:

- historical Managed qualification remains attributable;
- Docker and SSH have reproducible installed-binary, explicit-session,
  fresh-selection, operation/readback, continuation and exit evidence;
- SSH terminal/handler/disconnect boundaries are recorded honestly;
- Linux VT is accurately NOT TESTED and not substituted by a PTY;
- cleanup and private-artifact rules passed; and
- ordinary desktop and Wayland rows remain explicitly incomplete.

Therefore 0.7C is complete with explicit limitations. This is not v0.7
milestone qualification.

## 20. v0.7 roadmap

- Discovery: **COMPLETE**.
- 0.7A: **COMPLETE / VALIDATED**.
- 0.7B: **COMPLETE / VALIDATED**.
- 0.7C: **COMPLETE / VALIDATED WITH EXPLICIT LIMITATIONS**.
- 0.7D: **PLANNED / NOT AUTHORIZED**.
- 0.7E: **PLANNED / NOT AUTHORIZED**.
- v0.7 milestone close: **NOT STARTED / NOT AUTHORIZED**.

## 21. Git and release status

- Branch: `v0.7/deployment-environment-completeness`.
- HEAD: documentation commit containing this handoff.
- Worktree: clean.
- Remote: local branch contains the prior unpushed contract revision and this
  phase's commits; no push was authorized or performed.
- Package version: `0.3.0`.
- Public tags: unchanged (`v0.1.0`, `v0.2.0`, `v0.3.0`).
- v0.7.0 tag/release: none.

## 22. Next recommended authorization

If Local Linux VT is intended to be a 1.0 Supported topology, authorize one
narrow real virtual-console TTY -> Managed qualification next. Otherwise the
next planned phase is **0.7D — Wayland and XWayland Qualification**. Neither
starts automatically.

## 23. Next Codex context

首先阅读 `AGENTS.md`、`docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、`docs/planning/v0.7-deployment-environment.md`、
`docs/planning/v0.7-headless-first-contract.md`、`docs/planning/v0.7-roadmap.md`、
v0.6 milestone HANDOFF、v0.7A/v0.7B HANDOFF、历史 0.7C HANDOFF 及本 HANDOFF。
当前分支为 `v0.7/deployment-environment-completeness`，准确 HEAD 以本 HANDOFF
所在文档提交为准；v0.4-v0.6 已完成内部 Milestone Qualification，0.7A、0.7B
已完成，0.7C 现为 COMPLETE / VALIDATED WITH EXPLICIT LIMITATIONS。历史 Ubuntu
24.04 arm64 Managed Xvfb 资格继续有效；当前 `bd43199` 已在 Ubuntu 24.04 arm64
非特权 Docker 环境中通过真实 `docker exec -it` 和 loopback OpenSSH PTY 验证，
包括安装、Doctor、explicit Managed selection、fresh application selection、
semantic operation/readback、dynamic continuation、Vim terminal handoff、正常退出、
受控异常断开、fresh reconnect、container restart 后 stale descriptor/authority
拒绝和安全卸载。Managed helper 已修复 restart 后 stale display 文件竞态及早期
cleanup 局部变量问题。Linux native VT、普通本地 X11、SSH -> existing Desktop、
Native Wayland/XWayland/Headless Wayland 仍为 NOT TESTED；Docker 只在记录的验证
拓扑获得带限制资格，不存在正式生产镜像。Session 仅选择 transport；application
authority 仍必须由 fresh registry enumeration、用户明确选择、fresh generation、
exact locator、current scope/capability/ticket 和 authoritative readback 建立。
PasswordText、private artifact 0700/0600、external handler 不延长权限、terminal
single owner/reader 等不变量不变。0.7D、0.7E、v0.7 Milestone Close、1.0
Integration、RC/tag/release 均未授权；package version 仍为 0.3.0，下一计划公开
版本仍是 v1.0.0。若 1.0 要声称 Local Linux VT，应先另行授权真实 virtual-console
资格验证；否则等待用户明确授权 0.7D。不得自行扩展任务。
