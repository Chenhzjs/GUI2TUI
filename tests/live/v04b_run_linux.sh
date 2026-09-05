#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-125}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI="$target_dir/debug/gui2tui"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v04b.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    xvfb_pid=
    qt_pid=
    gtk_pid=
    cleanup() {
        if [[ -f "$runtime_dir/gui2tui/product.log" ]]; then
            cp "$runtime_dir/gui2tui/product.log" /tmp/gui2tui-v04b-product.log
        fi
        for pid in "$gtk_pid" "$qt_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x900x24 >/tmp/gui2tui-v04b-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/fixtures/qt6_live_fixture.py" \
        >/tmp/gui2tui-v04b-qt.log 2>&1 &
    qt_pid=$!
    deadline=$((SECONDS + 12))
    until "$INSPECT" --app gui2tui-qt-fixture >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "Qt fixture did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    python3 "$PROJECT_ROOT/tests/fixtures/gtk4_live_fixture.py" \
        >/tmp/gui2tui-v04b-gtk.log 2>&1 &
    gtk_pid=$!
    deadline=$((SECONDS + 12))
    until "$INSPECT" --app gui2tui-live-fixture >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "GTK fixture did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    python3 "$PROJECT_ROOT/tests/live/v04b_surface_scope_continuation.py"

    cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --lib transcompile::scope::tests
    echo "AMBIGUOUS_SURFACE_AUTHORITY_REFUSAL=PASS"
    cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --lib transcompile::command::tests
    cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --lib tui::transition::tests
'
