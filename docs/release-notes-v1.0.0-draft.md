# GUI2TUI v1.0.0-rc.1 (unreleased Release Candidate)

> Internal Release Candidate preparation artifact. This is not a public tag,
> GitHub Release or public download.

GUI2TUI is a Headless-first Linux Accessibility runtime. It reconstructs
public GUI semantics, capabilities and useful spatial relationships as a
terminal-native interaction model. It does not copy GUI pixels and does not
use private toolkit APIs, OCR, coordinate input or application-specific
adapters.

## RC scope

The v1.0.0-rc.1 candidate integrates the verified semantic pipeline with a complete
first-use path: unprivileged installation, session diagnosis, Managed headless
setup, application selection, terminal interaction, authoritative result
readback and safe exit. The qualified task baseline covers structured forms,
selection, table rows, PageTab/hierarchy, menus/dialogs/modals, Open File,
Choose Folder, Reader, Value, single-line text, qualified non-secret complete
plain-text external editing and dynamic manual continuation.

The GUI remains authoritative. Unsupported or incomplete public semantics are
read-only or explicitly refused; a backend return or event does not become a
false success. Password content is never exported. External text editing uses
only a private GUI2TUI-owned candidate and public AT-SPI write/readback.

## Deployment evidence

The RC contract is Headless-first: Managed Xvfb, the recorded
unprivileged Docker interactive-PTY topology, same-host SSH into Managed and
the recorded Weston headless Native Wayland/XWayland topology. The detailed
architecture, distribution, compositor and terminal boundaries are in the
[RC support matrix](validation/v1.0/final-support-matrix.md).

The aarch64 package has native recorded live evidence. The x86_64 archive and
runtime smoke are qualified through amd64 emulation on an arm64 host; native
x86_64 hardware is not silently claimed. Linux native VT, ordinary desktop
compatibility, Wayland over SSH and cross-host Remote Companion remain outside
the RC contract.

## Safety and known limitations

Operation authority is exact and generation/scope-bound. Resize, events,
application replacement and recovery do not migrate old locators or tickets.
Partial realization is reported as partial. Rich-text fidelity, arbitrary
drag/drop, exhaustive virtualization, pointer-only context menus, Wayland
static capture and application-specific workarounds are not part of this
RC candidate.

The user installer intentionally refuses to overwrite existing targets. To
replace an installation, stop Managed, run the exact-file uninstaller, retain
configuration/runtime/recovery data, install the new archive, then run Doctor
and semantic smoke. Unsafe or modified manifests fail closed.
