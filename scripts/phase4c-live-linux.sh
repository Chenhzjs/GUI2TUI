#!/usr/bin/env bash
# One isolated, repeatable real-application workflow. Never attach to the user's desktop.
set -euo pipefail
export PROJECT_ROOT
PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export GUI2TUI_BIN=${GUI2TUI_BIN:-/var/tmp/gui2tui-p4c-target/release}
export TMPDIR=${TMPDIR:-/var/tmp}
export RESULT_DIR
RESULT_DIR=$(mktemp -d "$TMPDIR/gui2tui-p4c-XXXXXX")
export TEST_APP=${1:?usage: phase4c-live-linux.sh APP [probe|workflow|benchmark|fresh-benchmark]}
export TEST_MODE=${2:-probe}
printf 'RESULT_DIR=%s\n' "$RESULT_DIR"
cd "$PROJECT_ROOT"
dbus-run-session -- bash -euo pipefail -c '
export HOME="$RESULT_DIR/home" XDG_CONFIG_HOME="$RESULT_DIR/config" XDG_CACHE_HOME="$RESULT_DIR/cache" XDG_RUNTIME_DIR="$RESULT_DIR/runtime"
mkdir -m 700 "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
export XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
Xvfb -displayfd 3 -screen 0 1440x1000x24 -dpi 96 3>"$RESULT_DIR/display" >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
trap '\''kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
for attempt in $(seq 1 100); do
  [[ -s "$RESULT_DIR/display" ]] && break
  sleep .05
done
export DISPLAY=":"$(<"$RESULT_DIR/display")
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>" >/dev/null
done
if [[ "$TEST_MODE" == quarantine ]]; then
  python3 tests/live/phase4a_qt_quarantine.py | tee "$RESULT_DIR/quarantine-result.txt"
else
  python3 tests/live/phase4c_workflows.py
fi
'
