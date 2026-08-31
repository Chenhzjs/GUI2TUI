#!/usr/bin/env bash
# Optional real-session probes. Browser/toolkit names occur only in test setup.
set -euo pipefail
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
export PROJECT_ROOT="$project_root"
export TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}"
export TEST_APP="${1:-chrome}"
export TEST_DISPLAY="${DISPLAY_NUMBER:-:96}"
export RESULT_DIR
RESULT_DIR=$(mktemp -d /tmp/gui2tui-p3g-XXXXXX)
cd "$project_root"
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --bins
dbus-run-session -- bash -euo pipefail -c '
export DISPLAY="$TEST_DISPLAY" XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
Xvfb "$DISPLAY" -screen 0 1280x800x24 >"$RESULT_DIR/xvfb.log" 2>&1 &
xvfb_pid=$!
app_pid=
trap '\''[[ -z "$app_pid" ]] || kill "$app_pid" 2>/dev/null || true; kill "$xvfb_pid" 2>/dev/null || true'\'' EXIT
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
for property in IsEnabled ScreenReaderEnabled; do
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>"
done
case "$TEST_APP" in
chrome)
  google-chrome --no-sandbox --disable-gpu --disable-dev-shm-usage --no-first-run --no-default-browser-check --disable-background-networking --force-renderer-accessibility=complete --user-data-dir="$RESULT_DIR/profile" "file://$PROJECT_ROOT/tests/fixtures/browser_fixture.html" >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR="Google Chrome" ;;
firefox)
  mkdir "$RESULT_DIR/profile"
  cp tests/fixtures/firefox-user.js "$RESULT_DIR/profile/user.js"
  "${FIREFOX_BIN:-/opt/firefox-154.0.1/firefox}" --no-remote --profile "$RESULT_DIR/profile" "file://$PROJECT_ROOT/tests/fixtures/browser_fixture.html" >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=Firefox ;;
gtk)
  python3 tests/fixtures/gtk4_modality_fixture.py >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=python3 ;;
qt)
  python3 tests/fixtures/qt6_modality_fixture.py >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=qt6_modality_fixture.py ;;
libreoffice)
  cp tests/fixtures/libreoffice_modality_fixture.fodt "$RESULT_DIR/embedded.fodt"
  libreoffice --nologo --nodefault --norestore -env:UserInstallation="file://$RESULT_DIR/profile" "$RESULT_DIR/embedded.fodt" >"$RESULT_DIR/application.log" 2>&1 &
  app_pid=$!; export APP_SELECTOR=soffice ;;
*) exit 2 ;;
esac
sleep 8
python3 tests/live/phase3g_probe.py
if [[ "${OFFICE_RECOVERY:-0}" == 1 && "$TEST_APP" == libreoffice ]]; then
  APP_PID="$app_pid" python3 tests/live/phase4a_office.py
fi
'
echo "RESULT_DIR=$RESULT_DIR"
