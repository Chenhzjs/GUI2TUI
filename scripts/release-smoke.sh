#!/usr/bin/env bash
# Usage: bash /path/to/extracted/gui2tui-*/smoke/run.sh
# No Cargo, source repository, or target/debug lookup is permitted here.
set -euo pipefail
bundle=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
[[ -x "$bundle/bin/gui2tui" ]] || { echo 'Run the smoke script inside an extracted release bundle' >&2; exit 1; }
export GUI2TUI_BUNDLE="$bundle"
export RESULT_DIR
RESULT_DIR=$(mktemp -d "${TMPDIR:-/tmp}/gui2tui-release-smoke-XXXXXX")
echo "RESULT_DIR=$RESULT_DIR"
dbus-run-session -- bash -euo pipefail -c '
export HOME="$RESULT_DIR/home" XDG_CONFIG_HOME="$RESULT_DIR/config" XDG_CACHE_HOME="$RESULT_DIR/cache" XDG_RUNTIME_DIR="$RESULT_DIR/runtime"
mkdir -m 700 "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
export DISPLAY="${DISPLAY_NUMBER:-:145}" XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 TERM=xterm-256color
Xvfb "$DISPLAY" -screen 0 1280x800x24 -dpi 96 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
trap '\''kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
# Xvfb startup is observably slower on some native runners.  Activating the
# AT-SPI registry before the display accepts connections leaves that registry
# permanently detached from X11, so wait on capability rather than timing.
for _ in $(seq 1 100); do
    if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
        display_ready=true
        break
    fi
    sleep 0.05
done
test "${display_ready:-false}" = true || {
    echo "Xvfb did not become ready" >&2
    exit 1
}
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>" >/dev/null
done
cd "$RESULT_DIR"
python3 "$GUI2TUI_BUNDLE/smoke/release_smoke.py"
'
