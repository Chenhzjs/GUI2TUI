# Phase 3H — static acquisition and deployment topology

**PHASE 3H STATIC MODALITY ACQUISITION VALIDATED** within the explicitly constrained
provider support below; this is not a claim of universal GUI object capture.

Validated on 2026-08-31: macOS arm64 build/tests and Ubuntu 24.04 arm64 under OrbStack,
AT-SPI 2.52, GTK 4.14.5, Xvfb (one 1280×800, 24-bit screen, explicitly 96 DPI),
scrot 1.10, EOG 45.3. Wayland acquisition: **NOT IMPLEMENTED**.

## Boundaries

```text
Fidelity-required Image
  → reference-first ModalityResolver
  → ModalityResource (= ModalityResolution)
      ReferencedResource / OriginalArtifact / RenderedSnapshot / LiveVisualState / Unavailable
  → user chooses disposition, independently of resource resolution
      InspectReference / MaterializeOnHost / OpenSameHost / SendToEndpoint / Unavailable

Explicit unresolved-Image request only:
  AT-SPI role + visibility + semantic bounds + native process identity
  → StaticVisualAcquisitionProvider
  → AcquiredVisual (one cropped PNG; no Debug/Serialize payload)
  → ArtifactDescriptor(origin=RenderedSnapshot, hash, size, TTL)
  → ArtifactMaterializer (host file, not transport)
  → optional, separately authorized same-host viewer
```

No bytes enter `SemanticCache`, `ContentArena`, or `TuiScene`. Ordinary semantic controls,
documents, menus and forms do not become capture candidates. `Image` is the initial supported
static role; live graphical state stays a fallback. Referenced resources are inspected/handed
off without downloading or capturing their bytes. Existing original-artifact producers can
use the independent materializer; automatic original-resource extraction remains **NOT IMPLEMENTED**.

`ArtifactOrigin::{OriginalResource, RenderedSnapshot}` is serialized in descriptors and manifests.
A rendered PNG is not called the original embedded resource (the tested GTK source was an SVG).
Legacy wire descriptors without `origin` retain their supplied-original-artifact meaning; new
descriptors explicitly carry origin. An old strict broker may reject the new field rather than
silently reinterpret it. Coordinate/quality metadata accompanies the host snapshot, not a full
desktop image or unrelated screen content.

## Capture contract and coordinate honesty

