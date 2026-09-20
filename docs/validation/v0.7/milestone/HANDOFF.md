# GUI2TUI v0.7 Deployment & Environment Completeness Milestone Handoff

## 1. Status

- Starting HEAD: `ea04f47c0b42ee37f1758effc68501cf2d52c884`.
- Final HEAD: the documentation/status commit containing this handoff.
- Branch: `v0.7/deployment-environment-completeness`.
- Worktree: clean at final handoff.
- Production changes during Milestone Qualification / Close: none.
- Tests changed during Milestone Qualification / Close: none.
- Functional development: **COMPLETE**.
- Milestone qualification: **QUALIFIED WITH THE RECORDED ENVIRONMENT
  LIMITATIONS**.
- P0: none.
- P1: none.
- Overall: **GUI2TUI v0.7 DEPLOYMENT & ENVIRONMENT COMPLETENESS — COMPLETE /
  MILESTONE QUALIFIED / INTERNAL**.

This closes only the internal v0.7 milestone. It does not authorize v1.0
Integration & Stabilization, an RC, a tag, a package-version change or a
public release.

## 2. Qualification decision

The approved Headless-first contract has sufficient attributable evidence for
every environment currently labelled qualified, while every topology without
matching evidence remains explicitly limited, optional, not tested or
deferred. The installed package can operate independently of the checkout in
the recorded Managed Xvfb, Docker, same-host SSH, Native Wayland and XWayland
topologies. Session selection, application authority, terminal ownership,
private artifacts and authoritative readback retain the v0.1-v0.6 contracts.

The evidence therefore supports closing v0.7 as an internal qualified
deployment milestone. This is not a claim of universal Linux, compositor,
terminal, toolkit or application compatibility, and it is not a public 1.0
support declaration.

## 3. Audit basis and source identities

The close reviewed current source, planning, product documentation, install
and package scripts, Doctor/session-selection implementation, live harnesses,
the v0.6 milestone record and all v0.7 phase handoffs. It did not infer the
result from phase `Overall` fields alone.

Important identities are distinct:

- v0.7 close starting source: `ea04f47c0b42ee37f1758effc68501cf2d52c884`;
- exact 0.7E package source: `eb5841fbfb97b50ccf8a0d93d702012667b47ee3`;
- final package-driven live-harness source:
  `82fed46f241af2fb50180e523ecce6de472a65c3`; and
- final milestone documentation source: the commit containing this handoff.

Everything after `eb5841f` and before this close changed only live harnesses or
documentation. The production payload at milestone close therefore matches
the qualified package source. Package version remains `0.3.0`.

## 4. Phase qualification summary

### 0.7A — Environment Contract and Session Selection

**COMPLETE / VALIDATED.** `--session desktop` uses only the inherited current
session and never reads a Managed descriptor. `--session managed` requires one
existing valid private descriptor and never creates a session or falls back.
The documented no-flag compatibility path is visible and predictable.
Selection happens before asynchronous/D-Bus initialization and chooses
transport only. Current registry enumeration plus explicit current application
selection establishes a fresh application generation.

Evidence: [0.7A handoff](../environment-session-selection/HANDOFF.md).

### 0.7B — Installation and Diagnostic Completeness

**COMPLETE / VALIDATED.** The user-prefix installer requires no root, installs
the fixed `bin`/`libexec` layout, and resolves helpers relative to the running
main program rather than a checkout or current directory. Doctor separates
installation, terminal, selected Session D-Bus, Accessibility bus, registry,
zero applications and unassessed application semantics. Optional helper or
handler absence remains feature-local. The private manifest/hash/ownership
uninstaller removes only owned files and preserves configuration, runtime and
recovery data plus unrelated prefix contents.

Evidence: [0.7B handoff](../installation-diagnostics/HANDOFF.md).

### 0.7C — Headless-first Environment Qualification

**COMPLETE / VALIDATED WITH EXPLICIT LIMITATIONS.** The original available-
environment run qualified Managed Xvfb but correctly recorded real local X11
and SSH as not tested. It remains an immutable historical record, not later
SSH evidence. A separately authorized Headless supplement then qualified an
unprivileged Docker interactive PTY and a real loopback OpenSSH -> Managed PTY,
including Vim handoff, authoritative writeback/readback, normal exit,
controlled disconnect/reconnect, container restart, stale authority refusal
and exact cleanup. Linux native VT and ordinary desktop rows remain unclaimed.

