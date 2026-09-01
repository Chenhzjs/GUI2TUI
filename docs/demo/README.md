# GUI2TUI v0.1 demo

The public demo is a real split-screen recording of the GTK demo fixture and
the GUI2TUI terminal runtime. It is not a mockup. The terminal discovers the
application through AT-SPI, opens semantic content in Reader, searches exposed
content, and invokes a named safe action. The original GTK checkbox and status
label provide the independent GUI-side confirmation.

## Reproduce

On an isolated Debian/Ubuntu graphical test host, install the recording-only
tools (`ffmpeg`, `xterm`, `openbox`, `xdotool`, `wmctrl`, and `xvfb`), then:

```bash
cargo build --release --locked
OUTPUT_DIR=/tmp/gui2tui-demo scripts/demo/record.sh
```

The script creates a private HOME, XDG runtime, D-Bus session, AT-SPI service,
and Xvfb display. It records one 1440x900 H.264 frame stream and derives a small
PNG/GIF preview. These tools are not GUI2TUI runtime dependencies.

## Walkthrough

1. `gui2tui doctor` verifies the isolated accessibility session.
2. The application selector filters to `GUI2TUI Demo Fixture`.
3. Reader reflows a real read-only GTK Text object.
4. Search finds `semantic` in exposed content.
5. The command palette resolves `Activate safely`.
6. The named AT-SPI action changes the original GTK checkbox and status label.

## Captured frames

Reader and semantic search:

![GUI2TUI Reader over a real GTK accessibility tree](../assets/readme/reader-search.png)

Authoritative action confirmation in both the original GUI and TUI:

![GTK checkbox and status changed after the GUI2TUI semantic action](../assets/readme/action-confirmed.png)

Only synthetic text is used. The script deletes its private session directory
on exit. The full MP4 is a GitHub Release asset rather than a Git-tracked file.
Exact capture dimensions, hashes and the production source boundary are recorded
in [`recording.json`](recording.json).
