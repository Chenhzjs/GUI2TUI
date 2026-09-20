# GUI2TUI v0.7 Phase 0.7D — Wayland and XWayland Qualification Handoff

## 1. Status

- Starting HEAD: `2864bdf13ae847d5a870f52ce6601a59747a52c4`
- Product source under live qualification:
  `5df49404dec3e59f97b3c074f230d597043015d5`
- Branch: `v0.7/deployment-environment-completeness`
- Production changes: one generic spatial-evidence safety fix
- P0: 0 open
- P1: 0 open; one reproduced presentation-safety defect was fixed and
  qualified
- Overall: **COMPLETE / QUALIFIED WITH EXPLICIT LIMITATIONS**

This phase answers the authorized 0.7D question. It does not qualify a package
archive, every compositor/toolkit, a visible Wayland desktop, Wayland over SSH,
or v0.7 as a milestone. Phase 0.7E remains separately unauthorized.

## 2. Environment identity

The successful live campaign used:

- macOS 26.6.2 arm64 as the Docker/OrbStack host only;
- an unprivileged Ubuntu 24.04 aarch64 container;
- glibc 2.39;
- Weston 13.0.0, headless backend and Pixman software renderer;
- Xwayland 23.2.6;
- AT-SPI 2.52.0;
- GTK 4.14.5;
- PyQt/Qt 6.6.1; and
- the user-prefix-installed `gui2tui 0.3.0` built from the product source
  commit above.

The validation container was not privileged, shared no host PID namespace,
mounted no host directory or credential, published no port, used no host X11
or Wayland socket, and received no graphical device. GUI2TUI, Weston, the
private session D-Bus, AT-SPI and fixtures ran as the ordinary `gui2tui` user.
The Dockerfile is a reproducible live fixture, not a production image.

## 3. Headless Wayland topology

Weston started with its real `headless` backend, Pixman renderer, a private
mode-0700 `XDG_RUNTIME_DIR`, a private Wayland socket, and XWayland enabled.
It required no display manager, GNOME/KDE/XFCE, DRM device, GPU, seat,
systemd-logind, host display socket, VNC or RDP. `wayland-info` observed the
real compositor globals and `xdg_wm_base` on the private socket.

A private session D-Bus and `org.a11y.Bus` supplied the Accessibility bus and
registry. GUI2TUI used explicit `--session desktop` because this was the
current inherited private Wayland session; it did not use or create a Managed
Xvfb descriptor. Doctor reported Wayland topology, a reachable session bus,
Accessibility bus, registry and applications. Session choice still supplied
transport only; each GUI application was freshly enumerated and explicitly
selected.

## 4. Native Wayland identity

The GTK fixtures ran with `GDK_BACKEND=wayland`, no `DISPLAY`, and the private
`WAYLAND_DISPLAY`. The Qt fixture ran with `QT_QPA_PLATFORM=wayland`, no
`DISPLAY`, and the same socket. Bounded public Wayland client traces observed
`wl_display`, registry binding and `xdg_wm_base`; merely seeing an environment
variable was not accepted as identity evidence.

Both `gui2tui-v05a-gtk-selection`, `gui2tui-live-fixture` and
`gui2tui-qt-fixture` appeared in the private AT-SPI registry. Application
names and protocol identity were evidence only; exact current registry
selection established application authority.

## 5. XWayland identity

The XWayland GTK fixture ran with `GDK_BACKEND=x11`, the Weston-published
`DISPLAY`, and no `WAYLAND_DISPLAY`. Qualification additionally observed:

- the Weston-owned `/usr/bin/Xwayland` process on that exact display;
- the controlled window in the X window tree; and
- `_NET_WM_PID` equal to the exact controlled fixture PID.

This proves the tested application was an XWayland client rather than a native
Wayland client, Xvfb client, or independent Xorg session. These observations
did not enter GUI2TUI production identity or authority.

## 6. Native Wayland semantic evidence

Through a real Docker interactive PTY, the installed GUI2TUI TUI:

1. explicitly selected the current private Wayland session and controlled GTK
   application;
2. rendered the semantic scene;
3. accepted Tab, Shift+Tab, Left, Right and Enter navigation;
4. opened the Command Palette;
5. invoked `Reorder current items` through its advertised public AT-SPI
   action;
6. freshly read back `Structure: reordered; selected Gamma`;
7. rendered that current state and manually invoked `Reset current items`;
8. freshly read back `Selected: Alpha`;
9. survived a real PTY resize; and
10. exited with canonical input and echo restored.

The native Qt fixture exposed its current public tree without revealing its
PasswordText value. GUI2TUI invoked `Activate safely` through the exact
advertised action and a separate fresh tree read confirmed `Status:
activated`.

The native GTK live fixture exposed a modal dialog through current public
semantics. Scope analysis identified `GTK Fixture Dialog` as the active
`ModalDialog`; closing it retired that scope. The later application restart
rejected the old exact locator before any fresh work.

