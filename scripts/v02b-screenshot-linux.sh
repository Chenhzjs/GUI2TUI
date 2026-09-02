#!/usr/bin/env bash
# Capture a real xterm rendering in an isolated Linux AT-SPI/Xvfb session.
# Application identities are validation inputs only; production layout logic
# never observes them.
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
application=${1:?usage: v02b-screenshot-linux.sh APP COLUMNS ROWS OUTPUT}
columns=${2:?usage: v02b-screenshot-linux.sh APP COLUMNS ROWS OUTPUT}
rows=${3:?usage: v02b-screenshot-linux.sh APP COLUMNS ROWS OUTPUT}
output=${4:?usage: v02b-screenshot-linux.sh APP COLUMNS ROWS OUTPUT}

if [[ "${GUI2TUI_CAPTURE_SESSION:-0}" != 1 ]]; then
    export GUI2TUI_CAPTURE_SESSION=1
    exec dbus-run-session -- "$0" "$application" "$columns" "$rows" "$output"
fi

gui2tui_bin=${GUI2TUI_BIN:-/tmp/gui2tui-live-target/release}
screenshot_helper=${SCREENSHOT_HELPER:-/mnt/mac/Users/chenhz/.codex/skills/screenshot/scripts/take_screenshot.py}
capture_dir=$(mktemp -d /var/tmp/gui2tui-v02b-shot.XXXXXX)
output_path=$(realpath -m "$project_root/$output")
mkdir -p "$(dirname -- "$output_path")" "$capture_dir/config" "$capture_dir/cache" "$capture_dir/runtime"
chmod 700 "$capture_dir/runtime"

export XDG_CONFIG_HOME="$capture_dir/config"
export XDG_CACHE_HOME="$capture_dir/cache"
export XDG_RUNTIME_DIR="$capture_dir/runtime"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

xvfb_pid=
application_pid=
xterm_pid=
cleanup() {
    for pid in "$xterm_pid" "$application_pid" "$xvfb_pid"; do
        if [[ -n "$pid" ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup EXIT

Xvfb -displayfd 3 -screen 0 1600x1100x24 -dpi 96 \
    3>"$capture_dir/display" >"$capture_dir/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 100); do
    [[ -s "$capture_dir/display" ]] && break
    sleep .05
done
export DISPLAY=":$(<"$capture_dir/display")"
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE \
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status \
        "$property" "<true>" >/dev/null
done

case "$application" in
    mousepad)
        selector=Mousepad
        command=(mousepad --disable-server "$project_root/tests/fixtures/spatial_review.txt")
        ;;
    chromium)
        selector="Google Chrome"
        command=(google-chrome --disable-gpu --disable-dev-shm-usage --no-first-run
            --no-default-browser-check --disable-background-networking
            --force-renderer-accessibility=complete
            "--user-data-dir=$capture_dir/profile"
            "file://$project_root/tests/fixtures/browser_fixture.html")
        ;;
    eog)
        selector=eog
        command=(eog --new-instance /usr/share/pixmaps/debian-logo.png)
        ;;
    gtk-demo)
        selector=gtk4-demo
        command=(gtk4-demo)
        ;;
    designer)
        selector=designer
        command=(/usr/lib/qt6/bin/designer)
        ;;
    *)
        echo "unsupported validation application: $application" >&2
        exit 2
        ;;
esac

"${command[@]}" >"$capture_dir/application.log" 2>&1 &
application_pid=$!
for _ in $(seq 1 100); do
    if "$gui2tui_bin/gui2tui-inspect" --list 2>/dev/null | grep -qi -- "$selector"; then
        break
    fi
    sleep .2
done
if ! "$gui2tui_bin/gui2tui-inspect" --list | grep -qi -- "$selector"; then
    echo "application was not exposed through AT-SPI: $selector" >&2
    exit 1
fi
if [[ "$application" == chromium ]]; then
    sleep 8
else
    sleep 2
fi

xterm -geometry "${columns}x${rows}" -fa Monospace -fs 10 \
    -title "GUI2TUI validation" \
    -e "$gui2tui_bin/gui2tui" --app "$selector" --layout spatial --log-level off &
xterm_pid=$!
window_id=
for _ in $(seq 1 100); do
    window_id=$(xdotool search --name "GUI2TUI validation" 2>/dev/null | tail -1 || true)
    [[ -n "$window_id" ]] && break
    sleep .1
done
if [[ -z "$window_id" ]]; then
    echo "xterm window was not created" >&2
    exit 1
fi
sleep 3
diagnostic_base=${output_path%.png}
"$gui2tui_bin/gui2tui-inspect" --app "$selector" --dump-layout-plan \
    >"${diagnostic_base}.layout.txt" 2>&1
"$gui2tui_bin/gui2tui-inspect" --app "$selector" --verbose \
    >"${diagnostic_base}.tree.txt" 2>&1
python3 "$screenshot_helper" --window-id "$window_id" --path "$output_path"