Evidence: [original 0.7C handoff](../x11-headless-ssh/HANDOFF.md) and
[Headless supplement](../headless-first-qualification/HANDOFF.md).

### 0.7D — Wayland and XWayland Qualification

**COMPLETE / QUALIFIED WITH EXPLICIT LIMITATIONS.** Native GTK4/Qt6 and
XWayland GTK4 clients were identified independently in an unprivileged Ubuntu
24.04 aarch64 Weston 13 headless/Pixman environment. Public AT-SPI semantics,
real operations, fresh readback, dynamic refresh, modal scope, native Vim
handoff, terminal restoration and application/compositor rebuild passed.
Collapsed GTK screen origins are now rejected as untrusted presentation
evidence without revoking semantic operation; differentiated Qt geometry
remains usable. No Wayland-specific semantic backend was added.

Evidence: [0.7D handoff](../wayland-xwayland/HANDOFF.md).

### 0.7E — Package and Environment Contract Qualification

**COMPLETE / QUALIFIED WITH EXPLICIT LIMITATIONS.** Exact-source GNU/Linux
x86_64 and aarch64 archives passed layout, ELF, ABI, permissions, checksums,
extracted smoke, fresh installation, installed Doctor/Managed setup/semantic
smoke and safe uninstall. The exact aarch64 package additionally reran the
approved Managed, Docker, real SSH, Native Wayland and XWayland integrations.
The x86_64 execution used amd64 Docker/OrbStack platform emulation on an arm64
host; native x86_64 hardware and the full x86_64 environment matrix are not
claimed.

Evidence: [0.7E handoff](../package-environment/HANDOFF.md).

## 5. Final Headless-first environment contract

These are v0.7 evidence classifications and boundaries for later 1.0
integration. They do not publish a new package support promise.

| Environment / capability | v0.7 qualification | Exact boundary |
| --- | --- | --- |
| Managed Xvfb | **QUALIFIED** | Exact aarch64 package on Ubuntu 24.04; install, lifecycle, task/readback, handler, terminal and cleanup |
| Docker Headless interactive PTY | **QUALIFIED** | Unprivileged Ubuntu 24.04 aarch64 container; no host display/socket/device, privileged mode or production image claim |
| Same-host SSH -> Managed | **QUALIFIED** | Exact aarch64 package, loopback OpenSSH interactive PTY, same container/user/session; bounded client and disconnect contract |
| Native Wayland Headless | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Ubuntu 24.04 aarch64, Weston 13 headless/Pixman, controlled native GTK4 and Qt6 |
| XWayland Headless | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Same compositor with XWayland 23.2.6 and controlled GTK4; Qt-over-XWayland not run |
| GNU/Linux aarch64 package | **QUALIFIED** | Native arm64 container execution, fresh install and full recorded environment integration |
| GNU/Linux x86_64 package | **QUALIFIED WITH EXPLICIT EMULATION LIMITATION** | Actual amd64 ELF execution/install/Doctor/Managed/semantic smoke/uninstall under platform emulation; no native x86_64 hardware/full environment campaign |
| Linux native VT/TTY | **NOT TESTED** | PTY evidence is not substituted for a real virtual console |
| Ordinary local X11 desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | Controlled Xvfb is not substituted |
| Ordinary full-desktop Wayland | **NOT TESTED / OPTIONAL COMPATIBILITY** | Headless Weston is not substituted |
| SSH -> existing Desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | Same-host SSH -> Managed evidence is not substituted |
| Wayland over real SSH PTY | **NOT TESTED** | Separate SSH/Xvfb and local Wayland evidence is not combined |
| VNC/RDP/independent remote desktop | **NOT TESTED / OUTSIDE HEADLESS CORE** | No named product topology qualified |
| Wayland static visual capture | **UNSUPPORTED / DEFERRED** | Public semantic operation remains independent of capture |
| Cross-host Remote Companion | **UNSUPPORTED / DEFERRED POST-1.0** | No cross-host semantic transport or remote authority model |
| macOS/Windows semantic backend | **UNSUPPORTED / OUTSIDE LINUX BASELINE** | Build/development hosts only |

