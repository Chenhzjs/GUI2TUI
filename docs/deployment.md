# Headless-first deployment and environment direction

The following table distinguishes the user-approved 1.0 development target
from current qualification. A target is not a formal support claim.

| Deployment | 1.0 direction | Current qualification |
| --- | --- | --- |
| GUI2TUI-managed Xvfb | Core support target | QUALIFIED on Ubuntu 24.04 aarch64 with an installed current-source binary: create/reuse/stop/recreate, operation/readback, terminal/handler lifecycle and cleanup passed. 0.7E package integration remains. |
| Local Linux TTY -> Managed | Core candidate | Managed local PTY passed; real Linux virtual-console TTY is NOT TESTED. |
| Same-host SSH -> Managed | Core candidate; highest-priority evidence gap | NOT TESTED end to end: the 0.7C environment had no SSH server/listener or real interactive PTY. |
| Docker/OCI Headless | Important separately qualified candidate | NOT TESTED; no container image or deployment is currently qualified. |
| Local Linux X11, same user/session | Optional compatibility | NOT TESTED in a real local desktop; controlled Xvfb evidence is not a substitute. |
| Same-host SSH -> existing Desktop | Optional compatibility | NOT TESTED; it is not a Headless-core prerequisite. |
| Native Wayland | Evidence-driven candidate | NOT TESTED in a real native session; 0.7D decides status from public semantics and safe geometry degradation. |
| XWayland | Separate evidence-driven candidate | NOT TESTED; native Wayland results do not qualify this row or vice versa. |
| Headless Wayland | Feasibility candidate | NOT TESTED; no compositor is bundled or qualified. |
| Local TUI + GUI on another host | Deferred until after 1.0 | No Remote Companion, cross-host semantic transport, authentication, event/cache sync, or remote backend is implemented. |
| Linux same-host graphical viewer | Existing optional modality | Explicit private socket, configured handler and local authorization; this is not Remote Companion. |
| macOS/Windows GUI backend | Outside the Linux AT-SPI 1.0 baseline | macOS remains build/development verification only; no GUI semantic backend. |
| New TTY attaches to existing runtime | Not implemented | Same-process/same-PTY detach/resume is distinct and verified. |

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
the application's qualified background graphical environment; ordinary
sessions may use an existing desktop as optional compatibility. Native
Wayland, XWayland and Headless Wayland require separate evidence. Wayland
static capture is NOT IMPLEMENTED, and no compositor is bundled. Missing
global Wayland geometry must degrade presentation and cannot by itself
invalidate otherwise working semantic interaction.

No viewer endpoint means no endpoint wait on startup. F4 resource tasks remain reference-first;
materialization on the GUI2TUI host is independent of transport. A captured region is labelled
RenderedSnapshot, never an original embedded resource. Only explicit user requests capture one frame.

Use a private current-user runtime directory for broker sockets, artifacts and diagnostic logs.
Artifact ownership/leases prevent one live session's files being scavenged by another. Running as
root is unnecessary and does not solve access to another user's session bus.

The current 0.7C evidence is intentionally asymmetric: Managed Xvfb passed in
the named Ubuntu environment, while real Local Linux TTY, SSH -> Managed,
ordinary local X11 and SSH -> existing Desktop remain NOT TESTED. A local PTY,
an SSH-origin environment variable or a controlled Desktop Xvfb cannot
substitute for those named environments. Under the revised Headless-first
contract, SSH -> Managed and Local TTY -> Managed are core evidence gaps;
ordinary desktop rows are optional compatibility. Docker/OCI and all Wayland
rows also remain NOT TESTED.

See the
[Headless-first product and environment contract](planning/v0.7-headless-first-contract.md)
for the current classification and revised phase exits.
