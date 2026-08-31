# Phase 4A runtime and recovery model

Phase 4A runtime and recovery hardening is validated; see the linked completion evidence below.
The relational Semantic IR, content IR, Scene, capability and modality resource
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

Broker receive operations and standalone materialization share a same-UID private root,
random operation directory, ownership JSON, shared live lease and registered fixed files.
Filename ownership is fsynced and atomically renamed before payload creation; the directory is
synced too. Recovery requires an exclusive lease. A standalone TTL reaper acknowledges its
shared lease before the producer releases ownership (five-second startup bound). Startup examines
at most 4096 candidate directories;
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

The final GTK run on 2026-08-31 completed 20 generations in 96.22 s. It verified event processing while
detached, exact RuntimeNode focus preservation, 80×24 / 160×50 / 60×20 resize, stale old locator
rejection, explicit generation reopening and SIGTERM terminal restoration. After changing restart
to reuse the healthy AT-SPI connection, RSS stayed 17,900–18,240 KiB, file descriptors remained
exactly 17 and threads remained exactly 13 across all generations. A preliminary run exposed the
old +1 fd/thread per restart and is not counted as the final result. This is an accelerated churn
test, **not** a claimed 30–60 minute soak.

The real GTK event-storm fixture generated 12,004 AT-SPI events from 2,000 label and selection
mutations: 2,645 dequeued, 9,359 dropped after the bounded queue reached its 2,048 high-water mark,
one resync request, zero queued at completion and authoritative final `Storm complete: 2000` state.
No resync loop occurred. Qt completed three generations in 18.00 s; Chrome completed two in 15.43 s.
Chrome's complete Reader/search/table probe used 399 nodes and its separate reference-first broker
probe transferred zero artifact bytes. Firefox reference regression passed. LibreOffice exposed
1,963 nodes and safely left its embedded image unresolved; no private extraction or fake handoff.

An independent-process broker test sent 4,096 bytes of a declared 2 MiB artifact and SIGKILLed the
broker. The server producer stayed alive, a replacement broker recovered exactly the abandoned
operation namespace, a second live broker namespace remained intact, the stale socket was rebound,
and fresh capabilities changed from `image/*` to `application/pdf`. SIGTERM then removed sockets
and owned artifacts. This proves bounded local recovery, not cross-host transport.

Completion evidence, including two independent thirty-minute runs and final regression review,
is recorded in [phase4a-completion.md](phase4a-completion.md).
The [v0.1 core architecture freeze](architecture-freeze.md) now applies.
New-TTY attachment and production cross-host transport remain **NOT IMPLEMENTED**, deliberately
outside the v0.1 hard gates. Same-process, same-PTY detach/resume is the supported contract.
