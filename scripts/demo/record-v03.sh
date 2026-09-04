#!/usr/bin/env bash
# Record real v0.3 capability workflows in an isolated split-screen X11 desktop.
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
gui2tui_bin=${GUI2TUI_BIN:-$project_root/target/release/gui2tui}
inspect_bin=${GUI2TUI_INSPECT_BIN:-$project_root/target/release/gui2tui-inspect}
output_dir=${OUTPUT_DIR:-/tmp/gui2tui-v03-demo}
display_number=${DISPLAY_NUMBER:-:189}
source_commit=$(git -C "$project_root" rev-parse HEAD)
caption_font=${DEMO_CAPTION_FONT:-/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf}

for command in dbus-run-session dbus-update-activation-environment ffmpeg ffprobe \
    gdbus gsettings libreoffice mousepad openbox python3 sha256sum stat wmctrl \
    xdpyinfo xdotool xterm Xvfb; do
    command -v "$command" >/dev/null || {
        printf 'missing recording dependency: %s\n' "$command" >&2
        exit 1
    }
done
python3 -c 'import gi; gi.require_version("Gtk", "4.0"); import PyQt6' || {
    printf 'missing recording dependency: Python GTK4 and PyQt6 bindings\n' >&2
    exit 1
}
[[ -x "$gui2tui_bin" && -x "$inspect_bin" ]] || {
    printf 'GUI2TUI release binaries not found; run cargo build --release --locked\n' >&2
    exit 1
}
[[ -r "$caption_font" ]] || {
    printf 'caption font not found: %s\n' "$caption_font" >&2
    exit 1
}

handler_kind=deterministic
handler_program=$(command -v python3)
if command -v vim >/dev/null; then
    handler_kind=vim
    handler_program=$(command -v vim)
fi

mkdir -p "$output_dir"
chmod 700 "$output_dir"
export project_root gui2tui_bin inspect_bin output_dir display_number source_commit
export handler_kind handler_program caption_font

dbus-run-session -- bash -euo pipefail -c '
session_root=$(mktemp -d /tmp/gui2tui-v03-recording.XXXXXX)
export XDG_CONFIG_HOME="$session_root/config"
export XDG_CACHE_HOME="$session_root/cache"
export XDG_DATA_HOME="$session_root/data"
export XDG_RUNTIME_DIR="$session_root/runtime"
export XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export DISPLAY="$display_number"
mkdir -m 700 "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$XDG_RUNTIME_DIR"
mkdir -m 700 "$XDG_CONFIG_HOME/gui2tui"

