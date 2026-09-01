#!/usr/bin/env bash
# Record a real split-screen GTK + GUI2TUI session in an isolated X11 desktop.
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
gui2tui_bin=${GUI2TUI_BIN:-$project_root/target/release/gui2tui}
output_dir=${OUTPUT_DIR:-/tmp/gui2tui-demo}
display_number=${DISPLAY_NUMBER:-:188}
duration=${DEMO_DURATION_SECONDS:-60}

for command in dbus-run-session ffmpeg gdbus openbox python3 wmctrl xdotool xterm Xvfb; do
    command -v "$command" >/dev/null || {
        printf 'missing recording dependency: %s\n' "$command" >&2
        exit 1
    }
done
[[ -x "$gui2tui_bin" ]] || {
    printf 'GUI2TUI binary not found: %s\n' "$gui2tui_bin" >&2
    printf 'build it with: cargo build --release --locked\n' >&2
    exit 1
}

mkdir -p "$output_dir"
chmod 700 "$output_dir"
export project_root gui2tui_bin output_dir display_number duration

dbus-run-session -- bash -euo pipefail -c '
session_root=$(mktemp -d /tmp/gui2tui-recording.XXXXXX)
export HOME="$session_root/home"
export XDG_CONFIG_HOME="$session_root/config"
export XDG_CACHE_HOME="$session_root/cache"
export XDG_RUNTIME_DIR="$session_root/runtime"
export XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export DISPLAY="$display_number"
mkdir -m 700 "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"

cleanup() {
    jobs -pr | xargs -r kill 2>/dev/null || true
    rm -rf -- "$session_root"
}
trap cleanup EXIT

Xvfb "$DISPLAY" -screen 0 1440x900x24 -dpi 96 >"$output_dir/xvfb.log" 2>&1 &
for _ in $(seq 1 100); do
    xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep .05
done
openbox >"$output_dir/openbox.log" 2>&1 &
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" "<true>" >/dev/null
done

# Start the registry before the fixture so its first accessible objects are not
# lost to a test-environment activation race.
"$gui2tui_bin" doctor >"$output_dir/preflight-doctor.txt" 2>&1 || true

python3 "$project_root/scripts/demo/demo_fixture.py" >"$output_dir/fixture.log" 2>&1 &
for _ in $(seq 1 100); do
    wmctrl -l | grep -q "GUI2TUI Demo Fixture" && break
    sleep .1
done

xterm -title "GUI2TUI Terminal" -fa "DejaVu Sans Mono" -fs 13 \
    -geometry 104x43 -bg "#111827" -fg "#e5e7eb" \
    -e bash -lc "\"$gui2tui_bin\" doctor; sleep 6; clear; exec \"$gui2tui_bin\"" &
for _ in $(seq 1 100); do
    wmctrl -l | grep -q "GUI2TUI Terminal" && break
    sleep .1
done

wmctrl -r "GUI2TUI Demo Fixture" -e 0,12,55,500,790
wmctrl -r "GUI2TUI Terminal" -e 0,522,55,906,790

ffmpeg -hide_banner -loglevel error -y -f x11grab -framerate 20 \
    -video_size 1440x900 -i "$DISPLAY.0" -t "$duration" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    "$output_dir/gui2tui-v0.1-demo.mp4" &
capture_pid=$!

sleep 9
terminal=$(xdotool search --name "GUI2TUI Terminal" | tail -1)
xdotool windowactivate "$terminal" key slash
xdotool type --delay 55 "gui2tui-demo"
sleep 2
xdotool key Return
sleep 2
xdotool key Return
sleep 7

# The first focusable scene element is the semantic article Reader entry.
xdotool key Return
sleep 7
xdotool key slash
xdotool type --delay 90 "semantic"
sleep 7
xdotool key Escape
sleep 3
xdotool key Escape
sleep 4

# Use the semantic command palette to invoke the real GTK button.
xdotool key colon
xdotool type --delay 75 "Activate safely"
sleep 5
xdotool key Return
sleep 12
xdotool key q

wait "$capture_pid"

# Repository-sized preview assets are derived from the real recording.
ffmpeg -hide_banner -loglevel error -y -ss 52 -i "$output_dir/gui2tui-v0.1-demo.mp4" \
    -frames:v 1 "$output_dir/hero.png"
ffmpeg -hide_banner -loglevel error -y -ss 41 -t 18 \
    -i "$output_dir/gui2tui-v0.1-demo.mp4" \
    -vf "fps=8,scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=96[p];[s1][p]paletteuse=dither=bayer:bayer_scale=3" \
    "$output_dir/hero-demo.gif"

printf "DEMO_RECORDING=PASS\n"
printf "MP4=%s\n" "$output_dir/gui2tui-v0.1-demo.mp4"
printf "HERO_PNG=%s\n" "$output_dir/hero.png"
printf "HERO_GIF=%s\n" "$output_dir/hero-demo.gif"
'