A complete GNOME, KDE, XFCE, VNC or RDP environment is not required by any
qualified Headless-core path. The GUI application still requires the recorded
background graphical runtime and same-host Accessibility session.

## 6. Package and ABI basis

Both internal archives were built from `eb5841f` with the same locked
dependencies on the Ubuntu 22.04 qualification baseline using Rust 1.88.

| Artifact | SHA-256 | Actual maximum glibc reference |
| --- | --- | --- |
| `gui2tui-0.3.0-linux-x86_64.tar.gz` | `0fe066eb1835d4816d7cff804c668b6f06c88d9ecdb1a30fd451648fc921fa9a` | 2.34 |
| `gui2tui-0.3.0-linux-aarch64.tar.gz` | `4da7ef82d0ef8bd657ba22fe0f8b7638d737f21008eff2e0978113361b621e57` | 2.34 |

Both passed the declared glibc 2.35 gate. Direct dependencies were limited to
the ordinary loader/libc/libgcc_s/libm set appropriate to each architecture;
no GLIBCXX dependency was reported. The archives contained the main program,
Inspector, optional same-host helper, Managed helper, installer/uninstaller,
metadata and smoke resources, with no dangerous member, writable member,
unexpected host metadata or developer checkout path. These were internal
qualification bytes and were not published.

## 7. Installation, session and Doctor conclusion

The installed package runs from an arbitrary directory and finds required
helpers without Rust, Cargo or a checkout. Default and space-containing
prefixes passed on both package architectures. Installation neither selects
an application nor persists semantic authority.

Doctor from the installed package distinguished:

- no Session D-Bus;
- Session D-Bus without `org.a11y.Bus`;
- missing, stopped, invalid, unsafe and unreachable Managed descriptors;
- reachable registry with zero applications;
- required versus optional helper failures;
- absent or unavailable external handler without executing it; and
- healthy PTY versus dumb `TERM`, non-UTF-8 locale or zero dimensions.

Doctor deliberately reports application semantic sufficiency as `NOT CHECKED`
rather than traversing or mutating GUI controls. The narrower live injection
where an Accessibility address is returned and the registry then dies was not
constructed during 0.7E; the bounded source path remains, and this is an
evidence limitation rather than a reproduced defect.

## 8. Semantic authority and runtime-continuity audit

The deployment work preserves the v0.1-v0.6 model:

- authority remains `RuntimeSessionId` + `ApplicationGenerationId` + exact
  `BackendLocator`, current `InteractionScope`, current advertised capability,
  operation contract/ticket and authoritative readback;
- session, descriptor, display, name, PID, title, geometry, historical index
  and `RuntimeNodeId` do not grant or migrate operation authority;
- application/session replacement requires current registry enumeration,
  explicit selection and a fresh generation;
- old generation work, bindings, locators, commands and tickets cannot publish
  into or operate a replacement;
- transport recovery restores communication only;
- events remain bounded wakeup/invalidation signals, while current public
  Accessibility reads establish truth;
- full refresh uses the current public Accessible tree rather than cache
  residency or incomplete event history; and
- non-idempotent operations are never automatically replayed.

Installed-package live evidence exercised Action/Toggle, current selection,
selection/value smoke, modal scope, dynamic refresh, stale command/locator
refusal, qualified complete text, conflict refusal and manual continuation.
The complete historical v0.3-v0.6 campaigns were not rerun; their milestone
handoffs remain the qualification source for the broader interaction matrix.

## 9. Terminal, external work, privacy and security

- Terminal ownership remains single-owner/single-reader during normal TUI,
  external handler handoff and reacquisition.
- Normal exit and the tested Docker/SSH/Wayland paths restored canonical input
  and echo. Existing v0.6 signal and Linux suspend/resume evidence remains the
  basis for those broader lifecycle guarantees.
- A destroyed SSH PTY is not transparently restored; reconnect starts a fresh
  TUI and application selection.