## 7. XWayland semantic evidence

The separately identified XWayland GTK fixture completed the same real-TUI
reorder/reset workflow, dynamic scene refresh, authoritative readback,
keyboard navigation, PTY resize and normal terminal restoration. Stopping the
application made its old locator unusable. XWayland and native Wayland both
used the existing AT-SPI semantic backend; no runtime or semantic branch was
selected from display protocol or toolkit identity.

Qt-over-XWayland was not run. The XWayland qualification is therefore bounded
to the controlled GTK 4 application and recorded software stack.

## 8. Geometry and safe fallback

The phase reproduced a real genericity/safety defect. Native GTK and XWayland
GTK returned positive AT-SPI screen extents for many objects while collapsing
virtually all origins to `(0, 0)`. The existing spatial layer classified each
individual rectangle as consistent and could therefore manufacture overlap or
containment relationships that the public evidence did not prove.

The fix is generic and display-neutral. When a bounded sample contains at
least eight consistent screen-coordinate rectangles, at least 75% share one
origin, and those entries contain at least three different sizes, the spatial
layer marks that screen evidence inconsistent. It does not inspect toolkit,
application, compositor, process, title or environment identity.

Final live evidence showed the collapsed GTK samples with zero accepted
geometry successes, inconsistent trust and zero topology anchors, comparable
pairs or relations. The semantic `ControlSurface` remained navigable and its
qualified actions remained usable. Native Qt returned differentiated screen
extents and retained consistent spatial evidence. Geometry therefore changed
presentation evidence only and never role, identity, scope, capability or
operation authority.

## 9. External handler and terminal lifecycle

On a fresh native-Wayland GTK application generation, a configured direct-argv
Vim handler edited the qualified complete non-password `TextView` candidate
through a real PTY handoff. GUI2TUI retired its input reader, Vim owned the
terminal, the handler exited, GUI2TUI reacquired one reader, validated current
generation/locator/scope/ticket/conflict state, wrote through public
`EditableText`, and freshly read back `Wayland qualified edit`. The successful
candidate was removed and terminal canonical/echo state was restored.

After the earlier modal lifecycle, an old text target was safely refused
rather than guessed or rebound. The qualification deliberately retired that
application, proved an old locator unusable, then used fresh enumeration and
selection for the handler test. XWayland external-handler handoff was not
separately run.

## 10. Compositor, session and application lifecycle

The live campaign stopped and rebuilt the private compositor/session, then
proved:

- locators from the old native application were rejected;
- locators from the old XWayland application were rejected;
- the old scene, binding and operation authority were not restored;
- a new current registry enumeration and explicit application selection were
  required; and
- a new semantic operation and independent authoritative readback succeeded.

Compositor restart was not treated as proof that the D-Bus, registry or
application generation survived. No non-idempotent operation was replayed and
no authority was reconstructed from name, PID, window title, display or
geometry.

## 11. Static visual and external modality boundary

Doctor explicitly reported `Wayland static capture NOT IMPLEMENTED` while the
semantic session remained healthy. The native-X11-only static visual provider
was not replaced with compositor-private capture, OCR, screenshots, or input
injection. This is an explicit External Modality limitation, not a failure of
the qualified public AT-SPI semantic workflow.

## 12. Qualification matrix

| Environment / capability | Qualification | Evidence / limitation |
| --- | --- | --- |
| Existing Managed Xvfb | Historical qualified baseline | Retained from 0.7C; not rerun as Wayland evidence |
| Native Wayland headless startup | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Weston 13 headless/Pixman, Ubuntu 24.04 arm64 only |
| Native Wayland application identity | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Real protocol trace for GTK4 and Qt6 fixtures |
| Native Wayland AT-SPI semantics | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Controlled GTK4 and Qt6 public trees |
| Native Wayland semantic interaction | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Real GTK TUI task; Qt exact action |
| Native Wayland authoritative readback | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Independent current GTK and Qt reads |
| Native Wayland geometry/fallback | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Collapsed GTK rejected; differentiated Qt retained |
| Native Wayland external handler | **QUALIFIED WITH EXPLICIT LIMITATIONS** | GTK complete plain text + Vim, one PTY topology |
| XWayland startup and identity | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Weston XWayland 23.2.6 + exact GTK window/PID evidence |
| XWayland AT-SPI semantics | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Controlled GTK4 public tree |
| XWayland semantic interaction/readback | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Real TUI reorder/reset and fresh reads |
| Wayland/XWayland lifecycle recovery | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Old locators rejected; fresh selection and operation passed |
| Headless terminal operation | **QUALIFIED WITH EXPLICIT LIMITATIONS** | Docker interactive PTY, resize and restoration |
| Wayland static visual capture | **UNSUPPORTED / DEFERRED** | Explicit Doctor result; not part of semantic qualification |
| Wayland over real SSH PTY | **NOT TESTED** | Local Docker PTY cannot substitute |
| Native Linux VT/TTY | **NOT TESTED** | No real virtual console was available |
| Ordinary full desktop Wayland | **NOT TESTED** | Headless Weston is not a GNOME/KDE desktop |

