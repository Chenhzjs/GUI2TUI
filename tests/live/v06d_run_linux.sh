#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-130}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export GUI2TUI="$target_dir/debug/gui2tui"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0

cargo build --manifest-path "$project_root/Cargo.toml" --target-dir "$target_dir" --bins
cargo test --manifest-path "$project_root/Cargo.toml" --target-dir "$target_dir" \
    --lib continuing_handler_artifact_is_bounded_and_deferred_until_expiry
cargo test --manifest-path "$project_root/Cargo.toml" --target-dir "$target_dir" \
    --lib explicitly_cancelled_ticket_cannot_publish_and_releases_capacity
cargo test --manifest-path "$project_root/Cargo.toml" --target-dir "$target_dir" \
    --lib retained_artifact_budget_and_expiration_are_enforced

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v06d.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    xvfb_pid=
    cleanup() {
        if [[ -n "$xvfb_pid" ]]; then
            kill "$xvfb_pid" 2>/dev/null || true
            wait "$xvfb_pid" 2>/dev/null || true
        fi
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x900x24 >/tmp/gui2tui-v06d-xvfb.log 2>&1 &
    xvfb_pid=$!
    for _ in $(seq 1 100); do
        if xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
            break
        fi
        sleep 0.05
    done
    xdpyinfo -display "$DISPLAY" >/dev/null
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/live/v06d_external_terminal_lifecycle.py"
'
