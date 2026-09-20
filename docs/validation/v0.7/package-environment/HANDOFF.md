# GUI2TUI v0.7 Phase 0.7E — Package and Environment Contract Qualification Handoff

## 1. Status

- Starting HEAD: `6dcb5534b3e17a75bf91631ef29c0530de48a08f`
- Qualification package source: `eb5841fbfb97b50ccf8a0d93d702012667b47ee3`
- Final live-harness source: `82fed46f241af2fb50180e523ecce6de472a65c3`
- Branch: `v0.7/deployment-environment-completeness`
- Package version: `0.3.0` (unchanged)
- Production changes: two bounded deployment fixes; no Rust production or
  semantic/runtime change
- P0: 0
- P1: 0 open; one user-prefix uninstall defect was reproduced and fixed
- Overall: **0.7E COMPLETE / QUALIFIED WITH EXPLICIT LIMITATIONS**

The exact final repository HEAD and remote state are reported by the phase
close after this handoff is committed. This phase does not close the v0.7
milestone and does not authorize 1.0 integration, an RC, a tag or a release.

## 2. Fixed source and artifacts

Both internal qualification archives were built from the same exact Git
commit and `Cargo.lock`:

`eb5841fbfb97b50ccf8a0d93d702012667b47ee3`

The later `2e58d7d`, `10a3e87` and `82fed46` commits change only live
qualification sequencing or evidence identity. They do not change the
qualified package payload. The final Wayland result records both identities:
`source_commit=eb5841f...` for the installed package and
`harness_source_commit=82fed46...` for the driver.

| Artifact | SHA-256 | Size | ELF architecture |
| --- | --- | ---: | --- |
| `gui2tui-0.3.0-linux-x86_64.tar.gz` | `0fe066eb1835d4816d7cff804c668b6f06c88d9ecdb1a30fd451648fc921fa9a` | 15,785,071 bytes | Advanced Micro Devices X86-64 |
| `gui2tui-0.3.0-linux-aarch64.tar.gz` | `4da7ef82d0ef8bd657ba22fe0f8b7638d737f21008eff2e0978113361b621e57` | 15,559,671 bytes | AArch64 |

The archives are internal evidence under `/tmp/gui2tui-v07e-package-eb5841f`.
They were not published and are not a v0.7.0 or v1.0.0 release artifact.

## 3. Build, ABI and runtime dependencies

Both architectures were built in an Ubuntu 22.04 qualification container with
Rust 1.88 and locked dependencies. `BUILD-INFO.json`, per-archive `ABI.json`,
the assembled manifest and checksums all identify the same source. The ABI
gate allowed at most glibc 2.35; the actual maximum referenced symbol version
was glibc 2.34 for every binary on both architectures.

The three ELF programs are `gui2tui`, `gui2tui-inspect` and `gui2tui-local`.
Their direct dynamic dependencies are bounded to:

- x86_64: `ld-linux-x86-64.so.2`, `libc.so.6`, `libgcc_s.so.1`, `libm.so.6`;
- aarch64: `libc.so.6`, `libgcc_s.so.1`, `libm.so.6`.

No `libstdc++`/GLIBCXX dependency was reported. Runtime graphical and
Accessibility components such as Xvfb, D-Bus, AT-SPI or Weston remain
environment dependencies rather than ELF dependencies or bundled daemons.

Archive validation passed these gates:

- one top-level directory and no duplicate member;
- no absolute path or `..` traversal;
- no symlink, hard link or special member;
- no group/world-writable member;
- all required binaries, helpers, metadata and smoke resources present;
- ABI report reproduced from the extracted archive;
- no recorded developer checkout path or test secret leakage;
- no `.DS_Store`, AppleDouble or `__MACOSX` host metadata; and
- archive, per-artifact and assembled-manifest checksums valid.

## 4. Package architecture qualification

| Architecture | Result | Evidence boundary |
| --- | --- | --- |
| aarch64 GNU/Linux | **QUALIFIED** | Native arm64 Docker execution on the recorded macOS/OrbStack host; Ubuntu 22.04 build/install and Ubuntu 24.04 environment integration |
| x86_64 GNU/Linux | **QUALIFIED WITH EXPLICIT EMULATION LIMITATION** | Ubuntu 22.04 amd64 container executed through OrbStack/Docker platform emulation on an arm64 host; no native x86_64 hardware campaign |

