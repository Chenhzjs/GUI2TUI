# Phase 3G validation evidence — 2026-08-31

This document separates resource discovery, recording-handler dispatch, launcher
acceptance, and actual visual consumption. They are not interchangeable.

**PHASE 3G EXTERNAL MODALITY HANDOFF VALIDATED** for the explicit reference and
minimal-artifact contracts below. Automatic extraction of hidden/embedded original
resources remains unavailable; this label does not claim universal media support.

## Environment / commands

macOS 26.3.1 arm64, Rust 1.91.0. OrbStack `gui2tui-live`: Ubuntu 24.04 arm64,
Xvfb/X11, AT-SPI 2.52, Chrome 152.0.7977.64, Firefox 154.0.1,
LibreOffice 24.2.7.2. Linux build output is isolated in `/tmp/gui2tui-live-target`.

```bash
# Linux, optional live dependencies; no ordinary CI dependency:
CARGO_INCREMENTAL=0 scripts/phase3g-live-linux.sh chrome
CARGO_INCREMENTAL=0 scripts/phase3g-live-linux.sh firefox
CARGO_INCREMENTAL=0 scripts/phase3g-live-linux.sh gtk
CARGO_INCREMENTAL=0 scripts/phase3g-live-linux.sh qt
CARGO_INCREMENTAL=0 scripts/phase3g-live-linux.sh libreoffice

# On both macOS and Linux:
python3 tests/live/modality_wire.py /path/to/gui2tui-local
# Linux controlling-terminal authorization:
python3 tests/live/modality_authorization.py /tmp/gui2tui-live-target/debug/gui2tui-local

# Regressions actually executed:
CARGO_INCREMENTAL=0 scripts/live-test-linux.sh
CARGO_INCREMENTAL=0 DISPLAY_NUMBER=:91 scripts/phase3f-live-linux.sh chrome
```

The harness creates unique profiles/output directories, sets both
`org.a11y.Status.IsEnabled` and `ScreenReaderEnabled`, queries AT-SPI, then drives
a real Ratatui PTY. Browser `--no-sandbox` is used only in the isolated test VM.
No DOM/CDP or toolkit resource side channel is used by production discovery.

## Accessibility observations

| Application | AX nodes | Auto bootstrap | Original resource evidence |
| --- | ---: | ---: | --- |
| GTK image | 5 | Cache 0.754 ms | Image exists; no usable reference |
| Qt image | 4 | Walk 4.574 ms | QLabel/Label, not Image; no candidate |
| Chrome fixture | 399 | Cache 15.064 ms | Image GetURI + attributes; PDF/video/model Hyperlink |
| Firefox fixture | 277 | Cache 10.290 ms | enclosing image link and PDF/video/model GetURI; video poster `src` |
| LibreOffice embedded image | 1961 | Walk 2081.428 ms | Image exists, resource UNRESOLVED; incomplete bulk cache |

These are single diagnostic observations, not a new benchmark. Fresh Firefox
profiles initially produced only 36 objects in earlier probes; final event/realization
runs contained the actual page. Do not treat initial absence as permanent lack of
toolkit support. Chrome/Firefox `NAnchors` property calls failed, but a bounded
read-only `GetURI(0)` succeeded. No anonymous Action was invoked.

Representative actual resolutions (volatile runtime IDs omitted):

```text
Chrome Image: file:///.../tests/fixtures/modality/architecture.svg
  provenance=HyperlinkUri mime=image/svg+xml
Firefox Image: https://example.invalid/assets/architecture.svg?REDACTED
  provenance=HyperlinkUri mime=image/svg+xml
PDF link: https://example.invalid/manual.pdf?REDACTED application/pdf
Video link: https://example.invalid/demo.mp4?REDACTED video/mp4
Model link: https://example.invalid/model.gltf?REDACTED model/gltf+json
Chrome native <video>: UNRESOLVED
Firefox video poster: Image src=.../architecture.svg (NOT the video payload)
GTK Image / LibreOffice embedded Image: UNRESOLVED
```

The `.invalid` URLs are deliberate descriptor fixtures. Network downloading,
PDF rendering, video playback and 3D rendering are **NOT TESTED** by those links.
The existing tiny `sample.pdf`/`sample.gltf` are not used as viewer-validation evidence.
No embedded ODT/PDF extraction, static visual extraction, Canvas/game/3D streaming,
or Electron-specific probe was added (**NOT IMPLEMENTED / NOT TESTED**).

## Real terminal / broker results

Chrome and Firefox each: F4 → Image → Enter; then PDF/video/model links → Enter.
Each completed `reference_only=4 artifact_bytes=0 recorded_invocations=4`.
Esc returned with `semantic position preserved`; Ctrl-C exited normally.
GTK/Qt/LibreOffice used safe read-only degradation and zero handler calls.
Actual trimmed frame is in [GUI→TUI examples](gui-to-tui-examples.md).

Independent-process socket test passed on both hosts:

```text
WIRE_FAILURE_RECOVERY hash: PASS bytes=41
WIRE_FAILURE_RECOVERY cancel: PASS bytes=4
WIRE_FAILURE_RECOVERY timeout: PASS bytes=0
WIRE_POLICY once: reference_only=5 artifact_bytes=86 recorded_invocations=6
WIRE_POLICY session: reference_only=5 artifact_bytes=41 recorded_invocations=6
WIRE_POLICY deny: reference_only=0 artifact_bytes=0 denied=6 recorded_invocations=0
```

