# Headless-first deployment and environment direction

The RC v1.0 support wording is consolidated in the
[Phase 1.0C final support matrix](validation/v1.0/final-support-matrix.md).
The table below remains the detailed evidence boundary; it does not broaden
claims beyond the named topology, architecture and compositor.

The following table distinguishes the user-approved 1.0 development target
from current qualification. A target is not a formal support claim.

| Deployment | 1.0 direction | Current qualification |
| --- | --- | --- |
| GUI2TUI-managed Xvfb | Core support target | QUALIFIED on Ubuntu 24.04 aarch64 with the exact internal 0.7E package: create/reuse/stop/recreate, operation/readback, terminal/handler lifecycle and cleanup passed. |
| Local Linux TTY -> Managed | Core candidate | Managed local PTY passed; real Linux virtual-console TTY is NOT TESTED. |
| Same-host SSH -> Managed | Core candidate | QUALIFIED with the exact aarch64 package on the recorded Ubuntu 24.04 arm64 loopback OpenSSH interactive-PTY topology, with explicit client and disconnect limitations. |
| Docker/OCI Headless | Important separately qualified candidate | QUALIFIED WITH EXPLICIT LIMITATIONS with the exact aarch64 package on one unprivileged Ubuntu 24.04 arm64 live topology; no production image is supplied. |
| Local Linux X11, same user/session | Optional compatibility | NOT TESTED in a real local desktop; controlled Xvfb evidence is not a substitute. |
| Same-host SSH -> existing Desktop | Optional compatibility | NOT TESTED; it is not a Headless-core prerequisite. |
| Native Wayland | Evidence-driven candidate | QUALIFIED WITH EXPLICIT LIMITATIONS with the exact aarch64 package on Ubuntu 24.04 arm64, Weston 13 headless/Pixman and controlled GTK4/Qt6. |
| XWayland | Separate evidence-driven candidate | QUALIFIED WITH EXPLICIT LIMITATIONS with that package/compositor and XWayland 23.2.6 plus controlled GTK4. |
| Headless Wayland | Headless candidate | QUALIFIED WITH EXPLICIT LIMITATIONS with the exact aarch64 package for that non-privileged Weston topology; no compositor is bundled. |
| Local TUI + GUI on another host | Deferred until after 1.0 | No Remote Companion, cross-host semantic transport, authentication, event/cache sync, or remote backend is implemented. |
| Linux same-host graphical viewer | Existing optional modality | Explicit private socket, configured handler and local authorization; this is not Remote Companion. |
| macOS/Windows GUI backend | Outside the Linux AT-SPI 1.0 baseline | macOS remains build/development verification only; no GUI semantic backend. |
| New TTY attaches to existing runtime | Not implemented | Same-process/same-PTY detach/resume is distinct and verified. |

The internal 0.7E package baseline is exact-source commit `eb5841f`. Its
aarch64 archive is qualified on the recorded native arm64 container host. Its
x86_64 archive passed ELF/ABI, extracted smoke, fresh install, installed
Doctor/Managed setup/semantic smoke and safe uninstall through amd64
Docker/OrbStack platform emulation on an arm64 host; native x86_64 hardware
and the full x86_64 environment matrix remain outside that claim. Both
archives reference at most glibc 2.34 and passed the declared glibc 2.35 gate.
They are internal qualification artifacts, not a published v0.7 package. The
combined evidence review closes v0.7 as an internal qualified milestone; it
does not broaden this matrix or authorize v1.0 integration or release.

GUI2TUI's core user may have only a local TTY, SSH PTY or container TTY. The
target GUI application can still require a background X11 display server or
Wayland compositor. Headless-first means no directly interactive graphical
desktop is required for the user; it does not mean the application has no
graphical runtime dependency. A complete visible desktop environment is not a
prerequisite for the Managed path.

Choose the connection environment before application discovery:

```bash
gui2tui --session desktop doctor
gui2tui --session desktop

gui2tui setup persistent
gui2tui --session managed doctor
gui2tui --session managed
```

Desktop selection never imports the managed descriptor. Managed selection
requires the private current-user descriptor and fails on missing, invalid,
stopped, unsafe or unreachable state; it neither creates a session nor falls
back. With no flag, the documented compatibility mode reuses a valid managed
descriptor if present, honors `GUI2TUI_NO_MANAGED_SESSION`, otherwise uses the
inherited desktop environment, and reports the result. Session selection does
not grant application authority: the selected registry is enumerated afresh
and an exact current application must still be chosen.

## Installation and diagnosis foundation

Current source and future bundles use the same unprivileged prefix contract:

```bash
cargo build --release --locked --bins
./scripts/install-user.sh --prefix "$HOME/.local"
"$HOME/.local/bin/gui2tui" --session desktop doctor
```

The installer copies only the main executable, inspector, Managed Headless
helper, optional same-host modality helper and exact-file uninstaller. Private
helpers are resolved relative to the running main executable, not the checkout
or current directory. An owned mode-0600 hash manifest makes removal bounded;
the uninstaller refuses changed files, symlinks and a remaining Managed
descriptor, then preserves configuration, runtime/recovery data and unrelated
prefix entries. It never uses `sudo` or recursively removes a prefix.

Doctor reports installation entry/helper integrity separately from terminal,
session D-Bus, `org.a11y.Bus`, AT-SPI registry and accessible-application
count. A reachable registry with zero applications is a warning, not a
connection failure. Application presence is not semantic-capability
qualification: Doctor performs no control traversal or mutation and reports
application-level semantic sufficiency as NOT CHECKED. A configured complex
text handler is validated as direct argv and executable without starting it or
creating a candidate; an absent handler remains valid.

Headless does not mean launching GUI programs without any display server. It
means the terminal frontend needs no graphical viewer. Managed Xvfb supplies
one qualified background graphical environment; the bounded 0.7D evidence
also qualifies Native Wayland and XWayland semantics on one Weston 13
headless/Pixman topology. Those remain separate rows and do not qualify other
compositors, distributions, architectures or ordinary desktop sessions.
Wayland static capture is NOT IMPLEMENTED, and no compositor is bundled.
Missing or collapsed global Wayland geometry degrades presentation and cannot
by itself invalidate otherwise working semantic interaction.

No viewer endpoint means no endpoint wait on startup. F4 resource tasks remain reference-first;
materialization on the GUI2TUI host is independent of transport. A captured region is labelled
RenderedSnapshot, never an original embedded resource. Only explicit user requests capture one frame.

Use a private current-user runtime directory for broker sockets, artifacts and diagnostic logs.
Artifact ownership/leases prevent one live session's files being scavenged by another. Running as
root is unnecessary and does not solve access to another user's session bus.

The current evidence is intentionally bounded. Managed Xvfb, unprivileged
Docker interactive TTY, real SSH -> Managed, and the recorded Weston
Native-Wayland/XWayland topology have matching live evidence. Real Linux
virtual-console TTY, ordinary local X11, SSH -> existing Desktop, ordinary
full-desktop Wayland and Wayland over SSH remain NOT TESTED. One PTY, display
server or compositor result cannot substitute for another named environment.

See the
[Headless-first product and environment contract](planning/v0.7-headless-first-contract.md)
for the current classification and revised phase exits.
