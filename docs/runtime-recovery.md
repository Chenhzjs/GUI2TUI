# Phase 4A runtime and recovery model

This document records implemented facts. It does not claim that all Phase 4A live gates have
passed yet. The relational Semantic IR, content IR, Scene, capability and modality resource
types are unchanged: lifecycle ownership wraps their bindings instead of entering those IRs.

```text
RuntimeSession (single logical owner)
 ├─ Terminal: Attached / Detached / Reattaching
 ├─ ApplicationGeneration N
 │    ├─ SemanticCache / ContentRuntime / Scene
 │    ├─ AT-SPI event subscription
 │    └─ operation registry
 └─ Optional EndpointProfile: Unavailable / Connecting / Available / Disconnected
```

## Identity and ownership

`RuntimeSessionId` is process-session local. Each explicit application open creates a monotonically
increasing `ApplicationGenerationId`. Application root locator identity, not its display/process
name, determines whether a refresh belongs to the current generation. Once that root vanishes,
the event receiver is closed, editing/search/Reader/Choice/command/modality state is discarded,
materialized artifacts are dropped and all registered operation cancellation tokens fire.

The runtime does not reconcile role/name fingerprints across generations. F5 explicitly opens a
fresh instance of the prior selector; `b` returns to the application selector. Either path builds
a new cache before incrementing the generation. Stale `RuntimeNodeId`, `ContentBlockId`,
`SceneElementId`, modality candidates and operation tickets from the old generation cannot be
executed. Within one generation, existing locator/structural reconciliation remains unchanged.

Long operations receive a `(RuntimeSessionId, ApplicationGenerationId, OperationId)` ticket.
Results are applied only if the ticket is still registered and its generation is current. Static
capture was moved out of the terminal input handler into a cancellable task; GUI exit cancels and
aborts it, and any late result is dropped. Reference/same-host handoffs use the same guard.
Progressive content search is currently stepped by the single runtime owner and is discarded on
generation loss rather than delegated to an unowned worker.

## Terminal lifecycle

Unix signal plumbing is isolated in `runtime::signals`: SIGINT/SIGTERM stop, SIGUSR1 detaches,
SIGUSR2 reattaches. Detached deployment keeps the AT-SPI cache/event runtime alive and suppresses
draw/input. Reattach enters raw/alternate-screen state again, invalidates Ratatui's fullscreen
buffer without issuing a blocking cursor-position query, drops buffered input, then derives a
fresh frame from current Scene state. It does not replay frames. Focus, Reader position, outline,
search/table positions and Choice state stay in semantic runtime while the renderer is absent.

The terminal guard restores echo/canonical mode, mouse reporting, cursor and alternate screen on
normal return, SIGINT, SIGTERM and unwind. SIGKILL cannot run process cleanup. A panic hook performs
best-effort terminal restoration before the normal panic report; corrupt runtime state is still a
fatal error, while application/endpoint/timeouts are recoverable.

## Endpoint and artifact recovery

No configured endpoint is a valid headless state and causes no retry loop. F4 negotiates current
capabilities; a completed/lost handoff invalidates that lease so a later Open requires fresh
negotiation. Endpoint disconnect cannot kill `RuntimeSession`. Cross-host production transport is
**NOT IMPLEMENTED**; `EndpointProfileId` is configuration identity only, not a remote-auth scheme.

Broker receive operations use a same-UID private root, random operation directory, ownership JSON,
exclusive lease and registered fixed files. Startup examines at most 4096 candidate directories;
it skips locked live sessions and refuses unregistered files, hard links, symlinks, wrong UID/mode
or malformed manifests. It never recursively deletes a `/tmp/gui2tui*` glob. Private socket stale
recovery similarly requires the owner lock plus a failed connectivity check. Session grants remain
in one broker process and disappear on restart, so a replacement endpoint cannot inherit them.

## Backpressure, limits and status

The AT-SPI queue remains bounded (default 2048). Counters now expose received/dequeued/dropped,
high-water and resync requests without event contents. Overflow has one pending flag; a resync
overlapped by another flood produces at most one additional correctness baseline. The 10,000-event
unit storm held four queued items and one pending resync. AT-SPI bootstrap/node/text/relation calls
retain the backend timeout; capture is 10 s overall / 5 s command, broker control 5 s, reference
60 s and artifact transfer 300 s. `RuntimeLimits` centralizes the runtime-facing defaults; a full
public configuration UX is reserved for Phase 4B.

F12 renders a contents-free status snapshot: session/state/generation, attachment, endpoint,
active operations, temporary artifact count, semantic node count, full snapshots and event queue
statistics. It excludes names, text, passwords, URIs and submitted values. Debug tracing prints
the same redacted structure.

## Reproducible validation

```bash
# Linux Xvfb + real AT-SPI. Default is 20 accelerated application generations.
./scripts/phase4a-live-linux.sh gtk
RESTART_ROUNDS=3 ./scripts/phase4a-live-linux.sh qt

# Independent real broker processes, partial transfer and SIGKILL.
python3 tests/live/phase4a_broker_recovery.py target/debug/gui2tui-local
```

The GTK run on 2026-08-31 completed 20 generations in 85.99 s. It verified event processing while
detached, exact RuntimeNode focus preservation, 80×24 / 160×50 / 60×20 resize, stale old locator
rejection, explicit generation reopening and SIGTERM terminal restoration. This is an accelerated
churn test, **not** a claimed 30–60 minute soak.

Remaining Phase 4A live gates are tracked honestly: real AT-SPI daemon loss/reconnect is
**NOT TESTED**; cross-host endpoint is **NOT IMPLEMENTED**; a 30–60 minute soak is **NOT TESTED**;
Electron remains **BLOCKED** by its current accessibility exposure. Until all phase criteria have
real evidence, the project status is `PHASE 4A NOT YET VALIDATED`.
