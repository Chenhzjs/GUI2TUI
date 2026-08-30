# GUI2TUI architecture

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

No production transcompiler rule selects behavior by toolkit or application
name. The legacy flat widget projection remains available with
--presentation legacy; the default is transcompiled.

Document/rich-text subtrees are derived into `SemanticContentModel`. Their main-scene presentation
is one bounded summary rather than one row per paragraph; interactive descendants retain scene
bindings. Body text is loaded through AT-SPI Text only when a Reader viewport needs it and is kept
in a separately bounded cache. See [content-navigation.md](content-navigation.md).

## Hybrid semantic + opaque future

The current product renders semantic regions and represents fidelity-required
content as an explicit OpaqueContent placeholder. It performs no capture,
raster conversion, compositor work, or GUI-pixel hit testing.

A future OpaqueSurfaceProvider may be attached behind a narrow boundary:

    OpaqueContent region
      → user explicitly requests visual handoff
      → provider obtains a surface
      → optional terminal image or external viewer

The provider must not become the tree source or the default representation. Its
surface identity, lifecycle, damage, popup relationship, and input mapping must
remain separate from semantic identities and semantic actions. Semantic
controls surrounding an opaque surface continue to use AT-SPI.

This makes an independently implemented Wayland provider possible without
copying compositor code into the MIT OR Apache-2.0 semantic core.
