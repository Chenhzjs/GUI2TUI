#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-126}

export PROJECT_ROOT="$project_root"
export GUI2TUI="$target_dir/debug/gui2tui"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export V06A_DISPLAY=":$display_number"

cargo build --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --bins
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib \
    retired_generation_cannot_confirm_even_if_late_state_matches
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib \
    explicitly_cancelled_ticket_cannot_publish_and_releases_capacity
cargo test --manifest-path "$project_root/Cargo.toml" \
    --target-dir "$target_dir" --lib \
    bounded_registry_shutdown_cancels_every_owner

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v06a.XXXXXX)
    export DISPLAY="$V06A_DISPLAY"
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export NO_AT_BRIDGE=0
    export V06A_DELAY_READY="$runtime_dir/delay-ready"
    export V06A_REPLACE_READY="$runtime_dir/replace-ready"
    export V06A_REPLACE_RESUME="$runtime_dir/replace-resume"
    export GUI2TUI_VALIDATION_HANDLER_MODE=conflict
    export GUI2TUI_VALIDATION_HANDLER_READY="$runtime_dir/handler-ready"
    export GUI2TUI_VALIDATION_HANDLER_RESUME="$runtime_dir/handler-resume"

    xvfb_pid=
    cleanup() {
        if [[ -n "$xvfb_pid" ]]; then
            kill "$xvfb_pid" 2>/dev/null || true
            wait "$xvfb_pid" 2>/dev/null || true
        fi
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x800x24 >/tmp/gui2tui-v06a-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment \
        DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/live/v06a_runtime_late_work.py"
'