The x86_64 result is not build-only: its ELF binaries executed, release smoke
ran, Managed setup/Doctor/stop ran, a semantic smoke task ran, and both default
and space-containing prefix installs/uninstalls completed. It nevertheless
does not claim native x86_64 hardware, performance, all distributions or all
environment integrations. The aarch64 package supplied the full Docker, SSH,
Managed and Wayland integration evidence.

## 5. Fresh installation and helper discovery

Both archives passed the same isolated, non-root, fresh-home flow:

- default `$HOME/.local` install;
- an absolute prefix containing spaces;
- invocation from an unrelated working directory;
- installed main binary, required private inspector, Managed helper, optional
  same-host modality helper and exact-file uninstaller discovery;
- Inspector and installed semantic smoke independent of the checkout and
  Cargo target directory;
- `gui2tui setup persistent`, Managed Doctor and explicit stop;
- refusal to uninstall while a Managed descriptor remained active; and
- safe uninstall preserving an unrelated prefix file and XDG configuration.

The installer did not require root, modify a shell profile, install a system
service or change another application. The uninstaller removed only
manifest-owned files whose ownership, type and hash remained valid.

## 6. Defects found and fixes

### Installed uninstaller prefix inference

The installed `libexec/gui2tui/uninstall-user` inferred its default prefix by
walking three parents, yielding the parent of the actual prefix. This broke the
documented no-argument uninstall path. Commit `375d4d3` changes the inference
to two parents. A real installed-path regression proved the default inference,
exact removal and unrelated-file preservation. The failure was fail-closed;
no broad directory was deleted.

### Checkout-dependent archive contents

`package-linux.sh` copied the entire local `docs` directory and therefore
included ignored macOS `.DS_Store` files. Commit `d589332` packages only
version-controlled documentation and makes release validation reject common
host metadata. The previous archive was rejected by the new gate and the
final archives contain no such entries.

### Qualification-only defects

The live evidence work also fixed bounded harness issues: user identity in
containers, an external-handler check key, exact bundle mount counting, PTY
JSON tail parsing, package/harness identity separation and XWayland geometry
sampling before the controlled descendants were realized. These did not alter
the product semantic/runtime behavior. The final XWayland run first completed
the real semantic workflow, then obtained the independent geometry evidence.

## 7. Doctor and failure recovery

Doctor was run from the installed binaries on both package architectures. The
following were directly constructed and passed:

| Condition | Result |
| --- | --- |
| No inherited Session D-Bus | `session-bus=FAIL`; AT-SPI/application rows explicitly not probed |
| Session D-Bus reachable, no `org.a11y.Bus` | session passes; Accessibility bus fails; registry/app rows remain not probed |
| Managed descriptor missing | explicit Managed selection fails with setup guidance |
| Managed supervisor stopped | stale descriptor is refused; no Desktop fallback |
| Managed descriptor invalid | invalid JSON is refused |
| Managed descriptor permissions unsafe | non-private descriptor is refused |
| Managed bus address unreachable | selected Managed topology remains explicit; session bus fails without leaking the address |
| Registry reachable, zero applications | registry passes; application count warns; not a connection failure |
| Required inspector non-executable | installation check fails |
| Optional modality helper non-executable | feature-local warning; core operation is not failed |
| Configured text handler unavailable | feature-local warning; the handler is not executed |
| Valid interactive PTY | TTY, `TERM`, UTF-8 and readable size pass |
| Interactive `TERM=dumb` | terminal type fails |
| Interactive non-UTF-8 locale | locale fails |
| Interactive zero-size PTY | terminal size fails |

Doctor deliberately does not traverse or mutate an application's controls.
With an application present, it reports application semantic sufficiency as
`NOT CHECKED`; a missing action or control is an application Accessibility
limitation, not deployment corruption. The narrower case where
`org.a11y.Bus` returns an address but the registry then dies was not separately
forced in this package campaign. Existing error classification remains, but
that exact injected failure is **NOT TESTED live** here.

