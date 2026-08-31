# External modality handoff

This document records the Phase 3G broker/transport contract. Phase 3H adds a separate
[explicit static acquisition and headless materialization path](static-acquisition.md).
Below, statements that static acquisition is not implemented describe Phase 3G only.
The current resource variants are `OriginalArtifact` and `RenderedSnapshot` (formerly
`PortableArtifact` and the unimplemented `StaticVisualArtifact` variant); producer structs
retain their names for compatibility. Artifact descriptors now carry `origin`.

GUI2TUI remains semantic-first. Images, PDFs, audio/video files and portable
models are represented by `ExternalModality`; their bytes never enter
`SemanticCache`, `ContentArena`, or `TuiScene`.

```text
AT-SPI semantic node
        ↓
generic reference evidence
        ↓
ModalityResolver
   ├─ ReferencedResource ───────────────┐
   ├─ PortableArtifact (explicit input) ┤
   ├─ LiveVisualState                   │
   └─ Unavailable                       │
                                        ↓
                              LocalModalityBroker
                              (user policy authority)
```

## Resolution contract

The default `PreferReference` policy uses this order: an AT-SPI Hyperlink URI,
exact URI keys in Document attributes, then exact URI keys in Accessible
attributes. Names and descriptions are never parsed as paths or URIs. Debug
output removes URI fragments and redacts the full query.

`PortableArtifact` is a separate, bounded payload plane. Authorization happens
before the first payload read or partial file creation. Transfers enforce a
configurable maximum (512 MiB by default), fixed-size chunks, declared length,
SHA-256, a five-minute stream timeout, cancellation and partial-file cleanup. Remote display names are never
used as local filenames.

The private Unix-socket adapter applies 100 ms OS read polling plus an absolute
deadline, so even a peer that never sends EOF times out. The generic `Read`
adapter is cooperative; third-party adapters must provide equivalent bounded I/O.
Control JSON is length-prefixed and limited to 64 KiB; extra descriptor fields
are rejected. Artifact bytes follow only an `Approved` response. EOF, declared
length and SHA-256 must all agree before a handler is invoked.

Static visual acquisition, continuous capture, remote desktop, media streaming,
round-trip editing, directory synchronization, and executable/script handoff
are **NOT IMPLEMENTED**.

## Trust boundary

The GUI/session side is an untrusted descriptor producer. `gui2tui-local` owns
Once / Session / Deny authorization, allowed schemes and MIME classes, canonical
path mappings, local handler registration, and temporary cleanup. Handler
executable paths are local configuration and are never disclosed as capabilities
or accepted in a server descriptor.

## Debugging

```bash
gui2tui-inspect --app APP --dump-modalities
gui2tui-inspect --app APP --resolve-modality NODE_ID
gui2tui-inspect --app APP --dump-resource-reference NODE_ID
gui2tui-inspect --modality-capabilities

gui2tui-local capabilities
# These two commands use a recording handler unless --handler-program is supplied:
gui2tui-local reference --uri https://example.test/a.png \
  --mime image/png --kind image --authorization once
gui2tui-local artifact --input image.svg --mime image/svg+xml \
  --kind image --authorization once
```

## Connected TUI / local broker

The broker is a separate process with a private local socket; it needs no AT-SPI.
Create a private directory and configure a **trusted local viewer launcher**:

```bash
client_dir=$(mktemp -d)
gui2tui-local serve --socket "$client_dir/broker.sock" \
  --mime 'image/*' --handler-program /absolute/path/to/trusted-viewer-launcher

# In another terminal, in the selected graphical user's D-Bus session:
gui2tui --app APP --modality-socket "$client_dir/broker.sock"
gui2tui-inspect --app APP --handoff-modality NODE_ID \
  --modality-socket "$client_dir/broker.sock"
```

F4 opens the modality task; arrows select a resource, Enter requests handoff,
Esc returns without changing the GUI or replacing Reader/Choice state. A resolved
reference alone does not enable Open: a connected matching client capability is
required. Background content and modal scope restrictions remain enforced. Mouse
activation inside this overlay is **NOT IMPLEMENTED**.

The broker prompts on its own controlling terminal: `[o] Once / [s] Session /
[d] Deny (default)`. Session grants are local to this broker process and type/MIME;
an explicit Deny always wins. `--authorization once|session|deny` is an explicit
local automation policy. Without a controlling terminal or explicit policy it
denies. `--recording-handler` is test-only and **does not display resources**.

For explicit minimal-artifact producers (not automatic image extraction):

```bash
gui2tui-local send-artifact --socket "$client_dir/broker.sock" \
  --input image.svg --mime image/svg+xml --kind image
```

This command hashes the local input as a stream, sends only its descriptor until
approval, then transfers that single file. There is no directory operation.
`--cancel-before-transfer` exercises cancellation. The TUI currently exposes
resolved references; automatic portable-artifact acquisition from AT-SPI and
artifact submission inside the TUI are **NOT IMPLEMENTED**.

## Local safety and lifetime

* Socket directory must be 0700; socket is 0600; existing sockets are never replaced.
  Only local Unix sockets are implemented. SSH forwarding/authenticated remote
  transport and remote-profile authorization are **NOT IMPLEMENTED**.
* Network references permit HTTP(S), reject URL credentials, and are never downloaded
  by GUI2TUI. Query/fragment/credentials are redacted in diagnostics.
* `--map /server/prefix=/local/prefix` is local configuration only. Canonical targets
  must be regular files within that prefix; traversal and escaping symlinks fail.
* Viewable MIME classes and local file extensions must agree. MIME is still only a
  hint, **not file-content validation or a viewer sandbox**. Keep viewers updated.
* Payload files have random names under a private per-broker TempDir. Display names
  and remote artifact IDs cannot choose filenames. Completed artifacts are retained
  for a clamped 30–1800-second TTL (Session: up to 1800 seconds), or until broker exit.
  Failed transfers are deleted immediately. SIGINT cleans up; SIGKILL/crash cleanup
  of leftovers on the next run is **NOT IMPLEMENTED**.
  Each broker also caps retained artifacts at 64 files / 1 GiB, refusing more until
  cleanup rather than evicting a resource still in use. Byte-only progress is
  available through tracing debug instrumentation; no payload is logged.
* Launcher success means accepted, not visually confirmed. Long-running viewers are
  reaped in a background thread after a two-second launch check. Closing a viewer is
  not tracked, and a viewer can outlive the artifact TTL. No upload/round-trip edits.

Validation commands, evidence and remaining gates: [Phase 3G validation](phase3g-validation.md).

AT-SPI Image objects commonly expose description and geometry, not an original
URI. A surrounding Hyperlink can provide a trustworthy reference. Embedded
LibreOffice images may remain `UNRESOLVED`; GUI2TUI does not use UNO, parse ODT,
inspect browser DOM/CDP, or guess from labels. Canvas and 3D viewports are live
visual state rather than fake portable files.