The generic provider boundary is in `src/modality/acquisition.rs`; protocol-specific fixed
commands are isolated in `src/backend/static_visual.rs`. There is no GTK/Qt/LibreOffice/browser
name dispatch. The implementation uses locally installed `/usr/bin/scrot -z -a x,y,w,h <private PNG>`.
No server descriptor can supply a program, shell, arguments, filename or executable code.
The [scrot 1.10 source](https://github.com/resurrecting-open-source-projects/scrot/blob/1.10/src/scrot.c)
shows that `scrotGrabAutoselect` reads only the rectangle through `imlib_create_image_from_drawable`.
No uncropped full-screen frame is saved or transported by this path.

Safety checks, before and after capture:

- Live AT-SPI `Image`, `Component`, Visible/Showing, non-defunct; no Text/Image byte extraction.
- Screen coordinates only; positive finite dimensions, within source, ≤8,388,608 pixels.
- Bounded parent/sibling checks reject overlapping semantic siblings, including the actual GTK
  mixed-layout defect observed below. Ancestor searches stop after 16 levels / 128 siblings.
- Exactly one native X11 screen and active monitor at origin (0,0); 96 DPI, identity RandR
  transform, no rotation, no explicit GDK/Qt scale or incompatible Xft DPI. Unsupported or
  ambiguous HiDPI/multi-monitor/XWayland configurations return unavailable, never rescale/guess.
- AT-SPI top-level bounds must match exactly one native client window from `xwininfo`; its
  `_NET_WM_PID` must match D-Bus `GetConnectionUnixProcessID`. The Image must fit inside it.
- PNG signature/IHDR dimensions must exactly match the requested region. No clipping accepted.
- Per-command deadline 5 s, overall acquisition deadline 10 s; cancellation kills/reaps the
  capture child and private temporary crop is removed. No capture loop or streaming exists.

The provider declares **CompositedScreenSnapshot**. It cannot rule out another window or popup
occluding the target between checks. UI/metadata explicitly say “may be occluded”; this is not
`CleanWindowCapture` or a guaranteed isolated-object snapshot. `UnknownCaptureQuality` and
`CleanWindowCapture` are contract values, not implemented providers. Geometry checks are
conservative detection, not proof against every broken/malicious accessibility implementation.

## Usage without a viewer

On the graphical-session host (SSH into that session is possible; SSH alone does not create AT-SPI):

```bash
gui2tui-inspect --app python3 --verbose
gui2tui-inspect --app python3 --dump-resource-reference NODE_ID
gui2tui-inspect --app python3 --materialize-modality NODE_ID --artifact-ttl-secs 300

gui2tui --app python3
# F4: inspect available modality objects
# Enter: inspect reference, or hand off only if a configured broker supports it
# m: explicit one-frame materialization for an unresolved Image
# o: separately request a same-host viewer for an already materialized snapshot
# Esc: return to semantic TUI; normal focus/Reader position remains unchanged
```

No `gui2tui-local` process or socket is needed for materialization. Reference path prints the
redacted reference and `payload_bytes=0`; even `--materialize-modality` does not capture a
resource already resolved by reference. Headless does not imply absence of the GUI's graphical
session: the viewer endpoint is absent, but capture still needs a graphical source.

Materializer limits: 32 MiB per artifact, locally generated private `gui2tui-artifact-<random>`
directory (0700), fixed MIME-derived basename, SHA-256/length verification, TTL 1–1800 seconds
(also bounded by producer lifetime). TUI retains at most 8 artifacts, expiring them during its
event loop and removing them on normal exit. CLI starts a restricted same-executable TTL reaper
before returning the path. The worker only deletes its validated manifest/generated file and
empty directory; it has no viewer, bus or network connection. No catch-all recursive deletion.
Materializations now carry a private ownership marker, session/operation identity,
creation/expiry timestamps and an exclusive lease. Normal shutdown and the restricted TTL
reaper remove only the fixed generated files. Broker receive directories use the same leased
namespace and are reclaimed on the next broker/runtime startup after a crash. Startup recovery
for an interrupted standalone materializer before its manifest is complete remains
**NOT IMPLEMENTED**; such an unidentifiable directory is deliberately not deleted. A viewer can
outlive the TTL; the file is not extended automatically.

## Same-host viewer (separate authorization)

```bash
client_dir=$(mktemp -d)
gui2tui-local serve --socket "$client_dir/broker.sock" \
  --mime 'image/*' --handler-program /usr/bin/eog

gui2tui-inspect --app python3 --materialize-modality NODE_ID \
  --open-materialized --modality-socket "$client_dir/broker.sock"
```

The broker receives a local path reference, not PNG payload bytes. Its local handler policy,
Once/Session/Deny authorization and executable-safety rules remain unchanged. Snapshot provenance
is in the materialization manifest and its display label (also shown in local approval).
An endpoint is never awaited when none is configured. `--open-materialized`/TUI `o` explicitly
mean same-host: a remotely forwarded broker without this local file will safely fail, not cause
automatic transport. Actual cross-host companion use is **NOT TESTED** this phase.

## Deployment matrix

Each cell states resolution / materialization / opening / transfer. “Available” means the
resource/provider exists, not a promise that every GUI exposes it or every endpoint has a handler.

| Resource | Headless | Same-host endpoint | Remote endpoint |
| --- | --- | --- | --- |
| Reference | resolve & inspect; no materialization; no Open; 0 payload | resolve; optional authorized Open; 0 payload | resolve; optional descriptor handoff; 0 artifact payload |
| OriginalArtifact | supplied artifact resolves; materialize; no Open; no transfer | materialize + authorized local path Open; no artifact transfer | existing approved artifact protocol; transfer required unless shared reference; cross-host NOT TESTED |
| RenderedSnapshot | explicit provider resolves if safe; materialize; no Open; no transfer | materialize + authorized local path Open; 0 network payload | descriptor includes RenderedSnapshot origin; existing artifact transport usable by producer API; automatic snapshot-send UI NOT IMPLEMENTED; cross-host NOT TESTED |
| LiveVisualState | classification/fallback only; no materialization/Open/transfer | fallback; live handoff NOT IMPLEMENTED | fallback; live streaming NOT IMPLEMENTED |

`ArtifactMaterializer` accepts supplied original artifacts (unit-tested). Headless and same-host
rendered-snapshot rows were live-tested, as was reference-first/no-endpoint inspection. Remote
transport primitives retain their independent-process Phase 3G tests; no remote-network claim.

## Real results

### GTK no-reference Image — PASS

The genuine GTK Picture had no source URI in AT-SPI. For the visual-only fixture variant:

```text
Image "Architecture diagram" [sensitive,showing,visible]
interfaces=[Accessible,Action,Component]
geometry=(0,0 480x180)
Reference=UNRESOLVED
origin=RenderedSnapshot
quality=CompositedScreenSnapshot
capture_source_bytes=345600
final_artifact_bytes=6419
headless_materialization=1
same_host_open=0
remote_transfer=0
network_payload_bytes=0
SHA256=2ec36a26275b36ae23020c7c08714c5a37c0d0cb88fc6289028f5adec92273ab
```

`capture_source_bytes` counts the requested 32-bit drawable pixel area, not total X protocol
overhead. PNG hash/dimensions were independently checked; EOG displayed the complete diagram at
480×180. The captured representation has no surrounding Label/desktop region. No source SVG was
read by the inspector, materializer, or capture provider. The fixture itself naturally loads
its image to render its GUI; that is not an acquisition side-channel.

strace `execve` proves initial TUI + F4/metadata inspection: **0 scrot calls**; explicit `m`: **1**.
Headless TUI materialize / Esc / Ctrl-C: PASS. Explicit 2-second CLI TTL removal: PASS.
The same-host broker reported `reference_only=1 artifact_bytes=0`; actual EOG pixels were viewed.

### GTK coordinate-negative case — refusal PASS

The original Picture-under-Label layout reports Image Screen extents `(0,0 480x180)` although
its actual pixels start below the Label. An initial experimental capture included that Label:
**not accepted as validation**. The resulting generic sibling-overlap guard rejects this layout
before provider invocation (`capture_source_bytes=0`). Both fixture variants are retained.
The visual-only variant is a separate real GTK window, not a synthetic artifact producer.

### LibreOffice embedded Image — UNAVAILABLE

The FODT fixture's frame was corrected to an inline paragraph anchor so that the embedded image
really appears in the document accessibility tree; a toolbar icon is not a substitute.

```text
Image "Embedded architecture image" [editable,enabled,focusable,resizable,selectable,showing,visible]
AT-SPI role=image
interfaces=[Accessible,Collection,Component,Hypertext,Image]
geometry=(282,319 75x76)
Reference=UNRESOLVED
AcquisitionUnavailable: AT-SPI bounds do not match a unique native client window
snapshot_attempt=1 snapshot_success=0 capture_source_bytes=0
```

No UNO, ODT parser/ZIP extraction or application-private API was used. This safe refusal is a
compatibility limitation, not a rendered-snapshot PASS.

### Regression and quality

- GTK/Qt inspector safe Button operations and independent read-back: PASS (42/31 nodes).
- Chrome/Firefox F4 reference handoff: PASS, 4 references and 0 artifact bytes each.
- Chrome headless reference materialization request: PASS, 0 snapshot attempts / 0 payload bytes.
- Chrome Phase 3F Reader/outline/search/table regression: PASS (399 nodes).
- Password/anonymous-action/event-cache/identity/scope/choice/command safety unit regressions: PASS.
- Independent wire processes, hash failure, truncated transfer, timeout, Once/Session/Deny: PASS.
- No new crates or dependency upgrades. New source acquisition adds no bootstrap work.

Both macOS and Linux quality suites passed: **191 tests** (189 library + 2 inspector CLI),
up from 178, with 13 additions. Commands actually executed on each platform:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
git diff --check
```

Linux uses `CARGO_TARGET_DIR=/tmp/gui2tui-live-target CARGO_INCREMENTAL=0` to keep host/guest
build products separate. The added tests cover explicit/Image-only gating, reference-first
refusal to capture, headless dispositions, provenance, rectangle/scale/overlap refusal,
private bounded hash-verified materialization, cancellation, expiry, and CLI target ambiguity.

Reproduce (optional; not ordinary CI):

```bash
# Ubuntu dependencies: existing GTK/AT-SPI/Xvfb setup plus
sudo apt-get install --no-install-recommends scrot x11-utils x11-xserver-utils strace eog
./scripts/phase3h-live-linux.sh gtk
VISUAL_ONLY=0 ./scripts/phase3h-live-linux.sh gtk
./scripts/phase3h-live-linux.sh libreoffice
```

For independent viewer evidence set `SCREENSHOT_HELPER` to the screenshot test helper. That
whole-screen viewer evidence is not the production artifact and is never transported by it.
Saved representative output: [cropped snapshot](assets/phase3h/rendered-snapshot.png),
[viewer evidence](assets/phase3h/viewer.png), [real TUI frame](assets/phase3h/tui-materialized.txt),
[headless metadata/metrics](assets/phase3h/headless.txt). Temporary paths in exports have expired.

## Remaining limits

Native X11, one monitor, unscaled 96 DPI only. Occlusion cannot be excluded. Mixed or inconsistent
semantic geometry safely refuses, potentially rejecting legitimate overlapping layouts.
Explicit TUI acquisition is bounded but currently waits for its result (up to 10 seconds);
background acquisition progress UI is NOT IMPLEMENTED. Original extraction, Wayland provider,
live video/3D transport, OCR/vision, remote desktop, compositor, and round-trip visual editing
are NOT IMPLEMENTED. No toolkit-specific recovery is hidden behind these limitations.

**Architectural conclusion:** a real accessibility Image without a reference can yield a
generic, explicit, minimal rendered representation, honestly labelled as such. Resource
availability is independent of presentation: headless materialization works without a client,
same-host opening is separate authorization, and optional remote transfer is a different layer.
This does not claim that arbitrary objects or graphical sessions are capturable.