The qualification wording is intentionally bounded. It does not claim broad
compositor, distribution, terminal, toolkit, application or architecture
coverage. Final 1.0 support wording still depends on 0.7E package and
integrated environment qualification.

## 13. Product issue and fix

One product issue was reproduced: collapsed positive screen extents could be
treated as trusted spatial evidence. Commit `5df4940` rejects that generic
pattern and includes one focused unit regression. Commit `35eecc1` adds only a
local type alias required by warnings-denied Clippy; it does not change the
qualified behavior. Native Qt's valid geometry and existing semantics remain
intact. No application/toolkit/Wayland branch was added.

The initial full harness attempts also exposed two validation-only timing or
expectation errors: it assumed one presentation composition name, and it read
modal scope before the asynchronously created surface registered. Those
assertions were corrected to test the actual contracts. A later handler
attempt correctly refused a target after modal churn; a fresh application
generation then passed the independent handler qualification. These were not
masked product failures and no production safety check was weakened.

## 14. Genericity, security and privacy audit

- Semantic data and mutations came only from public AT-SPI.
- No DOM/CDP, UNO, toolkit-private API, compositor-private semantic API, OCR,
  screenshot semantics, GUI input injection or coordinate click was used.
- Fixture/toolkit/display identity existed only in live evidence and never in
  production behavior.
- Geometry remained presentation evidence and created no authority.
- PasswordText remained redacted.
- External candidate ownership and cleanup remained private and bounded.
- No host credential, personal directory, display socket or D-Bus credential
  entered the container.
- No new runtime, SessionManager, Wayland backend, deployment manager, remote
  companion or cross-host protocol was introduced.

## 15. Validation infrastructure and cleanup

The committed bounded live infrastructure is:

- `tests/live/Dockerfile.v07d-wayland`;
- `tests/live/v07d_wayland_container_control.py`;
- `tests/live/v07d_wayland_driver.py`; and
- `tests/live/v07d_wayland_qualification.sh`.

It builds current product source, installs to a user prefix, verifies
non-privileged/no-mount/no-port boundaries, owns exact PIDs, and removes only
its named container and image. The successful run removed its container,
image, compositor, XWayland server, applications, buses and transient runtime
data. Earlier exploratory containers/images were also removed by exact name;
unrelated Docker resources were untouched.

The successful evidence directory was
`/tmp/gui2tui-v07d-wayland-retry3`. It contains only bounded local validation
results and sanitized environment metadata; raw private bus addresses are not
part of this handoff or the repository.

## 16. Tests and quality

Completed checks:

- targeted Rust regression for collapsed screen origins: PASS;
- Python syntax compilation for both live helpers: PASS;
- shell syntax check for the qualification driver: PASS;
- complete v0.7D live campaign: PASS;
- installed Native Wayland and XWayland semantic workflows: PASS;
- exact temporary-resource cleanup: PASS; and
- `cargo fmt --all -- --check`: PASS;
- `cargo check --all-targets --locked`: PASS;
- `cargo test --all-targets --locked`: PASS (300 library tests, 2 inspector
  tests and 5 user-CLI integration tests; 0 failures);
- `cargo clippy --all-targets --locked -- -D warnings`: PASS;
- `python3 scripts/check-docs.py`: PASS (89 files, 307 local links); and
- `git diff --check`: PASS.

The first closing-matrix attempt found only the warnings-denied type-complexity
lint in the new geometry helper. The local alias fix passed the focused test
and full Clippy before the complete matrix above was rerun successfully. No
unexecuted check is reported as passing.

## 17. Architecture conclusion

The current session-D-Bus/AT-SPI/runtime architecture was sufficient for both
tested native Wayland and XWayland environments. It did not require a
Wayland-specific semantic backend or environment manager. A small generic
presentation-evidence rejection was sufficient for unreliable global
coordinates. Transport/session choice remained separate from application
authority, and v0.4-v0.6 generation, scope, ticket, readback, privacy and
terminal-ownership invariants held.

## 18. Roadmap and next boundary

- Discovery: COMPLETE.
- 0.7A: COMPLETE / VALIDATED.
- 0.7B: COMPLETE / VALIDATED.
- 0.7C: COMPLETE / VALIDATED WITH EXPLICIT LIMITATIONS.
- 0.7D: **COMPLETE / QUALIFIED WITH EXPLICIT LIMITATIONS**.
- 0.7E: PLANNED / NOT AUTHORIZED.
- v0.7 Milestone Qualification: NOT STARTED / NOT AUTHORIZED.

The next recommended direction is **0.7E — Package and Environment Contract
Qualification**, only after explicit user authorization. Phase 0.7D does not
authorize package production, milestone close, 1.0 integration, an RC, tag or
release.
