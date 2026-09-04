# v0.3 limitations

- Applications must expose useful Linux accessibility semantics. GTK/Qt/browser implementations
  differ. Read-only/unavailable is an honest result, not an invitation to inject guessed actions.
- Anonymous Chrome/Electron actions are never semantic index-0 fallbacks. Named safe actions or
  parent Selection are required. GTK ComboBox options may be accessibility-limited and remain read-only.
- Qualified plain single-line editing is atomic replacement with read-back. Complete, bounded,
  non-secret multiline plain text may use an optional configured handler, but rich-text fidelity,
  partial/virtualized whole-target writes, remote caret/selection, IME and clipboard integration
  are NOT IMPLEMENTED.
- Some Qt Text interfaces can crash their own bridge; failed probes quarantine only that generation.
  Firefox read-back remains authoritative even when an application rejects/normalizes a write.
- Partial/virtualized collections and tables only expose available semantic data. No guarantee of
  complete logical contents from enumerated realized children, and `PartialRealized` content is
  never treated as a writable whole target.
- Native Value mutation is limited to enabled, non-read-only Slider/SpinButton-style controls with
  finite current/min/max and a positive public increment. ProgressBar/LevelBar remain informational;
  ScrollBars do not become generic writable UI noise.
- External interaction handlers are optional, shell-free user configuration. They edit private
  GUI2TUI-owned candidates, never application backing files. Compatibility is not universal: a
  handler that replaces the owned inode is safely refused. PasswordText is never exported.
- Broad Selection recovery and generic Expand/Collapse remain unimplemented. Anonymous actions
  and action-index fallbacks are always refused.
- Reader/search depend on accessibility text availability. This is not a document-format parser.
- Static Image snapshots require reliable coordinates/provider and explicit action. They may be
  composited/occluded and are labelled accordingly. Live graphics/streaming/remote desktop out of scope.
- Wayland static acquisition, new-TTY attachment, remote production transport, persistent viewer
  trust UI, native deb/rpm/AppImage/Flatpak packages: NOT IMPLEMENTED.
- Electron remains best-effort/environment-dependent, not a v0.1 hard gate. No toolkit adapters,
  DOM/CDP/UNO, OCR, visual inference or application-specific extraction are used.
- Official Linux x86_64 and aarch64 archives are built and smoked natively; other architectures
  must be built/tested independently. A matching filename is not proof of binary compatibility.

See [compatibility evidence](compatibility.md), [runtime recovery](phase4a-completion.md),
[real examples](gui-to-tui-examples.md) and [architecture freeze](architecture-freeze.md).

Phase 4C real-application evidence is [tracked separately](phase4c-validation.md).
Fresh Chrome profiles can force a multi-second correctness walk when Cache.GetItems
is incomplete. Complete 5,158-item Cache startup remains about 0.2 seconds; five
fresh incomplete samples had a 4.07-second median. This is a documented P2 startup
limitation, not an invitation to accept a partial tree.
Unqualified multiline editor buffers remain Reader-only, never atomic single-line edit targets.
Writer long-document realized subsets do not imply complete-document search;
search completion explicitly refers only to exposed semantic content.
