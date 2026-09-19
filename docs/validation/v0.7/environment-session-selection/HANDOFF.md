# GUI2TUI v0.7A Environment Contract and Session Selection Handoff

## 1. Status

- Starting HEAD: `1cf610af1222357145eda65a472187cf22e65fd7`.
- Final HEAD: the documentation commit containing this handoff.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree: clean at final handoff.
- Production changes: explicit session selection, bounded descriptor handling,
  topology status and connection-error redaction.
- P0: none.
- P1: none.
- Overall: **0.7A COMPLETE / VALIDATED**.

## 2. Commits

- `4bd4d47` — `feat: make desktop and managed session selection explicit`:
  production selection/status behavior, focused regression tests and the
  bounded Linux live harness.
- `docs: validate v0.7 session selection` — approved environment direction,
  user/architecture documentation, Roadmap state and this evidence record.

## 3. Approved 1.0 environment direction

- Local Linux X11: core support target.
- Managed Xvfb/headless: core support target.
- Native Wayland and XWayland: separate priority validation targets; seek
  inclusion when real evidence supports it.
- Same-host SSH TUI: seek inclusion by reusing the same-user selected session
  and current terminal lifecycle.
- Cross-host Remote Companion: deferred until after 1.0.

These are development targets. Real local X11, native Wayland, XWayland and
SSH PTY have not become formally Supported merely by being named here.

## 4. Existing session behavior

Before 0.7A, `gui2tui` applied a valid persistent descriptor for every command
except `setup`, and `gui2tui-inspect` always applied it. The descriptor replaced
inherited `DISPLAY`, session bus and session type before AT-SPI connection.
`GUI2TUI_NO_MANAGED_SESSION` was the only opt-out. Doctor could probe the
resulting bus but could not name Current desktop versus Managed headless.

The descriptor contained schema version, supervisor PID, Xvfb display and
session-bus address. Existing loading checked bounded format, same-user private
storage and Linux supervisor PID/UID. `setup` created state and child processes;
it did not modify the caller's shell. The actual application selector always
enumerated the one connected registry and selected an exact current locator.

The defect was connection precedence, not application identity: a historical
descriptor could replace a valid inherited desktop environment and expose a
different application universe without an explicit CLI choice.

## 5. New session-selection contract

- `--session desktop`: use only inherited process environment; never load the
  managed descriptor.
- `--session managed`: require and apply the existing valid descriptor; never
  create a session and never fall back to desktop.
- No flag: preserve compatibility. `GUI2TUI_NO_MANAGED_SESSION` selects
  desktop; otherwise a valid descriptor selects managed; an absent descriptor
  selects desktop; an unusable descriptor warns and selects desktop.
- The chosen environment is applied before Tokio, zbus or AT-SPI creation.
- Unified `gui2tui inspect` passes the resolved choice to its private inspector
  child, so it cannot reselect another topology.
- `gui2tui setup persistent/restart` verifies with an explicit managed Doctor.

No cross-run recent-session preference is stored.

## 6. Current desktop behavior

Desktop mode preserves the current process's session D-Bus/display environment
and installs only a process-local opt-out for private children. It does not
read the descriptor, search other sessions/users or infer intent from
application names, PIDs, display numbers or geometry. Connection failure is
reported for the selected session and never triggers managed fallback.

## 7. Managed headless behavior

Managed mode reuses the existing descriptor/helper. Loading requires a
bounded known schema, current-user 0700/0600 storage, a regular single-link
descriptor, valid fields and a current same-UID Linux supervisor. The selected
bus is then probed normally by zbus/AT-SPI. Missing, invalid, stopped or unsafe
state fails before connection; an unavailable bus fails during connection.
GUI2TUI neither creates a replacement nor switches to desktop. A valid
registry with zero applications remains a non-blocking application-state
warning.

## 8. Stale descriptor safety

Live evidence covered missing descriptor, invalid JSON, stopped recorded
supervisor, unsafe descriptor permissions and a structurally valid descriptor
whose bus was unavailable. Default compatibility mode warned and used the
inherited environment; explicit managed mode blocked. GUI2TUI did not delete
the tested descriptor. Full foreign-UID execution was not performed; the
existing UID/private-path rejection remains source-verified rather than live
cross-user evidence. No more precise cause is claimed beyond the checks that
actually ran.

## 9. Session status and UX

Interactive startup/launch and the inspector print `Session: Current desktop`
or `Session: Managed headless` with explicit/default source. Doctor adds a
contents-free `session-selection` check and labels the session-bus probe with
the topology. The TUI's existing `d` diagnostics uses the same selection.
Connection failure, zero applications and application-not-found remain
separate states. Bus addresses, descriptor contents and display values are
redacted to set/unset presence in backend errors.

Recovery guidance is bounded: choose desktop explicitly, or use existing
`setup status`, `persistent`, `restart` or `stop` operations. No automatic
file deletion, permission change, system setup or session search occurs.

## 10. Semantic authority

Session selection changes transport environment only. It creates no
`RuntimeSession`, generation, cache, scene, scope, focus, binding, command,
ticket or application authority. Applications still come from a fresh current
registry enumeration. An explicit exact current selection constructs the
application view and `ApplicationGenerationId`; reselection retires the old
view through the existing v0.6 path. No name, PID, display, geometry,
`RuntimeNodeId` or prior locator is reused as authority, and no operation is
automatically replayed.

## 11. Linux live evidence

Environment: Ubuntu 24.04 arm64 OrbStack guest, unprivileged UID, two isolated
Xvfb/X11 + session-D-Bus + AT-SPI registries, GTK4 fixtures. Harness:
[`v07a_session_selection.sh`](../../../../tests/live/v07a_session_selection.sh).