- External handlers remain configured direct argv with exactly one `{file}`;
  candidate possession or handler completion does not extend target authority.
- Candidate and runtime namespaces retain 0700/0600 ownership. Stale/conflict
  data may remain only as bounded private recovery data and cannot attach to a
  replacement target.
- PasswordText remains unreadable, unexported and absent from reports and
  external handling.
- GUI2TUI never writes an application's backing file to simulate a semantic
  operation.

## 10. Genericity and forbidden audit

No v0.7 production path added an application, process, title, toolkit or
compositor-specific semantic branch. Validation identity observations remain
fixture evidence only. The milestone introduced no DOM/CDP, UNO, private
toolkit/compositor semantic API, OCR, screenshot semantics, GUI input
injection, coordinate click, fuzzy target matching, anonymous action guess,
Remote Companion, new runtime identity, session/deployment manager, daemon or
workflow engine.

The single Wayland-related production change rejects one generic collapsed-
origin geometry pattern. It changes presentation evidence only and neither
creates nor removes semantic capability.

## 11. Product defects and evidence gaps

### Resolved product/deployment defects

- Managed restart stale display-number race and early-cleanup variable scope:
  fixed and container-restart qualified in 0.7C.
- Collapsed positive screen origins manufacturing unsupported spatial
  relations: fixed generically and qualified with collapsed GTK plus valid Qt
  geometry in 0.7D.
- Installed uninstaller default-prefix inference: fixed and live-regressed in
  0.7E.
- Checkout-dependent host metadata entering packages: fixed and enforced by
  archive validation in 0.7E.

### Open severity

- P0: none.
- P1: none.
- Reproduced product P2 specific to v0.7: none.

### Evidence and compatibility boundaries

Native x86_64 hardware/full environment integration, Linux virtual-console
TTY, ordinary X11/Wayland desktops, SSH -> existing Desktop, Wayland over SSH,
other distributions, other compositors, Qt-over-XWayland and broad terminal/
SSH-client/application matrices remain outside the proof. They are not silently
classified as product defects, and none is represented as Supported.

## 12. Milestone exit gates

1. **PASS — approved contract:** Headless-first classifications and
   Remote-Companion deferral are documented without rewriting history.
2. **PASS — independent installation:** the approved current-source packages
   install without root and find every required helper outside the checkout.
3. **PASS — explicit session choice:** desktop and Managed select exactly one
   environment with no explicit-choice fallback.
4. **PASS — bounded diagnosis:** Doctor distinguishes installation, terminal,
   Session D-Bus, Accessibility bus, registry, zero-app and unassessed
   application semantics without private payloads or mutation.
5. **PASS — exact application authority:** fresh enumeration and explicit
   current selection remain the only authorization path.
6. **PASS — Headless environments:** every environment labelled qualified has
   attributable install-to-task-to-readback-to-cleanup evidence.
7. **PASS — SSH contract:** the declared same-host SSH -> Managed topology has
   real interactive-PTY, handler, normal-exit and disconnect-boundary evidence.
8. **PASS — Wayland classification:** Native Wayland, XWayland and Headless
   Wayland are separate, bounded results; geometry loss degrades presentation
   only.
9. **PASS — packages:** both architectures have current-source layout, ABI,
   extracted execution and fresh-install evidence with the x86_64 emulation
   boundary explicit.
10. **PASS — privacy:** configuration, reports, descriptors, candidates and
    runtime artifacts retain current-user ownership and 0700/0600 boundaries;
    PasswordText remains excluded.
11. **PASS — prior invariants:** representative semantic/runtime regression and
    the final full source-quality matrix retain v0.4-v0.6 authority, refresh,
    resource, terminal and external-writeback contracts.
12. **PASS — truthful boundary/severity:** untested/deferred rows are not
    presented as Supported; P0 = 0 and P1 = 0.

## 13. Quality basis for milestone close

The final production/package phase ran, on the production-identical source:

- dual-architecture package build, ELF/ABI/layout/checksum validation;
- both-architecture extracted and fresh-install smoke;
- package-driven Managed/Docker/SSH and Native-Wayland/XWayland live runs;
- `cargo fmt --all -- --check`;
- `cargo check --all-targets --locked`;
- `cargo test --all-targets --locked` — 300 library, 2 Inspector and 5 user-CLI
  integration tests passed;
