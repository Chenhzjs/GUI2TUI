# Proposed v1.0 Final Support Matrix

This matrix is the candidate contract for Phase 1.0C qualification. It is
based on the v1.0A/v1.0B and v0.7 evidence; it does not promote an untested
topology by analogy. The final release contract is frozen only by the 1.0C
handoff.

| Topology / capability | Candidate status | Boundary |
| --- | --- | --- |
| Managed Xvfb | Supported within evidence | Unprivileged user-prefix install, private session D-Bus/AT-SPI, recorded Ubuntu 24.04 arm64 topology. |
| Docker/OCI Headless interactive PTY | Supported with explicit limitations | One unprivileged Ubuntu 24.04 arm64 topology; no production container image or broad OCI claim. |
| Same-host SSH -> Managed | Supported with explicit limitations | Recorded loopback OpenSSH interactive PTY into the same host/session; other clients and architectures are outside evidence. |
| Headless Native Wayland | Supported with explicit limitations | Weston 13 headless/Pixman, Ubuntu 24.04 arm64, controlled native GTK4/Qt6; geometry and compositor limits apply. |
| Headless XWayland | Supported with explicit limitations | XWayland 23.2.6 in the recorded Weston topology, controlled GTK4; no broad desktop or compositor claim. |
| GNU/Linux aarch64 | Qualified | Exact package and live evidence exist on the recorded native arm64 host/topologies. |
| GNU/Linux x86_64 | Package/ABI qualified with emulation limitation | Archive, ABI, install, Doctor, Managed and semantic smoke were run through amd64 emulation on an arm64 host. Native x86_64 hardware and a full native environment matrix are not qualified. |
| Linux native VT | Not qualified / not tested | PTY evidence does not substitute for a real virtual-console TTY. |
| Ordinary local X11 desktop | Optional, not qualified | Controlled Xvfb is not evidence for a normal desktop session. |
| Ordinary Wayland desktop | Optional, not qualified | Headless Weston evidence does not claim a full desktop session. |
| Same-host SSH -> existing desktop | Not qualified / not tested | The supported SSH topology is SSH into the managed headless session. |
| Wayland over SSH | Not qualified / not tested | No claim is made for remote Wayland transport. |
| Wayland static capture | Unsupported / deferred | Static capture is not implemented. |
| Cross-host Remote Companion | Post-1.0 | No cross-host semantic transport or companion protocol exists. |

## Product and terminal boundary

The product contract is Linux, Headless-first, and Accessibility-driven. A
user needs a real interactive UTF-8 terminal/PTY with cursor and alternate
screen support. Managed mode may still require a background Xvfb, session
D-Bus and AT-SPI service for the GUI application; the user does not need a
visible graphical desktop.

Only public AT-SPI semantics and explicitly advertised capabilities establish
operation authority. Partial realization, missing selected/current truth,
secret text, pointer-only behavior and application-specific Accessibility
omissions degrade to refusal or read-only presentation. The GUI remains the
authority; successful method return, event timing, local state or filesystem
state does not establish task success.

The v1.0 task contract is the qualified common-task baseline: structured
forms, exact selection and table-row operations, PageTab and qualified
hierarchy, menus/dialogs/modals, Open File, Choose Folder, Reader, Value,
single-line text, qualified complete non-secret plain-text external editing,
dynamic manual continuation, stale-authority refusal and safe terminal exit.
It does not include Remote Companion, arbitrary drag/drop, rich-text fidelity,
exhaustive virtualization, pointer-only context menus, native VT or broad
desktop compatibility.

## Upgrade and uninstall contract

The user installer is deliberately non-overwriting. A documented replacement
is:

1. Stop any owned Managed session.
2. Run the installed exact-file uninstaller.
3. Confirm that configuration, runtime/recovery data and unrelated prefix
   files remain; do not recursively delete the prefix.
4. Install the new archive into the same user prefix.
5. Run Doctor and the representative semantic smoke before normal use.

If the old manifest, installed files or Managed descriptor is unsafe or has
been modified, the uninstaller refuses and removes nothing. The user must
resolve that state explicitly; the package does not silently overwrite it.
