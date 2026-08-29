# term.everything architecture and license study

Studied repository: https://github.com/mmulet/term.everything

- commit: c8e04be0ae9a05b43f17065fe6b7d2687152f7c4 (2026-03-18)
- license: GNU Affero General Public License v3.0
- source copied into GUI2TUI: **none**

The study used a temporary shallow clone outside this repository. This document
contains architectural observations, not derived implementation.

## Architecture observed

term.everything is a display-server path, not an accessibility-semantic path.
Its Go wayland package implements protocol objects and socket/message dispatch.
wl_surface commits apply double-buffered state including buffers, damage,
scale, transform, input/opaque regions, and subsurface ordering. xdg_surface,
xdg_toplevel, and xdg_popup assign window roles and configuration lifecycle.
Shared-memory buffers are mapped and copied into surface textures.

wayland/Desktop.go composites drawable surfaces in z-order into one RGBA
desktop buffer, including recursively positioned subsurfaces and cursor
surfaces. termeverything/TerminalDrawLoop.go drives frame callbacks,
composition, status, and terminal frame pacing. framebuffertoansi passes the
RGBA desktop to Chafa, selecting symbols or terminal graphics based on terminal
capabilities. Terminal keyboard and mouse escape sequences are translated into
Wayland keyboard/pointer events. XWayland support is represented through
XWayland shell/keyboard-grab protocols and launch arguments.

Popup support exists through xdg_popup; grab behavior is explicitly marked TODO
in the inspected commit. Client and surface removal, roles, subsurface
ordering, frame callbacks, and cursor surfaces demonstrate why an opaque
provider needs an explicit lifecycle rather than a one-shot screenshot.

## Useful boundaries for GUI2TUI

Worth adopting as independently designed concepts:

- surface lifecycle and damage are owned by an opaque provider;
- popup/subsurface relationships must be modeled, not flattened into pixels;
- input coordinates belong to a surface provider, not semantic TUI hit regions;
- rendering/frame pacing is isolated from protocol ingestion;
- XWayland is a compatibility subsystem, not an application-specific adapter.

Not adopted:

- a compositor as the primary application execution path;
- framebuffer-to-ANSI/Chafa as default presentation;
- GUI-pixel layout or pointer coordinates for semantic controls;
- term.everything source, generated protocol code, or implementation details.

## License boundary

GUI2TUI is MIT OR Apache-2.0; term.everything is AGPL-3.0. No source was copied,
adapted, linked, vendored, or added as a dependency. A future implementation
must either be independently written against public Wayland protocol
specifications or be a clearly separated external process with a license
review. This study does not assert that process separation alone resolves every
AGPL obligation.