The 86-byte total includes one accepted 41-byte SVG, a 41-byte corrupt transfer,
and a 4-byte interrupted transfer: accounting includes failed payload, not just
successes. Early EOF is a failed transfer, not a guessed explicit Cancel event.
No reference was supplied for the minimal SVG artifact case. Same artifact ID
is safe to repeat: client generates distinct filenames, checks SHA-256 and size,
then removes partials or retains success until TTL/session cleanup.

Real local `/dev/tty` prompt test: `o → Opened`, `d → Failed`, `s → Opened`,
then session grant reused without another prompt; all four sent zero payload.
URI token was absent from prompt/log output. Local broker owns all decisions.

macOS independent image artifact test:

```text
gui2tui-local serve --socket <private-dir>/broker.sock --mime 'image/*' \
  --handler-program /usr/bin/open --authorization once
gui2tui-local send-artifact --socket <private-dir>/broker.sock \
  --input tests/fixtures/modality/architecture.svg --mime image/svg+xml --kind image
Opened { reference_only: false, artifact_bytes: 726 } payload_sent=726
```

Launcher accepted the artifact; SIGINT cleaned the broker session. macOS visual
inspection is **BLOCKED** by ungranted Screen Recording permission. The screenshot
skill preflight failed; no screenshot success is claimed.

Independent Linux image-viewer validation subsequently **PASSED**:

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends eog scrot
SCREENSHOT_HELPER=/path/to/screenshot/scripts/take_screenshot.py \
  scripts/phase3g-viewer-linux.sh
```

Installed 31 packages, 20.9 MB download / 72.5 MB disk; no desktop environment.
The independent broker received the reference-free descriptor, approved it, checked
the 726-byte SHA-256 payload, and launched EOG 45.3. AT-SPI discovered `eog` with
117 nodes and Window `artifact-QxaW2r.svg`. Screenshot inspection confirmed the
actual AT-SPI → Semantic IR → Local viewer drawing and the viewer property panel
`SVG image`, `480 × 180`, `726 bytes`. Broker exit removed its temporary artifact.
See [actual screenshot](assets/phase3g-eog.png). This proves explicit portable
artifact handoff, not automatic extraction from an unresolved GTK/LibreOffice image.

The full **reference-first GUI → TUI → real viewer** chain also passed:

```bash
VIEWER_PROGRAM=/usr/bin/eog \
SCREENSHOT_HELPER=/path/to/screenshot/scripts/take_screenshot.py \
  scripts/phase3g-live-linux.sh chrome
```

Chrome Image → F4 → Enter → client EOG → visible original SVG;
`reference_only=1 artifact_bytes=0`, `TUI_MODALITY_HANDOFF=True`, Esc and Quit PASS.
Screenshot inspection confirms the original browser fixture behind EOG displaying
`architecture.svg`, 480 × 180, 726 bytes. See [reference viewer](assets/phase3g-reference-eog.png).

To eliminate any ambiguity about a file-backed artifact having a usable local
reference, a separate generator test created SVG **only in process memory**, with
no source URI or source file:

```bash
MEMORY_ARTIFACT=1 SCREENSHOT_HELPER=/path/to/screenshot/scripts/take_screenshot.py \
  scripts/phase3g-viewer-linux.sh
NO_REFERENCE_AVAILABLE=True source=generated-memory bytes=221 result=Opened
reference_only=0 artifact_bytes=221 denied=0 cancelled=0 handler_unavailable=0
```

Actual EOG frame showed “Reference-free artifact”, SVG 480 × 180, 221 bytes.
[Screenshot](assets/phase3g-memory-artifact.png). The generator sends the descriptor,
waits for approval, then sends one minimal payload. This is a generic producer
test, not an extraction workaround or a claim about LibreOffice/GTK internals.

## Quality and scope of regression

178 Rust unit tests passed on macOS and Linux. On each host, actually ran:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
git diff --check
```

The inherited 166 tests remain, including content cache/search/table, graph/scope,
Choice, quarantine, password, anonymous-action safety, identity and overflow tests;
12 added tests cover socket framing/auth/integrity/timeout/cancel, reference policy,
capability rendering, symlink boundaries and disguised executable file extensions.
The additional storage-budget/expiry test bounds retained session artifacts.
GTK/Qt live Click/Press plus independent state reads passed. Chrome Phase 3F Reader,
Outline, progressive search, virtual collection/table harness exited zero. Not
every historical toolkit interaction was re-run in this continuation.

## Remaining boundaries

* No automatic portable extraction from accessibility; explicit minimal-artifact
  producer exists, while TUI currently submits resolved references only.
* Only local Unix sockets; SSH forwarding and remote profile grants NOT IMPLEMENTED.
* MIME/extension policy is not parser validation or a viewer sandbox.
* Artifact lifecycle is TTL/session, not exact third-party viewer-close tracking.
  Crash/SIGKILL cleanup sweep NOT IMPLEMENTED.
* No static capture backend or live visual transport in production.