Doctor also leaves raw mode, alternate screen and enhanced keyboard protocols
unprobed to avoid changing terminal state. Actual terminal acquisition,
navigation and restoration were instead exercised by the real TUI campaigns.

## 8. Managed Xvfb package integration

The aarch64 archive was installed into the unprivileged Ubuntu 24.04 live
container and completed the existing Managed qualification path:

- current-user Managed Xvfb, Session D-Bus and AT-SPI setup;
- installed helper discovery and Doctor;
- fresh registry enumeration and explicit application selection;
- real Action/selection/text tasks and fresh authoritative readback;
- dynamic scene refresh and manual continuation;
- external Vim handoff, public AT-SPI writeback and readback;
- controlled application/session replacement with old authority refused;
- terminal restoration; and
- exact Managed stop, owned-process cleanup and safe uninstall.

No descriptor, DISPLAY, PID or application name supplied application
authority. Container/session replacement required a fresh process, registry
enumeration and explicit current selection.

## 9. Docker Headless package integration

**QUALIFIED on the recorded aarch64 topology.** The exact aarch64 archive ran
inside an unprivileged Ubuntu 24.04 container without a host display socket,
host D-Bus credential, graphical device, host PID namespace or privileged
mode. A real `docker exec -it` PTY completed application selection, semantic
operation, authoritative readback, dynamic refresh, navigation, resize and
terminal restoration.

Container restart rejected the old Managed descriptor and did not restore old
application authority. A new session and fresh application selection completed
a new verified operation. This evidence does not create or qualify a
production OCI image and does not cover x86_64 container integration.

## 10. Same-host SSH package integration

**QUALIFIED on the recorded aarch64 topology.** A real loopback-only OpenSSH
client/server connection and interactive PTY entered the same Ubuntu container
that hosted GUI2TUI, Managed Xvfb, D-Bus/AT-SPI and the GUI applications. It
used a temporary test identity, no host private key, no public listener, no
X11 forwarding and no Remote Companion.

The installed package passed Doctor, explicit Managed selection, fresh
application selection, Tab/Shift+Tab/arrows/Enter, observed
F6/Shift+F6/Ctrl+Tab/Ctrl+Shift+Tab sequences, Command Palette operation,
semantic mutation/readback, dynamic continuation, normal exit and terminal
restoration. Vim received the real SSH PTY exclusively, edited a private
candidate, returned control, and the current exact target passed conflict,
ticket and full authoritative readback checks.

Controlled abrupt TUI and handler transport loss terminated the GUI2TUI
process while preserving the separately owned Managed session. Reconnection
created a fresh process and application selection; no old process, locator or
operation authority was restored. This is one OpenSSH/client topology, not a
promise for every SSH client or recovery of a destroyed remote PTY.

## 11. Native Wayland and XWayland package integration

**QUALIFIED WITH EXPLICIT LIMITATIONS on the recorded aarch64 topology.** The
exact aarch64 package ran as an unprivileged user in Ubuntu 24.04 with Weston
13.0.0 headless/Pixman, XWayland 23.2.6, AT-SPI 2.52.0, GTK 4.14.5 and Qt
6.6.1. No complete desktop, host display socket, DRM/GPU device or privileged
container was required.

Native GTK4 and Qt6 identities were established by real Wayland protocol
evidence; the XWayland GTK4 identity was established by the exact Weston-owned
XWayland process, controlled X window and matching fixture PID. Those facts
were validation evidence only. Fresh AT-SPI enumeration and explicit current
selection established product authority.

Native and XWayland real TUI tasks passed public semantic operation, fresh
authoritative readback, dynamic refresh, terminal navigation/resize and normal
restoration. Native Vim handoff/writeback/readback passed. Application and
compositor/session replacement refused old locators and required fresh
enumeration/selection. GTK's collapsed global screen origins were rejected and
created zero spatial topology relations while semantic actions remained
usable; differentiated native Qt geometry remained valid presentation
evidence.

The result does not qualify other compositors, distributions, architectures,
full desktop Wayland, Qt-over-XWayland or Wayland over SSH. Wayland static
visual capture remains explicitly unsupported/deferred; it was not replaced
with screenshots, OCR or compositor-private semantics.

