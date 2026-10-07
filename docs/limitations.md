# GUI2TUI v1.1 limitations

GUI2TUI is an Accessibility-driven semantic runtime. Its capabilities depend
on what the target application exposes through public Linux AT-SPI interfaces.
Read-only or unavailable is an honest result, not a reason to invent
capability or use private application APIs.

## Semantic and interaction boundaries

- Not every control exposes a compatible public action, selection truth or
  authoritative readback. Missing or ambiguous semantics remain read-only or
  are refused.
- The v1.0 baseline covers structured forms, buttons/toggles, values, single
  selection, table rows, PageTabs, qualified hierarchies, menus/dialogs,
  Open File, Choose Folder, Reader, single-line text and qualified complete
  non-secret plain-text external editing.
- Broad or multi-selection recovery, arbitrary drag/drop, pointer-only context
  menus, exhaustive virtualization and generic rich-text fidelity are not
  promised.
- Partial or virtualized collections expose only the realized semantic data.
  `PartialRealized` content is never treated as a complete writable target.
- Reader and search depend on exposed accessibility text; GUI2TUI is not a
  document-format parser. Long documents may remain partial.
- Some Qt transient choice shapes do not expose reliable selected/current
  truth. GUI2TUI refuses false success for those shapes.
- Surface content is bounded: large document bodies belong to Reader, not a
  second unbounded copy in the ordinary Surface.

## Text, secrets and external handlers

- Qualified single-line editing uses bounded atomic replacement and
  authoritative readback.
- Complete, bounded, non-secret multiline text may use an optional local
  handler. Explicit whole-text replacement may replace formatting or embedded
  content according to the provider; children and formatting do not veto it.
  Partial, virtualized, IME, clipboard and remote-caret editing remain limited.
  The default 256 KiB whole-text budget is configurable with --max-edit-bytes;
  tree discovery budgets are --max-nodes and --max-depth.
- Native text fallback is a bounded delivery strategy for verified EditText
  only. The current qualified native fallback accepts printable ASCII; a
  non-ASCII request is refused rather than silently transliterated or damaged.
- Handlers receive only a private GUI2TUI-owned candidate, are invoked without
  a shell, and must preserve the owned artifact identity. Application backing
  files are never used as the semantic backend.
- `PasswordText` is never read, exported or passed to an external handler.
- ProgressBar/LevelBar and other informational Values remain read-only; Value
  mutation is limited to enabled controls with finite public bounds and an
  advertised increment.
- F3 exposes exact public named actions, selection membership add/remove, and
  adjustable Values in the current scope. See the
  [public operations validation](validation/public-operations/REPORT.md).

## Environment boundaries

The formal v1.1 support contract is in the
[v1.1 Final Support Matrix](validation/v1.1/final-support-matrix.md). The
[v1.0 matrix](validation/v1.0/final-support-matrix.md) remains the historical
public-release record.

- Managed Xvfb, the recorded unprivileged Docker/OCI interactive PTY, and
  same-host SSH into Managed are supported within their recorded topology
  boundaries.
- Headless Native Wayland and XWayland are supported only in the recorded
  Weston 13 headless/Pixman configuration, with geometry and compositor
  limitations.
- GNU/Linux aarch64 has native package and live evidence.
- GNU/Linux x86_64 packages are available, but runtime evidence used amd64
  emulation on an arm64 host. Native x86_64 hardware and a full native
  environment matrix are not qualified.
- Linux native VT, ordinary local X11/Wayland desktop sessions, same-host SSH
  into an existing desktop and Wayland over SSH are not qualified.
- Wayland static capture is unsupported/deferred. No compositor is bundled.
- Semantic AT-SPI operations remain available on the qualified headless
  Wayland topology, but Raw Enter is unsupported when no qualified X11 native
  backend is available.
- Cross-host Remote Companion and semantic transport are post-1.0 work.
- Native deb/rpm/AppImage/Flatpak packages are not provided by the v1.1
  release pipeline.

## Deliberate safety exclusions

Production behavior does not use DOM/CDP, UNO, private toolkit APIs, OCR,
screen understanding, coordinate clicking, mouse injection, uinput, arbitrary
key scripting, action index guessing, fuzzy target migration or
application-specific adapters. Explicit Raw Enter is a narrowly bounded X11
capability, never an implicit Semantic Submit fallback.

See [Troubleshooting](troubleshooting.md) for safe recovery guidance and
[Compatibility](compatibility.md) for historical application evidence.
