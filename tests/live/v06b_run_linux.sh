#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-128}
runtime_root=$(mktemp -d /tmp/gui2tui-v06b-root.XXXXXX)
service_dir="$runtime_root/data/dbus-1/services"
mkdir -p "$service_dir"

export PROJECT_ROOT="$project_root"
export GUI2TUI="$target_dir/debug/gui2tui"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export V06B_DISPLAY=":$display_number"
export V06B_TRANSPORT_DISABLED="$runtime_root/transport-disabled"
export V06A_DELAY_READY="$runtime_root/delay-ready"
export V06A_REPLACE_READY="$runtime_root/replace-ready"
export V06A_REPLACE_RESUME="$runtime_root/replace-resume"

python3 - "$service_dir/org.a11y.Bus.service" "$project_root" <<'PY'
import pathlib
import sys

service = pathlib.Path(sys.argv[1])
project = pathlib.Path(sys.argv[2])
service.write_text(
    "[D-BUS Service]\n"
    "Name=org.a11y.Bus\n"
    f"Exec={project}/tests/fixtures/v06b_atspi_launcher.sh\n",
    encoding="utf-8",
)
PY

cleanup_root() {
    rm -r -- "$runtime_root"
}
trap cleanup_root EXIT

cargo build --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --bins
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib rejects_duplicate_exact_application_names
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib duplicate_names_remain_distinct_exact_current_choices
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib retired_generation_cannot_confirm_even_if_late_state_matches

XDG_DATA_HOME="$runtime_root/data" dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v06b-app.XXXXXX)
    export DISPLAY="$V06B_DISPLAY"
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export NO_AT_BRIDGE=0

    xvfb_pid=
    cleanup() {
        if [[ -n "$xvfb_pid" ]]; then
            kill "$xvfb_pid" 2>/dev/null || true
            wait "$xvfb_pid" 2>/dev/null || true
        fi
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x800x24 >/tmp/gui2tui-v06b-app-xvfb.log 2>&1 &
    xvfb_pid=$!
    for _ in $(seq 1 100); do
        if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
            break
        fi
        sleep 0.05
    done
    xdpyinfo -display "$DISPLAY" >/dev/null
    dbus-update-activation-environment \
        DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE V06B_TRANSPORT_DISABLED
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/live/v06b_application_recovery.py"
'

rm -f -- "$V06A_DELAY_READY"

XDG_DATA_HOME="$runtime_root/data" dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v06b-transport.XXXXXX)
    export DISPLAY="$V06B_DISPLAY"
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export NO_AT_BRIDGE=0

    xvfb_pid=
    cleanup() {
        if [[ -n "$xvfb_pid" ]]; then
            kill "$xvfb_pid" 2>/dev/null || true
            wait "$xvfb_pid" 2>/dev/null || true
        fi
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x800x24 >/tmp/gui2tui-v06b-transport-xvfb.log 2>&1 &
    xvfb_pid=$!
    for _ in $(seq 1 100); do
        if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
            break
        fi
        sleep 0.05
    done
    xdpyinfo -display "$DISPLAY" >/dev/null
    dbus-update-activation-environment \
        DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE V06B_TRANSPORT_DISABLED
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/live/v06b_transport_recovery.py"
'