## 12. Representative semantic and runtime regression

The installed package evidence covered representative existing capabilities,
not the complete v0.3-v0.6 historical matrix:

- exact current application selection and fresh generation;
- Action/Toggle and selection/value smoke paths;
- modal scope enter/exit;
- dynamic refresh and current-scene continuation;
- stale locator/command refusal after application/session replacement;
- qualified complete plain-text external edit with conflict refusal;
- independent full authoritative readback;
- no PasswordText exposure;
- no backing-file mutation;
- terminal single owner/reader during external handoff; and
- normal and bounded disconnect cleanup.

No non-idempotent operation was automatically replayed. Events remained
wakeup/invalidation signals, and current public Accessibility reads remained
authoritative.

## 13. Environment qualification matrix

| Environment / capability | Qualification | Actual evidence | Missing evidence |
| --- | --- | --- | --- |
| x86_64 package/build/ABI | **QUALIFIED WITH EXPLICIT EMULATION LIMITATION** | Ubuntu 22.04 amd64 container execution, extracted smoke, fresh install/Doctor/Managed setup/semantic smoke/uninstall | Native x86_64 hardware and full environment integration |
| aarch64 package/build/ABI | **QUALIFIED** | Native arm64 Ubuntu containers, full extracted/fresh-install checks | Other distributions/hardware breadth |
| Managed Xvfb | **QUALIFIED** | aarch64 package, Ubuntu 24.04, full lifecycle/task/readback/handler/cleanup | x86_64 full live campaign; broader distributions |
| Docker Headless + interactive PTY | **QUALIFIED** | unprivileged Ubuntu 24.04 aarch64, exact package | x86_64 and broader OCI engines/images |
| Same-host SSH -> Managed | **QUALIFIED** | real loopback OpenSSH PTY, aarch64 package, Vim and disconnect/reconnect | SSH-client breadth; x86_64 integration |
| Native Wayland Headless | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Weston 13/Pixman, native GTK4/Qt6, aarch64 package | other compositors/architectures/distributions/toolkits |
| XWayland Headless | **QUALIFIED WITH EXPLICIT LIMITATIONS** | XWayland 23.2.6 + controlled GTK4, aarch64 package | Qt-over-XWayland and broader environments |
| Linux native VT/TTY | **NOT TESTED** | none; PTY evidence is not substituted | real virtual-console run |
| Ordinary local X11 desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | none; Xvfb is not substituted | real desktop session |
| Ordinary full-desktop Wayland | **NOT TESTED / OPTIONAL COMPATIBILITY** | none; Weston headless is not substituted | real desktop session |
| SSH -> existing Desktop | **NOT TESTED / OPTIONAL COMPATIBILITY** | none | same-user real desktop plus SSH PTY |
| Wayland over real SSH PTY | **NOT TESTED** | local Docker PTY and separate SSH/Xvfb evidence are not combined | one real SSH-to-Wayland flow |
| Wayland static capture | **UNSUPPORTED / DEFERRED** | explicit diagnostic | separately authorized public protocol design, if ever required |
| Cross-host Remote Companion | **UNSUPPORTED / DEFERRED POST-1.0** | no implementation | separate product/architecture authorization |

## 14. Security, privacy and genericity audit

The package and environment work added no semantic backend, application or
toolkit branch, session/deployment manager, daemon, updater, Remote Companion,
cross-host protocol or runtime identity. It used no DOM/CDP, UNO, private
toolkit/compositor API, OCR, screenshot semantics, keyboard/mouse injection,
coordinate click or fuzzy target identity.

Session choice remained transport only. Authority remained
`RuntimeSessionId` + `ApplicationGenerationId` + exact `BackendLocator` +
current `InteractionScope` + current capability + operation ticket. Password
content remained excluded. External candidates and runtime directories kept
the existing 0700/0600 ownership contract. Doctor reports contained no raw bus
address, application text, password, candidate payload or credential.

## 15. Quality and evidence commands

The phase ran:

