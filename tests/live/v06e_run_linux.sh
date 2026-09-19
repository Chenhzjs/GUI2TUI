#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-131}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export GUI2TUI="$target_dir/debug/gui2tui"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

cargo build --manifest-path "$project_root/Cargo.toml" --target-dir "$target_dir" --bins

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v06e.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    export V06A_DELAY_READY="$runtime_dir/delay-ready"
    export V06A_REPLACE_READY="$runtime_dir/replace-ready"
    export V06A_REPLACE_RESUME="$runtime_dir/replace-resume"
    export V06E_SUSPEND_MUTATE="$runtime_dir/suspend-mutate"
    xvfb_pid=
    cleanup() {
        if [[ -f "$runtime_dir/gui2tui/product.log" ]]; then
            cp "$runtime_dir/gui2tui/product.log" /tmp/gui2tui-v06e-product.log
        fi
        if [[ -n "$xvfb_pid" ]]; then
            kill "$xvfb_pid" 2>/dev/null || true
            wait "$xvfb_pid" 2>/dev/null || true
        fi
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x900x24 >/tmp/gui2tui-v06e-xvfb.log 2>&1 &
    xvfb_pid=$!
    for _ in $(seq 1 100); do
        if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
            break
        fi
        sleep 0.05
    done
    xdpyinfo -display "$DISPLAY" >/dev/null
    dbus-update-activation-environment \
        DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/live/v06e_integrated_runtime_continuity.py" \
        | tee /tmp/gui2tui-v06e-live.log
'
