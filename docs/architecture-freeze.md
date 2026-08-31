# v0.1 core architecture freeze policy

Status: **CORE ARCHITECTURE FROZEN FOR v0.1** following the Phase 4A evidence review.

The freeze boundary is the existing SemanticNode/Region/Graph, TuiScene, SemanticCommand,
SemanticChoice, SemanticContentModel/ContentArena, VirtualCollectionModel, SemanticTableModel
and ExternalModality resource contracts. Phase 4A completion does not structurally change them.

Freezing does not mean the implementation is bug-free. Tests, packaging, diagnostics, recovery
plumbing and bounded correctness fixes may continue. A new widget, another toolkit quirk or
another visual format alone is not a reason to redesign core IR. Reopening the boundary requires
a reproduced v0.1 blocker demonstrating structural inadequacy, with an explicit architecture
decision and regression evidence.

## Next phase: 4B only

1. Installation and distro dependency diagnostics, including AT-SPI session visibility.
2. Documented configuration for existing timeouts, resource bounds and local handlers.
3. First-run application selection and keyboard help.
4. Consistent degraded-state messages and explicit retry/reselection instructions.
5. Headless and same-host release smoke tests, packaging and documentation.

No new TTY/session daemon protocol, remote authentication/transport, compositor, graphics
streaming or semantic architecture research is included in Phase 4B.

Evidence and scope limitations: [Phase 4A completion](phase4a-completion.md).
