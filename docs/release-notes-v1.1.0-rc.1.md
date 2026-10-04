# GUI2TUI v1.1.0-rc.1

**Internal Release Candidate — not a public release and not the final
v1.1.0 release.**

This candidate is for controlled qualification of the v1.1 behavior. It does
not create a public tag, GitHub Release or downloadable public artifact.

## Candidate scope

- A richer Semantic Surface preserves meaningful application-shell and
  document/content regions while implementation wrappers are normalized.
- Surface and Reader are separate projections: Surface keeps bounded context
  and controls; Reader provides continuous document reading, outline, search
  and paging where public Accessibility text supports them.
- Text editing requires strict authoritative Accessibility readback. An
  accepted write that reads back differently is not a successful edit and
  cannot be followed by automatic Submit.
- Semantic Submit remains distinct from low-level keyboard delivery. Public
  Accessibility semantics are required for Semantic Submit.
- `Send Enter to current control` is an explicit Raw Enter capability. It
  verifies the current runtime authority, exact target, owning X11 window and
  Accessibility focus before delivery, then reports delivery separately from
  any observed consequence.
- Post-operation observation is bounded and reuses the normal Surface refresh
  pipeline. It reports relevant state/structure changes, stale targets,
  refresh failures, timeouts and no detectable change without inventing
  semantic success.

## Support boundaries

Raw Enter is qualified only through the current X11 NativeInputBackend and
XTEST path. Semantic AT-SPI operations remain the supported route on the
qualified headless Wayland topology when Raw Enter is unavailable. Native text
fallback is limited to printable ASCII in this candidate; direct AT-SPI text
mutation is not subject to that fallback limitation.

Mouse input, coordinate clicking, uinput, arbitrary key scripting, DOM/CDP
and private application/toolkit APIs remain out of scope. Accessibility
quality and the resulting read-only/unsupported degradation remain
application-defined.

See [limitations](limitations.md), [deployment](deployment.md) and the
[v1.1 support matrix](validation/v1.1/final-support-matrix.md) for the
qualification boundaries. The final v1.1.0 release notes draft is retained in
[release-notes-v1.1.0.md](release-notes-v1.1.0.md) and is not this RC's public
release announcement.
