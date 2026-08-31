# Deployment baseline

| Deployment | v0.1 status | Requirement |
| --- | --- | --- |
| Linux headless terminal / SSH | SUPPORTED | Reach the same user's existing GUI AT-SPI session; GUI application still needs its own graphical environment. No broker required. |
| Linux same-host graphical viewer | SUPPORTED | Explicit private socket, configured handler, authorization. Local-path handoff, not remote desktop. |
| Remote companion | Architecture-ready; PRODUCTION TRANSPORT NOT IMPLEMENTED | No cross-host authentication/session protocol shipped. |
| macOS | Build/development verification only | Linux needed for live AT-SPI use. |
| New TTY attaches to existing runtime | NOT IMPLEMENTED | Same-process/same-PTY detach/resume is distinct and verified. |

Headless does not mean launching GUI programs without any display server. It means the terminal
frontend needs no graphical viewer. For tests, Xvfb supplies the application's graphical environment;
normal sessions can use their existing desktop. Wayland semantic access is separate from static capture;
Wayland capture is NOT IMPLEMENTED, and no compositor is bundled.

No viewer endpoint means no endpoint wait on startup. F4 resource tasks remain reference-first;
materialization on the GUI2TUI host is independent of transport. A captured region is labelled
RenderedSnapshot, never an original embedded resource. Only explicit user requests capture one frame.

Use a private current-user runtime directory for broker sockets, artifacts and diagnostic logs.
Artifact ownership/leases prevent one live session's files being scavenged by another. Running as
root is unnecessary and does not solve access to another user's session bus.
