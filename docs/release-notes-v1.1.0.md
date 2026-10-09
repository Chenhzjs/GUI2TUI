# GUI2TUI v1.1.0 — Release Candidate notes

Status: internal release candidate qualification. No public v1.1 tag or
release is created by this branch.

## What changed

The current candidate also adds parameterized public text/range/Value operations
and same-host/SSH single-file resource handoff. See the
[candidate scope](release-notes-v1.1.0-rc.1.md) and
[resource usage guide](local-resource-viewing.md). Remote resource mode prevents
accidental interpretation of a remote path as a receiver-local path.

- The default application view is a bounded Semantic Surface that preserves
  meaningful regions such as shell controls, forms, navigation and document
  structure. Reader remains an explicit continuous-reading projection.
- Text editing is verified by fresh authoritative Accessibility readback.
  An accepted write that does not read back as requested is not reported as a
  successful edit and cannot be followed by an automatic submit.
- Semantic Submit remains separate from low-level keyboard delivery. Public
  Accessibility semantics are required for Semantic Submit.
- `Send Enter to current control` is an explicit, narrowly bounded Raw Enter
  capability. It resolves a fresh target, owning window and exact focus under
  the normal runtime authority chain, then reports delivery separately from
  any observed consequence.
- Post-operation observation is bounded and reuses the normal Surface refresh
  pipeline. It distinguishes delivery, relevant state change, stale targets,
  refresh failure and no detectable consequence.

## Support boundaries

Raw Enter is qualified only through the current X11 NativeInputBackend and
XTEST path. Semantic AT-SPI operations remain the supported route on the
qualified headless Wayland topology even when Raw Enter is unavailable.
Native text fallback is limited to printable ASCII in this candidate; direct
AT-SPI text mutation is not subject to that fallback limitation. Mouse input,
coordinate clicking, uinput, arbitrary key scripting, DOM/CDP and private
application/toolkit APIs remain out of scope.

See [limitations](limitations.md), [deployment](deployment.md) and the
[v1.1 support matrix](validation/v1.1/final-support-matrix.md) for the exact
qualification boundary.
