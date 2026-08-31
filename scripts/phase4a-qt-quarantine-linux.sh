#!/usr/bin/env bash
set -euo pipefail
export PROJECT_ROOT
PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$PROJECT_ROOT"
export TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}"
export RESULT_DIR
RESULT_DIR=$(mktemp -d /tmp/gui2tui-p4a-qt-quarantine-XXXXXX)
CARGO_TARGET_DIR="$TARGET_DIR" CARGO_INCREMENTAL=0 cargo build --bins
dbus-run-session -- bash -euo pipefail -c '
export XDG_RUNTIME_DIR="$RESULT_DIR/runtime" DISPLAY="${DISPLAY_NUMBER:-:109}"
export XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
mkdir -m 700 "$XDG_RUNTIME_DIR"
Xvfb "$DISPLAY" -screen 0 1280x800x24 -dpi 96 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
trap '\''kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE \
  QT_LINUX_ACCESSIBILITY_ALWAYS_ON XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>"
done
python3 tests/live/phase4a_qt_quarantine.py
'
echo "RESULT_DIR=$RESULT_DIR"
