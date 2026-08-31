#!/usr/bin/env bash
# Isolated lifecycle test; no dependency on a modality endpoint.
set -euo pipefail
export PROJECT_ROOT
PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$PROJECT_ROOT"
export TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}"
export RESULT_DIR
RESULT_DIR=$(mktemp -d /tmp/gui2tui-p4a-XXXXXX)
export TEST_TOOLKIT="${1:-gtk}"
export TEST_DISPLAY="${DISPLAY_NUMBER:-:88}"
export FIREFOX_BIN="${FIREFOX_BIN:-/opt/firefox-154.0.1/firefox}"
CARGO_TARGET_DIR="$TARGET_DIR" CARGO_INCREMENTAL=0 cargo build --bins
dbus-run-session -- bash -euo pipefail -c '
export DISPLAY="$TEST_DISPLAY" XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
Xvfb "$DISPLAY" -screen 0 1280x800x24 -dpi 96 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
trap '\''kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
for property in IsEnabled ScreenReaderEnabled; do
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>"
done
if [[ "${CONTROLS_ONLY:-0}" == 1 ]]; then
  python3 tests/live/phase4a_controls.py
else
  python3 tests/live/phase4a_lifecycle.py
fi
'
echo "RESULT_DIR=$RESULT_DIR"
