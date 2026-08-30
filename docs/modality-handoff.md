# External modality handoff

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
SHA-256, a five-minute cooperative stream timeout, cancellation and partial-file cleanup. Remote display names are never
used as local filenames.

The current synchronous `Read` transport checks its deadline before and after
each read. It cannot interrupt a permanently blocked third-party `Read`
implementation; a future socket adapter must also apply an OS/I/O timeout.

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
gui2tui-local reference --uri https://example.test/a.png \
  --mime image/png --kind image --authorization once
gui2tui-local artifact --input image.svg --mime image/svg+xml \
  --kind image --authorization once
```

AT-SPI Image objects commonly expose description and geometry, not an original
URI. A surrounding Hyperlink can provide a trustworthy reference. Embedded
LibreOffice images may remain `UNRESOLVED`; GUI2TUI does not use UNO, parse ODT,
inspect browser DOM/CDP, or guess from labels. Canvas and 3D viewports are live
visual state rather than fake portable files.
