# Headless-first deployment

GUI2TUI v1.1 is designed for users who have a local TTY, SSH PTY or container
terminal while the target GUI application runs in a same-host graphical and
Accessibility session. The user does not need a visible GNOME/KDE desktop,
VNC or RDP. The application may still require a background Xvfb or qualified
Wayland compositor.

The exact v1.1 contract is frozen in the
[v1.1 Final Support Matrix](validation/v1.1/final-support-matrix.md). The
v1.0 matrix remains historical evidence.

## Supported deployment topologies

| Topology | v1.1 status |
| --- | --- |
| Managed Xvfb | Supported within the recorded unprivileged arm64 topology |
| Docker/OCI interactive PTY | Supported with the recorded limitations; no production container image is supplied |
| Same-host SSH -> Managed | Supported with the recorded loopback OpenSSH/PTy limitations |
| Headless Native Wayland | Supported with explicit Weston/Pixman and geometry limitations |
| Headless XWayland | Supported with the recorded Weston/XWayland limitations |
| GNU/Linux aarch64 | Qualified with native package/live evidence |
| GNU/Linux x86_64 | Package and emulated-runtime qualified; native hardware not qualified |
| Linux native VT | Not qualified |
| Ordinary local X11/Wayland desktop | Optional compatibility, not qualified |
| Wayland over SSH | Not qualified |
| Cross-host Remote Companion | Post-1.0 |

One PTY, compositor or display-server result never broadens another named
topology. See the support matrix for the exact distribution, architecture,
compositor and terminal boundaries.

Raw Enter is a capability-specific exception to the semantic-only baseline:
it is available only through the qualified X11 NativeInputBackend after exact
window and Accessibility focus verification. Semantic AT-SPI operations do
not depend on Raw Enter and remain the supported path on qualified Wayland.

## Session selection

Choose the connection environment before application discovery:

```bash
gui2tui --session desktop doctor
gui2tui --session desktop

gui2tui setup persistent
gui2tui --session managed doctor
gui2tui --session managed
```

`desktop` uses the session environment inherited by the current process.
`managed` requires a valid current-user-owned descriptor created by
`gui2tui setup persistent`; it never silently falls back or creates a missing
session. Omitting `--session` retains compatibility behavior: reuse a valid
managed descriptor when present, otherwise use the inherited environment.

Session selection only chooses the Accessibility environment. It does not
authorize an application. GUI2TUI enumerates the current registry and the user
must explicitly select an application.

## Managed Xvfb

Persistent Managed mode creates a private Xvfb, session D-Bus and AT-SPI
environment for future terminals:

```bash
gui2tui setup persistent
gui2tui setup status
gui2tui --session managed doctor
```

Register and launch an application without invoking a shell:

```bash
gui2tui --session managed app add mousepad
gui2tui --session managed launch mousepad
gui2tui --session managed
```

The application must be installed and must expose an AT-SPI application in that
session. GUI2TUI cannot manufacture semantics for an inaccessible program.
Use `gui2tui setup restart` to recreate the session and `gui2tui setup stop`
to stop it and remove its active descriptor. Strict Snap confinement may block
access to a private Managed bus; use a normal desktop session or a non-Snap
build rather than weakening the sandbox.

For an isolated shell, use:

```bash
gui2tui setup temporary
```

Temporary mode removes its private session state when the child command exits.

## Docker and SSH

The supported Docker contract is an unprivileged interactive PTY in the
recorded Ubuntu 24.04 arm64 topology. It is not a general production image or
a claim for every OCI runtime. The supported SSH contract is SSH into the same
host's Managed session; it is not cross-host semantic transport and does not
cover SSH into an existing desktop or Wayland-over-SSH.

The terminal must provide UTF-8, cursor and alternate-screen support. The
target GUI application's display/runtime remains on the same host as the
Accessibility session.

## Headless Wayland and XWayland

The qualified Wayland evidence uses Weston 13 headless/Pixman, Ubuntu 24.04
arm64 and controlled GTK4/Qt6/XWayland applications. Geometry is presentation
evidence only. Collapsed or incomplete geometry degrades layout; it never
creates operation authority. Other compositors, distributions, ordinary full
Wayland desktops and Wayland over SSH are outside the v1.1 claim. Wayland
static capture is not implemented and no compositor is bundled. Raw Enter is
reported unavailable where the qualified native backend is absent.

## Installation and upgrade

Use the official v1.0.0 archive for the stable public baseline, or the internal
v1.1.0-rc.1 archive for qualification. Both use the same unprivileged
installer, which refuses
pre-existing targets, symlinks and unsafe manifests rather than overwriting
files. To replace an existing installation:

1. Stop the owned Managed session.
2. Run the installed exact-file uninstaller.
3. Preserve configuration, runtime/recovery data and unrelated prefix files.
4. Install the selected archive into the same prefix.
5. Run `gui2tui doctor` and a representative semantic smoke before normal use.

Do not recursively delete `$HOME/.local` or use `sudo` for installation.

## Diagnosis and recovery

Run:

```bash
gui2tui doctor
gui2tui doctor --json --report ./gui2tui-report.json
```

Doctor checks installation, terminal basics, selected session D-Bus,
Accessibility services, registry and application presence without reading GUI
content. A registry with zero applications is a warning, not proof that the
session is unavailable. See [Troubleshooting](troubleshooting.md) for safe
recovery by symptom.
