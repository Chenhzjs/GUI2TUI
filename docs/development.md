# Development

## Build and quality suite

Rust 1.88 or newer is supported.

```bash
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo test --all-targets --locked
cargo clippy --all-targets -- -D warnings
git diff --check
```

macOS is a supported build/test host. Live operation requires Linux with a
reachable session D-Bus and AT-SPI registry.

## Controlled fixtures

```bash
python3 tests/fixtures/gtk4_live_fixture.py
QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 python3 tests/fixtures/qt6_live_fixture.py
```

Run `gui2tui` in the same desktop session. The fixtures contain only synthetic
values and safe local state changes.

## Linux live harnesses

The repeatable harness starts private HOME/XDG directories, D-Bus, Xvfb, AT-SPI
and controlled applications:

```bash
./scripts/live-test-linux.sh
CACHE_BOOTSTRAP_TEST=1 ./scripts/live-test-linux.sh
```

Browser packages are deliberately optional. Detailed event, bootstrap,
compatibility, runtime recovery and real-application evidence lives in the
corresponding documents under `docs/` and `docs/validation/`.

## Record the public demo

The recorder needs recording-only Linux tools: `ffmpeg`, `xterm`, `openbox`,
`xdotool`, `wmctrl`, and `xvfb`.

```bash
cargo build --release --locked
OUTPUT_DIR=/tmp/gui2tui-demo scripts/demo/record.sh
```

See [the demo walkthrough](demo/README.md). The MP4 belongs in the GitHub
Release, not Git history.

## Release assembly

```bash
./scripts/package-linux.sh
./scripts/validate-release.sh ARCHIVE --smoke
python3 -m unittest discover -s tests -p test_release_assembly.py
```

The authoritative dual-native build and attestation flow is described in
[release-pipeline.md](release-pipeline.md).