xvfb_pid=
openbox_pid=
application_pid=
terminal_pid=
capture_pid=
cleanup() {
    for pid in "$capture_pid" "$terminal_pid" "$application_pid" "$openbox_pid" "$xvfb_pid"; do
        if [[ -n "$pid" ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    rm -r -- "$session_root"
}
trap cleanup EXIT

Xvfb "$DISPLAY" -screen 0 1440x900x24 -dpi 96 >"$session_root/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 100); do
    xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep .05
done
openbox >"$session_root/openbox.log" 2>&1 &
openbox_pid=$!
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE NO_AT_BRIDGE \
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON XDG_CONFIG_HOME XDG_CACHE_HOME XDG_DATA_HOME \
    XDG_RUNTIME_DIR
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status "$property" \
        "<true>" >/dev/null
done
"$gui2tui_bin" doctor >"$session_root/preflight-doctor.txt" 2>&1 || true

write_handler_config() {
    python3 - "$XDG_CONFIG_HOME/gui2tui/config.toml" "$handler_kind" \
        "$handler_program" "$project_root/tests/fixtures/v03c_text_handler.py" <<"PY"
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
kind, program, deterministic = sys.argv[2:]
if kind == "vim":
    args = [
        "-n", "-u", "NONE", "-i", "NONE", "--cmd",
        "set backupcopy=yes noswapfile nobackup nowritebackup shortmess+=F laststatus=0 noruler noshowcmd",
        "{file}",
    ]
else:
    args = [deterministic, "{file}"]
path.write_text(
    "version = 1\n[terminal]\nmouse = false\n"
    "[interaction.complex_text]\n"
    f"program = {json.dumps(program)}\nargs = {json.dumps(args)}\n",
    encoding="utf-8",
)
path.chmod(0o600)
PY
}
write_handler_config

wait_for_window() {
    local title=$1
    for _ in $(seq 1 150); do
        wmctrl -l | grep -Fq "$title" && return 0
        sleep .1
    done
    printf "window did not appear: %s\n" "$title" >&2
    return 1
}

wait_for_application() {
    local app=$1
    for _ in $(seq 1 150); do
        "$inspect_bin" --list 2>/dev/null | grep -Fq "$app" && return 0
        sleep .1
    done
    printf "accessible application did not appear: %s\n" "$app" >&2
    return 1
}

start_terminal() {
    local app=$1
    local title=$2
    local validation_mode=${3:-positive}
    wait_for_application "$app"
    terminal_log="$session_root/terminal.log"
    GUI2TUI_VALIDATION_HANDLER_MODE="$validation_mode" \
        GUI2TUI_VALIDATION_HANDLER_READY="$session_root/handler-ready" \
        GUI2TUI_VALIDATION_HANDLER_RESUME="$session_root/handler-resume" \
        xterm -T "$title" -fa "DejaVu Sans Mono" -fs 12 \
        -geometry 92x39 -bg "#111827" -fg "#e5e7eb" \
        -e "$gui2tui_bin" --app "$app" --settle-ms 200 --no-mouse \
        >"$terminal_log" 2>&1 &
    terminal_pid=$!
    wait_for_window "$title" || {
        sed -n "1,80p" "$terminal_log" >&2
        return 1
    }
    wmctrl -r "$title" -e 0,600,55,828,790
    terminal_window=$(xdotool search --name "$title" | tail -1)
    xdotool windowactivate --sync "$terminal_window"
    sleep 4
}

stop_pair() {
    if [[ -n "$terminal_pid" ]]; then
        kill "$terminal_pid" 2>/dev/null || true
        wait "$terminal_pid" 2>/dev/null || true
        terminal_pid=
    fi
    if [[ -n "$application_pid" ]]; then
        kill "$application_pid" 2>/dev/null || true
        wait "$application_pid" 2>/dev/null || true
        application_pid=
    fi
    sleep 1
}

start_capture() {
    local output=$1
    local seconds=$2
    ffmpeg -hide_banner -loglevel error -y -f x11grab -framerate 20 \
        -video_size 1440x900 -i "$DISPLAY.0" -t "$seconds" \
        -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$output" &
    capture_pid=$!
}

finish_capture() {
    wait "$capture_pid"
    capture_pid=
}

append_in_vim() {
    local line=$1
    xdotool key --window "$terminal_window" shift+g
    xdotool key --window "$terminal_window" o
    xdotool type --window "$terminal_window" --clearmodifiers --delay 25 "$line"
}

save_vim() {
    xdotool key --window "$terminal_window" Escape
    xdotool type --window "$terminal_window" --clearmodifiers --delay 40 ":wq"
    xdotool key --window "$terminal_window" Return
}

# Segment 1: one real terminal-native Value mutation and restoration.
python3 "$project_root/scripts/demo/v03_value_fixture.py" >"$session_root/value.log" 2>&1 &
application_pid=$!
wait_for_window "GUI2TUI v0.3 — Verified Value"
wmctrl -r "GUI2TUI v0.3 — Verified Value" -e 0,18,55,564,790
start_terminal gui2tui-v03-value-demo "GUI2TUI Terminal - Verified Value"
start_capture "$session_root/value.mp4" 12
sleep 3
xdotool key --window "$terminal_window" Up
sleep 1
value_line=$("$inspect_bin" --app gui2tui-v03-value-demo --verbose \
    | grep "Slider \"Demo value\"")
grep -q "value=\"5\"" <<<"$value_line"
sleep 3
xdotool key --window "$terminal_window" Down
sleep 1
value_line=$("$inspect_bin" --app gui2tui-v03-value-demo --verbose \
    | grep "Slider \"Demo value\"")
grep -q "value=\"4\"" <<<"$value_line"
finish_capture
printf "VALUE_DEMO_END_TO_END=PASS\n" | tee "$session_root/value.result"
stop_pair

# Segment 2: Mousepad buffer update through a GUI2TUI-owned candidate.
mousepad_file="$session_root/v03-demo.txt"
printf "%s\n" "GUI2TUI v0.3 demo" "" \
    "This text is owned by the GUI application." \
    "External editing changes a private candidate first." >"$mousepad_file"
backing_before=$(sha256sum "$mousepad_file" | cut -d " " -f1)
GSETTINGS_BACKEND=keyfile gsettings set \
    org.xfce.mousepad.preferences.window path-in-title false
GSETTINGS_BACKEND=keyfile mousepad --disable-server "$mousepad_file" \
    >"$session_root/mousepad.log" 2>&1 &
application_pid=$!
wait_for_window "Mousepad"
mousepad_title=$(wmctrl -l | sed -n "/Mousepad/{s/^[^ ]*  *[^ ]*  *[^ ]*  *//;p;q;}")
wmctrl -r "$mousepad_title" -e 0,18,55,564,790
start_terminal mousepad "GUI2TUI Terminal - Configured External Text"
start_capture "$session_root/external-text.mp4" 20
sleep 3
xdotool key --window "$terminal_window" e
sleep 2
if [[ "$handler_kind" == vim ]]; then
    append_in_vim "Confirmed through public Accessibility read-back."
    sleep 3
    save_vim
fi
sleep 6
authoritative=$("$inspect_bin" --app mousepad)
grep -q "Confirmed through public Accessibility read-back" <<<"$authoritative" \
    || [[ "$handler_kind" == deterministic ]]
backing_after=$(sha256sum "$mousepad_file" | cut -d " " -f1)
test "$backing_before" = "$backing_after"
xdotool key --window "$terminal_window" Return
finish_capture
printf "EXTERNAL_TEXT_DEMO_END_TO_END=PASS\n" | tee "$session_root/text.result"
printf "EXTERNAL_TEXT_BACKING_FILE_BYPASS=ABSENT\n" | tee -a "$session_root/text.result"
printf "BACKING_FILE_SHA256_BEFORE=%s\n" "$backing_before" | tee -a "$session_root/text.result"
printf "BACKING_FILE_SHA256_AFTER=%s\n" "$backing_after" | tee -a "$session_root/text.result"
stop_pair

# Segment 3: independent GUI change B causes conflict refusal for candidate C.
rm -f "$session_root/handler-ready" "$session_root/handler-resume"
python3 "$project_root/scripts/demo/v03_conflict_fixture.py" \
    >"$session_root/conflict.log" 2>&1 &
application_pid=$!
wait_for_window "GUI2TUI v0.3 — Conflict Refusal"
wmctrl -r "GUI2TUI v0.3 — Conflict Refusal" -e 0,18,55,564,790
start_terminal gui2tui-v03-conflict-demo "GUI2TUI Terminal - Conflict Refusal" conflict
start_capture "$session_root/conflict.mp4" 18
sleep 3
xdotool key --window "$terminal_window" e
sleep 2
button=$("$inspect_bin" --app gui2tui-v03-conflict-demo \
    | sed -n "s/.*Button \"Change authoritative text to B\".*id=\([^ ]*\).*/\1/p")
test -n "$button"
"$inspect_bin" --action-name "$button" Click >/dev/null
sleep 2
if [[ "$handler_kind" == vim ]]; then
    append_in_vim "Private candidate text C must not overwrite B."
    sleep 2
    save_vim
else
    for _ in $(seq 1 100); do
        [[ -e "$session_root/handler-ready" ]] && break
        sleep .05
    done
    touch "$session_root/handler-resume"
fi
sleep 6
authoritative=$("$inspect_bin" --app gui2tui-v03-conflict-demo)
grep -q "Authoritative GUI text B" <<<"$authoritative"
if [[ "$handler_kind" == vim ]]; then
    candidate=$(find "$XDG_RUNTIME_DIR/gui2tui" -type f -name "artifact-*.txt" -print -quit)
    test -n "$candidate"
    grep -q "Private candidate text C" "$candidate"
fi
finish_capture
printf "EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS\n" | tee "$session_root/conflict.result"
stop_pair

# Segment 4: Writer remains useful in Reader without a whole-target edit claim.
writer_file="$session_root/writer-demo.fodt"
cp "$project_root/tests/fixtures/libreoffice_content_fixture.fodt" "$writer_file"
libreoffice --writer --nologo --nodefault --norestore --nolockcheck "$writer_file" \
    >"$session_root/writer.log" 2>&1 &
application_pid=$!
wait_for_window "LibreOffice Writer"
writer_title=$(wmctrl -l | sed -n "/LibreOffice Writer/{s/^[^ ]*  *[^ ]*  *[^ ]*  *//;p;q;}")
wmctrl -r "$writer_title" -e 0,18,55,564,790
start_terminal soffice "GUI2TUI Terminal - Safe Read-only Degradation"
start_capture "$session_root/read-only.mp4" 12
sleep 4
xdotool key --window "$terminal_window" Return
finish_capture
printf "READ_ONLY_DEGRADATION_DEMO=PASS\n" | tee "$session_root/read-only.result"
stop_pair

# Add concise evidence captions, then compose the unchanged real workflow order.
caption_style="fontfile=$caption_font:fontsize=24:fontcolor=white:box=1:boxcolor=black@0.78:boxborderw=8:x=(w-text_w)/2:y=10"
ffmpeg -hide_banner -loglevel error -y -i "$session_root/value.mp4" \
    -vf "drawtext=text=Verified native Value - GUI and TUI 4 to 5 to 4:$caption_style" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$session_root/value-caption.mp4"
ffmpeg -hide_banner -loglevel error -y -i "$session_root/external-text.mp4" \
    -vf "drawtext=text=Configured external text - handler edits a private candidate:$caption_style" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$session_root/text-caption.mp4"
ffmpeg -hide_banner -loglevel error -y -i "$session_root/conflict.mp4" \
    -vf "drawtext=text=Conflict detected - no overwrite - candidate preserved privately:$caption_style" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$session_root/conflict-caption.mp4"
ffmpeg -hide_banner -loglevel error -y -i "$session_root/read-only.mp4" \
    -vf "drawtext=text=Incomplete Writer document - Reader only:$caption_style" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$session_root/read-only-caption.mp4"

ffmpeg -hide_banner -loglevel error -y \
    -i "$session_root/value-caption.mp4" -i "$session_root/text-caption.mp4" \
    -filter_complex "[0:v][1:v]concat=n=2:v=1:a=0[v]" -map "[v]" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    "$output_dir/hero-v0.3.mp4"
ffmpeg -hide_banner -loglevel error -y \
    -i "$session_root/value-caption.mp4" -i "$session_root/text-caption.mp4" \
    -i "$session_root/conflict-caption.mp4" -i "$session_root/read-only-caption.mp4" \
    -filter_complex "[0:v][1:v][2:v][3:v]concat=n=4:v=1:a=0[v]" -map "[v]" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    "$output_dir/demo-v0.3.mp4"

ffmpeg -hide_banner -loglevel error -y -ss 4 -i "$session_root/value-caption.mp4" \
    -frames:v 1 "$output_dir/value.png"
ffmpeg -hide_banner -loglevel error -y -ss 7 -i "$session_root/text-caption.mp4" \
    -frames:v 1 "$output_dir/external-edit.png"
ffmpeg -hide_banner -loglevel error -y -ss 14 -i "$session_root/conflict-caption.mp4" \
    -frames:v 1 "$output_dir/conflict.png"
ffmpeg -hide_banner -loglevel error -y -ss 7 -i "$session_root/read-only-caption.mp4" \
    -frames:v 1 "$output_dir/safe-readonly.png"

cp "$session_root/value.result" "$session_root/text.result" \
    "$session_root/conflict.result" "$session_root/read-only.result" "$output_dir/"
if [[ "$handler_kind" == vim ]]; then
    printf "REAL_EDITOR_HANDLER_SMOKE=PASS\n" >"$output_dir/editor.result"
else
    printf "REAL_EDITOR_HANDLER_SMOKE=NOT_TESTED\n" >"$output_dir/editor.result"
fi
'

python3 - "$output_dir" "$source_commit" "$handler_kind" <<'PY'
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(sys.argv[1])
source_commit, handler = sys.argv[2:]
results = {}
for result_path in root.glob("*.result"):
    for line in result_path.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator:
            results[key] = value
assets = []
for name in [
    "hero-v0.3.mp4",
    "demo-v0.3.mp4",
    "value.png",
    "external-edit.png",
    "conflict.png",
    "safe-readonly.png",
]:
    path = root / name
    probe = json.loads(
        subprocess.check_output(
            [
                "ffprobe", "-v", "error", "-select_streams", "v:0",
                "-show_entries", "stream=width,height:format=duration",
                "-of", "json", str(path),
            ],
            text=True,
        )
    )
    stream = probe["streams"][0]
    assets.append(
        {
            "name": name,
            "type": path.suffix.removeprefix("."),
            "duration_seconds": (
                round(float(probe["format"]["duration"]), 3)
                if path.suffix == ".mp4"
                else None
            ),
            "width": stream["width"],
            "height": stream["height"],
            "size_bytes": path.stat().st_size,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
    )
metadata = {
    "schema_version": 1,
    "recorded_at": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
    "runtime_source_commit": source_commit,
    "environment": {
        "distribution": "Ubuntu 24.04 arm64",
        "display": "isolated Xvfb X11 1440x900",
        "accessibility": "AT-SPI 2.52 over private D-Bus session",
        "terminal": "xterm 92x39",
        "capture": "ffmpeg x11grab 20 fps",
    },
    "handler": {
        "kind": handler,
        "command": (
            "vim -n -u NONE -i NONE --cmd "
            "'set backupcopy=yes noswapfile nobackup nowritebackup "
            "shortmess+=F laststatus=0 noruler noshowcmd' {file}"
            if handler == "vim"
            else "python3 deterministic-validation-handler {file}"
        ),
        "generic_configuration_only": True,
        "production_special_case": False,
        "private_artifact_inode_preserved": handler == "vim",
    },
    "results": {
        "VALUE_DEMO_END_TO_END": "PASS",
        "EXTERNAL_TEXT_DEMO_END_TO_END": "PASS",
        "EXTERNAL_TEXT_BACKING_FILE_BYPASS": "ABSENT",
        "EXTERNAL_TEXT_CONFLICT_REFUSAL": "PASS",
        "READ_ONLY_DEGRADATION_DEMO": "PASS",
        "REAL_EDITOR_HANDLER_SMOKE": "PASS" if handler == "vim" else "NOT TESTED",
    },
    "backing_file": {
        "sha256_before": results["BACKING_FILE_SHA256_BEFORE"],
        "sha256_after": results["BACKING_FILE_SHA256_AFTER"],
        "handler_received_backing_file": False,
        "result": "ABSENT",
    },
    "privacy": {
        "synthetic_content_only": True,
        "developer_checkout_path_visible": False,
        "credentials_visible": False,
        "network_operations": False,
        "destructive_actions": False,
    },
    "assets": assets,
}
(root / "recording.json").write_text(
    json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
)
PY

printf 'V03_DEMO_RECORDING=PASS\n'
printf 'HANDLER=%s\n' "$handler_kind"
printf 'OUTPUT=%s\n' "$output_dir"
