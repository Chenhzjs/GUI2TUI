# GUI2TUI v0.1.0 — release candidate (not published)

GUI2TUI re-renders Linux accessibility semantics into terminal-native controls,
commands, choices and Reader content. It is not framebuffer-to-ASCII remote
desktop software.

Representative GTK, Qt, Chromium, Firefox, LibreOffice and Electron accessibility
workflows have been validated. Coverage depends on the semantics exposed by each
application. The candidate supports safe advertised actions, terminal-native
choice selection, atomic plain single-line editing with authoritative read-back,
read-only document navigation/search, an event-driven semantic cache, reference-
first modality handling and explicit static-image acquisition.

This candidate fixes two generic correctness boundaries:

- multiline document buffers cannot enter atomic single-line editing;
- incomplete AT-SPI Cache inventories and unrealized Document skeletons cannot
  be accepted as complete scenes, while partial Reader/search wording remains
  explicit about source coverage.

Known P2 limitations include multi-second fallback startup for large fresh
browser trees while Cache is incomplete, Writer `PartialRealized` long documents,
partial Electron workflows, unsupported complex Designer controls and a blocked
PCManFM-Qt bridge workflow. Anonymous actions and unavailable semantics degrade
without guessed input. Password and multiline editing, DOM/CDP, UNO, remote
transport, new-TTY attach and Wayland static capture are not implemented.

`v0.1.0 PUBLIC RELEASE NOT PUBLISHED`. The final non-publishing dual-architecture
pipeline evidence and exact RC source digest are recorded in
[Phase 4C validation](phase4c-validation.md).
