# GUI2TUI architecture

Before the pipeline is constructed, the product selects exactly one
same-host connection environment: the current process's desktop/session
environment or an existing private GUI2TUI-managed headless descriptor.
`--session desktop` never imports that descriptor; `--session managed` never
falls back or creates one. This transport choice occurs before Tokio/D-Bus
initialization and supplies the registry below. It does not create application
authority: only a fresh enumeration and exact current application selection
opens an `ApplicationGenerationId`.

    GUI toolkit / browser
            │
            ▼
    AT-SPI backend ─────────── BackendLocator
            │
            ▼
    SemanticCache arena ────── RuntimeNodeId
            │
            ├── task/control view
            │        ↓
            │   Region Analyzer → SemanticRegion → Presentation Planner
            │        ↓
            │   TuiScene ───────────────────────── SceneElementId
            │
            └── Content Analyzer
                     ↓
                SemanticContentModel ───────────── ContentBlockId
                     ├── bounded Reader viewport + ContentCache
                     ├── Outline
                     ├── indexed/loaded Search
                     └── VirtualCollectionModel

    TuiScene + content overlay
            ├── focus / viewport / terminal hit regions
            ├── command palette / choice overlay
            └── renderer

Input travels in the reverse direction through bindings:

    keyboard or terminal mouse
      → SceneElementId
      → SceneBinding(RuntimeNodeId, BackendLocator, safe intent)
      → SemanticOperation
      → BackendOperation
      → AT-SPI

Verified v0.3 mutations follow the same authority rule. Native single-line
text and bounded Value operations invoke explicit public interfaces and then
refresh from independent application read-back. Qualified complete multiline
plain text may use a configured local process only as the human interaction
modality: it edits a private bounded candidate, while GUI2TUI retains runtime
identity, conflict checks, public AT-SPI write-back, and complete verification.
`PartialRealized`, rich, password, stale, or unverifiable targets never enter
that whole-text path.

No production transcompiler rule selects behavior by toolkit or application
name. The compatibility linear layout remains available with `--layout flat`;
the v0.2 spatial/responsive composition is the default. `--presentation legacy`
continues to select the pre-v0.2 projection when explicitly requested.

Document/rich-text subtrees are derived into `SemanticContentModel`. Their main-scene presentation
is one bounded summary rather than one row per paragraph; interactive descendants retain scene
bindings. Body text is loaded through AT-SPI Text only when a Reader viewport needs it and is kept
in a separately bounded cache. See [content-navigation.md](content-navigation.md).

## Semantic runtime and explicit external modality

The product renders semantic regions and represents fidelity-required content
as an explicit OpaqueContent placeholder. It does not use pixels as the tree
source, perform continuous capture, or use GUI-pixel hit testing.

External modality resolution remains behind a narrow boundary:

    fidelity-required semantic region
      → user explicitly requests View or Materialize
      → reference / original artifact / one rendered snapshot / unavailable
      → headless materialization or optional same-host viewer

Reference paths transfer zero payload. A generic X11 provider can acquire one
minimal, explicitly requested static frame when coordinates are trustworthy;
the result is labelled `RenderedSnapshot`, never original content. Wayland
snapshot acquisition, continuous streaming, remote transport and compositor
work are not implemented. See [static acquisition](static-acquisition.md) and
[deployment](deployment.md).

The modality provider never becomes the semantic tree source. Controls around
an opaque region continue to use AT-SPI.
