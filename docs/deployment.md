# Deployment and environment direction

The following table distinguishes the user-approved 1.0 development target
from current qualification. A target is not a formal support claim.

| Deployment | 1.0 direction | Current qualification |
| --- | --- | --- |
| Local Linux X11, same user/session | Core support target | Real local-desktop install-to-exit qualification remains for 0.7C; Xvfb evidence is not a substitute. |
| GUI2TUI-managed Xvfb | Core support target | Explicit selection and bounded descriptor handling are validated; full deployment/recovery qualification remains for 0.7C/0.7E. |
| Native Wayland | Priority validation target; seek inclusion | NOT TESTED in a real native session; 0.7D decides status from public semantics and safe geometry degradation. |
| XWayland | Priority validation target; seek inclusion separately | NOT TESTED; native Wayland results do not qualify this row or vice versa. |
| Same-host SSH TUI | Seek inclusion | Connectivity evidence exists, but complete SSH PTY/session lifecycle qualification remains for 0.7C. |
| Local TUI + GUI on another host | Deferred until after 1.0 | No Remote Companion, cross-host semantic transport, authentication, event/cache sync, or remote backend is implemented. |
| Linux same-host graphical viewer | Existing optional modality | Explicit private socket, configured handler and local authorization; this is not Remote Companion. |
| macOS/Windows GUI backend | Outside the Linux AT-SPI 1.0 baseline | macOS remains build/development verification only; no GUI semantic backend. |
| New TTY attaches to existing runtime | Not implemented | Same-process/same-PTY detach/resume is distinct and verified. |

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

Headless does not mean launching GUI programs without any display server. It means the terminal
frontend needs no graphical viewer. For tests, Xvfb supplies the application's graphical environment;
normal sessions can use their existing desktop. Wayland semantic access is
separate from static capture; Wayland capture is NOT IMPLEMENTED, and no
compositor is bundled. Missing global Wayland geometry must degrade
presentation and cannot by itself invalidate otherwise working semantic
interaction.

No viewer endpoint means no endpoint wait on startup. F4 resource tasks remain reference-first;
materialization on the GUI2TUI host is independent of transport. A captured region is labelled
RenderedSnapshot, never an original embedded resource. Only explicit user requests capture one frame.

Use a private current-user runtime directory for broker sockets, artifacts and diagnostic logs.
Artifact ownership/leases prevent one live session's files being scavenged by another. Running as
root is unnecessary and does not solve access to another user's session bus.