- dual-platform `tests/live/v07e_package_qualification.sh`;
- per-archive `scripts/validate-release.sh --smoke`;
- `scripts/assemble-release.py` and SHA-256 verification;
- both-architecture `tests/live/v07e_package_install.sh`;
- package-driven v0.7C Headless/Docker/SSH live qualification;
- package-driven v0.7D Native-Wayland/XWayland live qualification;
- shell syntax, Python compile and archive hygiene checks; and
- the phase-close Rust, documentation and diff matrix recorded in the final
  repository close.

One attempted `python3 -m unittest tests.live.test_release_assembly` command
used a non-package import path and failed before loading tests. It is not
reported as a test failure or pass; the correct file-based test command was
run during phase close.

All test containers, temporary port mappings and labelled qualification images
were removed by their bounded harness cleanup. No host service, firewall,
global SSH configuration, personal Managed session or user installation was
modified. The internal `/tmp` package/evidence directories were retained for
the phase audit; they are not running resources or repository content.

## 16. Remaining issues and boundaries

- Open P0: none.
- Open P1: none.
- Open product P2: none reproduced by this phase.
- Evidence limitations: native x86_64 hardware, Linux virtual-console TTY,
  ordinary X11/Wayland desktop, SSH -> existing Desktop, Wayland over SSH,
  other distributions/compositors/toolkits and broader terminal/SSH client
  matrices remain outside the recorded proof.
- Doctor limitation: the exact injected state “Accessibility address returned
  but registry subsequently unavailable” was not live-constructed; Doctor's
  source path remains bounded and non-mutating.
- Application limitation: an application's missing public Accessibility
  capability remains an honest semantic limitation, not a deployment defect.

These evidence gaps are not silently downgraded, and they are not product
P0/P1 without a reproduced contract violation in a claimed environment.

## 17. Architecture conclusion

The existing package scripts, user-prefix installer, CLI, Doctor, explicit
session selection, Managed helper, public AT-SPI backend and v0.6 runtime model
were sufficient. No DeploymentManager, EnvironmentRegistry, SessionManager or
new semantic/runtime framework was required.

The phase demonstrates repeatable no-root archives and installs for both
target architectures under the stated execution boundaries, and it separates
deployment failures from AT-SPI infrastructure failures, zero applications
and unassessed application semantics. It preserves 0.7A session selection and
all application authority/runtime continuity invariants.

## 18. v0.7 roadmap and next authorization

- Discovery: complete.
- 0.7A: complete / validated.
- 0.7B: complete / validated.
- 0.7C: complete / validated with explicit limitations.
- 0.7D: complete / qualified with explicit limitations.
- 0.7E: **complete / qualified with explicit limitations**.
- v0.7 Milestone Qualification: **not started / not authorized**.

The only recommended next authorization is a separate **v0.7 Milestone
Qualification / Close** that audits the combined 0.7A-0.7E evidence and final
support wording. It must not silently add missing environment campaigns or
start v1.0 integration.

## 19. Next Codex context

Start by reading `AGENTS.md`, `docs/project-guide.md`,
`docs/planning/roadmap-to-1.0.md`,
`docs/planning/v0.7-deployment-environment.md`,
`docs/planning/v0.7-headless-first-contract.md`,
`docs/planning/v0.7-roadmap.md`, the v0.6 milestone handoff, and all v0.7
phase handoffs including this file. The branch is
`v0.7/deployment-environment-completeness`; obtain the exact current HEAD with
`git rev-parse HEAD` because this report is followed by its documentation
commit. v0.4-v0.6 are internally milestone-qualified. 0.7A/0.7B are complete;
0.7C and 0.7D are complete with their recorded environment limitations; 0.7E
qualified exact-source `eb5841f` x86_64/aarch64 internal packages, with native
aarch64 and emulated-runtime x86_64 boundaries, plus aarch64 Managed, Docker,
real SSH, Native Wayland and XWayland integration. Package version remains
0.3.0 and no v0.7 release/tag exists. Linux native VT, ordinary desktop,
Wayland-over-SSH and broader architecture/environment matrices remain untested.
Session selection grants transport only; fresh registry enumeration and
explicit application selection establish a new generation, while exact
locator/scope/capability/ticket/readback rules, privacy and terminal ownership
remain unchanged. v0.7 Milestone Qualification, v1.0 integration, RC/tag and
release work are not authorized. Do not expand the task without explicit user
authorization.
