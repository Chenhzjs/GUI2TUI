#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-v05b-target}
display_number=${1:-127}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI="$target_dir/debug/gui2tui"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v05b.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    xvfb_pid=
    gtk_demo_pid=
    gtk_pages_pid=
    qt_pages_pid=
    cleanup() {
        if [[ -f "$runtime_dir/gui2tui/product.log" ]]; then
            cp "$runtime_dir/gui2tui/product.log" /tmp/gui2tui-v05b-product.log
        fi
        for pid in "$qt_pages_pid" "$gtk_pages_pid" "$gtk_demo_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x1000x24 >/tmp/gui2tui-v05b-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    gtk4-demo >/tmp/gui2tui-v05b-gtk-demo.log 2>&1 &
    gtk_demo_pid=$!
    python3 "$PROJECT_ROOT/tests/fixtures/v05b_gtk_page_fixture.py" \
        >/tmp/gui2tui-v05b-gtk-pages.log 2>&1 &
    gtk_pages_pid=$!
    python3 "$PROJECT_ROOT/tests/fixtures/v05b_qt_page_fixture.py" \
        >/tmp/gui2tui-v05b-qt-pages.log 2>&1 &
    qt_pages_pid=$!

    deadline=$((SECONDS + 15))
    until "$INSPECT" --app gtk4-demo --bootstrap walk --verbose 2>/dev/null \
        | grep -q "Button \\\"Constraints\\\""; do
        if (( SECONDS >= deadline )); then
            echo "GTK Demo Constraints did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done
    for app in gui2tui-v05b-gtk-pages gui2tui-v05b-qt-pages; do
        deadline=$((SECONDS + 15))
        until "$INSPECT" --app "$app" --bootstrap walk --verbose >/dev/null 2>&1; do
            if (( SECONDS >= deadline )); then
                echo "$app did not become accessible" >&2
                exit 1
            fi
            sleep 0.1
        done
    done

    python3 "$PROJECT_ROOT/tests/live/v05b_hierarchy_page_continuation.py"
'