- `cargo clippy --all-targets --locked -- -D warnings`;
- documentation audit; and
- diff check.

Milestone Close changes documentation only. It does not rerun the Linux,
Docker, SSH, Wayland, package or Rust campaigns. It runs the documentation
link audit and `git diff --check` against the close; these results are recorded
by the final milestone commit.

## 14. Healthy baseline for 1.0 Integration

The current source is a healthy deployment baseline for a separately
authorized **v1.0 Integration & Stabilization** phase. No open deployment P0 or
P1 blocks beginning that work. Integration must still treat the recorded
environment boundaries as constraints rather than silently broadening support.

Priorities for that future phase include cross-milestone task regression,
long-running stability/resource evidence, complete first-use experience,
terminal/spatial/dynamic-continuation usability, final support wording,
performance and failure recovery, documentation/examples, and eventual release
qualification. This list is a handoff, not current authorization.

## 15. Roadmap status

- v0.4: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.5: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.6: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v0.7: **COMPLETE / MILESTONE QUALIFIED / INTERNAL**.
- v1.0 Integration & Stabilization: **PLANNED / NOT AUTHORIZED**.
- v1.0.0 RC and public release: **NOT AUTHORIZED**.

No v0.8 or v0.9 milestone is inferred by this close.

## 16. Git and release status

- Branch: `v0.7/deployment-environment-completeness`.
- HEAD: the `docs: qualify v0.7 deployment environment milestone` commit
  containing this handoff.
- Worktree: clean at final handoff.
- Remote: synchronized at final handoff.
- Package version: `0.3.0`, unchanged.
- Public tags: `v0.1.0`, `v0.1.1`, `v0.2.0`, `v0.3.0`, unchanged.
- v0.7.0 tag/release: absent.
- v1.0.0 RC/tag/release: absent.

## 17. Next recommended direction

Recommend a separately authorized **v1.0 Integration & Stabilization** phase.
It is not an RC and does not authorize a public release. This task did not
start it.

## 18. Next Codex context

新会话先读 `AGENTS.md`、`docs/project-guide.md`、
`docs/planning/roadmap-to-1.0.md`、`docs/planning/v0.7-deployment-environment.md`、
`docs/planning/v0.7-headless-first-contract.md`、`docs/planning/v0.7-roadmap.md`、
v0.6 milestone HANDOFF、0.7A–0.7E 全部阶段 HANDOFF 及本 milestone HANDOFF。
当前分支是 `v0.7/deployment-environment-completeness`；准确 HEAD 是包含本报告的
`docs: qualify v0.7 deployment environment milestone` 提交，工作树和远端应保持
同步。v0.4–v0.7 均为 COMPLETE / MILESTONE QUALIFIED / INTERNAL。v0.7 的
Headless-first 合同以 exact aarch64 package 在 Ubuntu 24.04 上的 Managed Xvfb、
unprivileged Docker interactive PTY、real same-host OpenSSH -> Managed、Weston
13 Headless Native Wayland 和 XWayland 证据为核心；x86_64 package 已真实执行并
完成安装/Doctor/Managed/semantic smoke/uninstall，但带 arm64 宿主上的 amd64
容器模拟限制。Linux native VT、普通 X11/Wayland desktop、SSH -> existing
Desktop、Wayland over SSH 和更广矩阵仍未测试且不声称支持。精确 package source
为 `eb5841fbfb97b50ccf8a0d93d702012667b47ee3`；最终生产源码与该 payload
一致，后续提交仅改 live harness/docs。Session selection 只选择 transport；fresh
registry enumeration、用户明确选择、fresh generation、exact locator、current
scope/capability/ticket 和 authoritative readback 才建立应用权限。full refresh、
event、PasswordText、0700/0600 artifact、external handler 和 terminal single-
owner/reader 不变量不变。package version 仍为 0.3.0，公开标签仅到 v0.3.0，
不存在 v0.7 release。下一步只能建议 v1.0 Integration & Stabilization，尚未授权；
不得自动开始 RC、tag、release、新功能或其他环境 campaign。