- Desktop selection: PASS for an inherited controlled Xvfb environment;
  **real ordinary local X11 desktop NOT TESTED**.
- Managed selection: PASS with the existing persistent helper/descriptor.
- Descriptor precedence: PASS; explicit universes did not mix, compatible
  default selected managed, and environment opt-out selected desktop.
- Invalid/stale descriptor: PASS for missing, invalid, stopped, unsafe and
  unreachable cases.
- Fresh application selection: PASS from each current registry.
- Basic semantic operation: PASS; an advertised fixture button action was
  followed by authoritative semantic readback.
- Cleanup: PASS; managed supervisor stopped, descriptor was absent and no
  fixture/helper process remained.

No Wayland, XWayland, SSH PTY or real local desktop qualification was run.

## 12. Existing capability regression

The managed fixture was freshly inspected, one existing safe Action was
invoked and its resulting accessible status was read back. Current registry
selection and zero-app distinction ran in both topologies. Focused CLI tests
also retained config, launcher and Doctor behavior. The v0.4–v0.6 campaigns,
selection/PageTab/Expand matrix, external handler and historical application
matrix were not repeated.

## 13. Genericity and forbidden audit

No application/toolkit production branch, private API, DOM/CDP, UNO, OCR,
vision, keyboard/mouse injection, coordinate operation, fuzzy identity,
backing-file mutation or anonymous action fallback was added. No
`SessionManager`, `DeploymentManager`, `EnvironmentRegistry`, remote protocol,
semantic backend or runtime identity was introduced. Descriptor and reports
retain current privacy boundaries; PasswordText behavior is unchanged.

## 14. Tests and quality

- Focused macOS/source checks: session-choice CLI regression, Doctor/config/
  launcher user CLI tests, inspector CLI tests and connection-value redaction:
  PASS.
- Ubuntu 24.04 arm64 controlled Linux live harness: PASS on the final
  production source; real local X11: NOT TESTED.
- `cargo fmt --all -- --check`: PASS.
- `cargo check --all-targets`: PASS.
- `cargo test`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `python3 scripts/check-docs.py`: PASS.
- `git diff --check`: PASS.

## 15. Remaining issues

- 0.7A P0: none.
- 0.7A P1: none.
- Environment limitation: no ordinary local X11 desktop was available; its
  install/start/operation/exit qualification remains 0.7C.
- 0.7B: install/remove and broader diagnostic completeness, not authorized.
- 0.7C: real X11, complete managed deployment and SSH PTY qualification, not
  authorized.
- 0.7D: native Wayland/XWayland evidence and classification, not authorized.
- 0.7E: current-source two-architecture package/environment qualification,
  not authorized.
- VNC/RDP, other distributions, headless Wayland and macOS/Windows semantic
  backends remain outside this phase; cross-host Remote Companion is post-1.0.

## 16. Architecture conclusion

- Existing architecture sufficient for explicit session selection: **YES**.
- New Session Manager required: **NO**.
- Transport/session choice remains separate from application authority:
  **YES**.
- Existing runtime continuity invariants remain intact: **YES**. Session
  selection is startup-only; in-process cross-session switching was not added.

## 17. v0.7 roadmap

- Discovery: **COMPLETE**.
- 0.7A: **COMPLETE / VALIDATED**.
- 0.7B: **PLANNED / NOT AUTHORIZED**.
- 0.7C: **PLANNED / NOT AUTHORIZED**.
- 0.7D: **PLANNED / NOT AUTHORIZED**.
- 0.7E: **PLANNED / NOT AUTHORIZED**.

No later phase was started.

## 18. Git status

- Branch: `v0.7/deployment-environment-completeness`.
- HEAD: the `docs: validate v0.7 session selection` commit containing this
  handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, `v0.3.0`, unchanged.
- v0.7.0 tag/release: absent.

## 19. Next recommended direction

Recommend **0.7B — Installation and Diagnostic Completeness**. It is not
authorized. Wait for explicit user review and authorization.

## 20. Next Codex context

新会话必须先读 `AGENTS.md`，再依次读 `docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、`docs/validation/v0.6/milestone/HANDOFF.md`、
`docs/planning/v0.7-deployment-environment.md`、
`docs/planning/v0.7-roadmap.md` 和本 HANDOFF。当前分支是
`v0.7/deployment-environment-completeness`；准确 HEAD 以本 HANDOFF 所在的
`docs: validate v0.7 session selection` 提交为准。v0.4–v0.6 均已完成内部
Milestone Qualification；0.7A 已完成并验证。用户确认的 1.0 开发目标是：本地
Linux X11 与 Managed Xvfb 为核心目标，Native Wayland/XWayland 优先验证并争取
纳入，same-host SSH TUI 争取纳入，cross-host Remote Companion 延期到 1.0 后。
这些是开发目标，不是已完成的正式支持资格。`--session desktop` 只使用继承的
当前桌面环境且绝不读取 descriptor；`--session managed` 只使用现有有效私有
descriptor，缺失、无效、停止、不安全或连接不可用时失败，不创建也不回退；未
指定时保留可见、可预测的兼容路径。Session 选择只决定 transport，不授予
application authority；fresh registry enumeration、明确 exact application 选择、
fresh generation、scope/capability/ticket/readback 规则不变。stale descriptor 检查
不会自动删除用户文件。受控 Ubuntu 24.04 arm64 双 Xvfb live 证据通过，但真实
本地 X11、SSH PTY、Native Wayland/XWayland 仍未资格验证。0.7B–0.7E 均未授权；
v0.7 仍为内部里程碑，package version 仍为 0.3.0，v1.0.0 仍是下一计划公开
版本。不得自动扩展任务或开始下一阶段。
