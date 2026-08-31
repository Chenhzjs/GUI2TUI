#!/usr/bin/env bash
# Explicit local-viewer test, never a server-side acquisition backend.
set -euo pipefail
export PROJECT_ROOT
PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}"
export RESULT_DIR
RESULT_DIR=$(mktemp -d /tmp/gui2tui-viewer-XXXXXX)
cd "$PROJECT_ROOT"
dbus-run-session -- bash -euo pipefail -c '
export DISPLAY=:90 XDG_SESSION_TYPE=x11
Xvfb "$DISPLAY" -screen 0 1000x650x24 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
broker_pid=
trap '\''[[ -z "$broker_pid" ]] || kill -INT "$broker_pid" 2>/dev/null || true; kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE
"$TARGET_DIR/debug/gui2tui-local" serve --socket "$RESULT_DIR/broker.sock" \
  --mime "image/*" --handler-program /usr/bin/eog --authorization once \
  >"$RESULT_DIR/broker.log" 2>&1 &
broker_pid=$!
sleep 1
if [[ "${MEMORY_ARTIFACT:-0}" == 1 ]]; then
  python3 tests/live/modality_memory_artifact.py "$RESULT_DIR/broker.sock" | tee "$RESULT_DIR/transfer.txt"
else
  "$TARGET_DIR/debug/gui2tui-local" send-artifact --socket "$RESULT_DIR/broker.sock" \
  --input "$PROJECT_ROOT/tests/fixtures/modality/architecture.svg" \
  --mime image/svg+xml --kind image | tee "$RESULT_DIR/transfer.txt"
fi
sleep 2
"$TARGET_DIR/debug/gui2tui-inspect" --list | tee "$RESULT_DIR/apps.txt"
"$TARGET_DIR/debug/gui2tui-inspect" --app eog >"$RESULT_DIR/viewer-tree.txt"
if [[ -n "${SCREENSHOT_HELPER:-}" ]]; then
  python3 "$SCREENSHOT_HELPER" --mode temp --path "$RESULT_DIR/viewer.png"
fi
kill -INT "$broker_pid"
wait "$broker_pid"
broker_pid=
cat "$RESULT_DIR/broker.log"
'
echo "RESULT_DIR=$RESULT_DIR"
