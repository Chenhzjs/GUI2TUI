# GUI2TUI v1.1 support matrix

This matrix describes the v1.1 release-candidate contract. A topology is not
qualified merely because another topology works; capability-specific limits
apply independently.

| Environment or capability | v1.1 status | Boundary |
| --- | --- | --- |
| Managed Xvfb | Supported | Recorded unprivileged Linux aarch64 session with AT-SPI |
| Docker/OCI interactive PTY | Supported with limits | Recorded Ubuntu 24.04 arm64 image and interactive PTY |
| Same-host SSH to Managed | Supported with limits | Loopback OpenSSH PTY into the same host/session |
| Headless Native Wayland | Supported with limits | Recorded Weston/Pixman topology; semantic operations only where public AT-SPI is exposed |
| Headless XWayland | Supported with limits | Recorded Weston/XWayland client path; does not broaden native Wayland Raw Enter |
| Linux aarch64 package/runtime | Qualified | Native package and live evidence in the recorded environment |
| Linux x86_64 package | Qualified with limits | Package/ABI and emulated runtime evidence; native x86_64 hardware not qualified |
| Raw Enter | Capability-qualified with limits | Explicit intent, X11 NativeInputBackend, fresh authority/window/focus verification only |
| Native text fallback | Capability-qualified with limits | Verified EditText only; printable ASCII boundary in this candidate |
| Semantic AT-SPI operations on Wayland | Supported with limits | Independent of Raw Enter availability |
| Linux native VT | Not qualified | No claim for a virtual-console-only environment |
| Ordinary desktop matrix | Not qualified | The recorded headless environments do not qualify every desktop/compositor |
| Wayland over SSH | Not qualified | No native-input or compositor claim |
| Mouse/coordinate input | Unsupported | No pointer injection or coordinate targeting |
| uinput/arbitrary key scripting | Unsupported | No low-level automation framework |
| Remote Companion | Deferred | Not part of v1.1 |

## Capability contract

Semantic operations resolve from current public Accessibility capabilities and
advertised actions. `Send Enter to current control` is a distinct explicit
low-level intent; it is not Semantic Submit and is never an implicit fallback.
Every native event requires the current runtime session, application
generation, interaction scope, operation ticket, fresh target, owning-window
verification and exact Accessibility focus verification. Delivery success is
reported separately from bounded post-delivery observation.

## Upgrade and uninstall contract

The v1.0.0 replacement flow remains the supported upgrade shape: stop an
owned Managed session, run the exact-file uninstaller, preserve configuration
and unrelated prefix files, install the new archive, run Doctor and perform a
representative semantic smoke. The installer refuses unsafe or pre-existing
targets rather than overwriting them. No v1.0.0 tag or release asset is
modified by v1.1 qualification.
