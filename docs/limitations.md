# v0.1 limitations

- Applications must expose useful Linux accessibility semantics. GTK/Qt/browser implementations
  differ. Read-only/unavailable is an honest result, not an invitation to inject guessed actions.
- Anonymous Chrome/Electron actions are never semantic index-0 fallbacks. Named safe actions or
  parent Selection are required. GTK ComboBox options may be accessibility-limited and remain read-only.
- Plain single-line editing is atomic replacement with read-back. Password writes, multiline/rich
  editing, remote caret/selection, IME and clipboard integration are NOT IMPLEMENTED.
- Some Qt Text interfaces can crash their own bridge; failed probes quarantine only that generation.
  Firefox read-back remains authoritative even when an application rejects/normalizes a write.
- Partial/virtualized collections and tables only expose available semantic data. No guarantee of
  complete logical contents from enumerated realized children.
- Reader/search depend on accessibility text availability. This is not a document-format parser.
- Static Image snapshots require reliable coordinates/provider and explicit action. They may be
  composited/occluded and are labelled accordingly. Live graphics/streaming/remote desktop out of scope.
- Wayland static acquisition, new-TTY attachment, remote production transport, persistent viewer
  trust UI, native deb/rpm/AppImage/Flatpak packages: NOT IMPLEMENTED.
- Electron remains best-effort/environment-dependent, not a v0.1 hard gate. No toolkit adapters,
  DOM/CDP/UNO, OCR, visual inference or application-specific extraction are used.
- A Linux aarch64 tarball is the release-test baseline. Other architectures must be built/tested
  independently; a matching filename is not proof of binary compatibility.

See [compatibility evidence](compatibility.md), [runtime recovery](phase4a-completion.md),
[real examples](gui-to-tui-examples.md) and [architecture freeze](architecture-freeze.md).

Phase 4C real-application evidence is [tracked separately](phase4c-validation.md).
Fresh Chrome profiles currently can force a multi-second walk when Cache.GetItems
is incomplete; the old fast-bootstrap measurement is not universally reproducible.
Multi-line editor buffers are Reader-only, never atomic single-line edit targets.
Writer long-document realized subsets do not imply complete-document search.
