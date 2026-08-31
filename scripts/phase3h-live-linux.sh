#!/usr/bin/env bash
# Real semantic bounds -> generic cropped pixels. No source-file extraction.
set -euo pipefail
export PROJECT_ROOT
PROJECT_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}"
export RESULT_DIR
RESULT_DIR=$(mktemp -d /tmp/gui2tui-p3h-XXXXXX)
export TEST_APP="${1:-gtk}"
cd "$PROJECT_ROOT"
CARGO_TARGET_DIR="$TARGET_DIR" CARGO_INCREMENTAL=0 cargo build --bins
dbus-run-session -- bash -euo pipefail -c '
export DISPLAY="${DISPLAY_NUMBER:-:89}" XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0
Xvfb "$DISPLAY" -screen 0 1280x800x24 -dpi 96 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
app_pid=
trap '\''[[ -z "$app_pid" ]] || kill "$app_pid" 2>/dev/null || true; kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE
for property in IsEnabled ScreenReaderEnabled; do
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>"
done
if [[ "$TEST_APP" == libreoffice ]]; then
  cp tests/fixtures/libreoffice_modality_fixture.fodt "$RESULT_DIR/embedded.fodt"
  libreoffice --nologo --nodefault --norestore -env:UserInstallation="file://$RESULT_DIR/profile" "$RESULT_DIR/embedded.fodt" >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=soffice
else
  export VISUAL_ONLY="${VISUAL_ONLY:-1}"
  python3 tests/fixtures/gtk4_modality_fixture.py >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=python3
fi
sleep 5
python3 tests/live/phase3h_probe.py
'
echo "RESULT_DIR=$RESULT_DIR"
