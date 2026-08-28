#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}

for command in cargo dbus-run-session gdbus Xvfb python3; do
    command -v "$command" >/dev/null || {
        echo "missing required command: $command" >&2
        exit 1
    }
done

cd "$project_root"
CARGO_TARGET_DIR="$target_dir" cargo build --bins

PROJECT_ROOT="$project_root" TARGET_DIR="$target_dir" dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-live-runtime.XXXXXX)
    export DISPLAY=:99
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
    export NO_AT_BRIDGE=0

    xvfb_pid=
    gtk_pid=
    qt_pid=
    cleanup() {
        for pid in "$qt_pid" "$gtk_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rmdir "$runtime_dir" 2>/dev/null || true
    }
    trap cleanup EXIT

    Xvfb :99 -screen 0 1280x800x24 >/tmp/gui2tui-live-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR \
        QT_LINUX_ACCESSIBILITY_ALWAYS_ON NO_AT_BRIDGE

    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/fixtures/gtk4_live_fixture.py" \
        >/tmp/gui2tui-live-gtk.log 2>&1 &
    gtk_pid=$!
    python3 "$PROJECT_ROOT/tests/fixtures/qt6_live_fixture.py" \
        >/tmp/gui2tui-live-qt.log 2>&1 &
    qt_pid=$!
    sleep 2

    inspect="$TARGET_DIR/debug/gui2tui-inspect"
    "$inspect" --list
    "$inspect" --app gui2tui-live-fixture > /tmp/gui2tui-live-gtk-tree.txt
    "$inspect" --app gui2tui-qt-fixture > /tmp/gui2tui-live-qt-tree.txt

    if [[ "${CACHE_BOOTSTRAP_TEST:-0}" == 1 ]]; then
        normalize_tree() {
            sed -E "s/ actions=\[[^]]*\] id=[^ ]*//; s/ id=[^ ]*//" "$1"
        }

        "$inspect" --app gui2tui-live-fixture --bootstrap walk \
            >/tmp/gui2tui-live-gtk-walk.txt
        "$inspect" --app gui2tui-live-fixture --bootstrap cache \
            >/tmp/gui2tui-live-gtk-cache.txt
        normalize_tree /tmp/gui2tui-live-gtk-walk.txt \
            >/tmp/gui2tui-live-gtk-walk-core.txt
        normalize_tree /tmp/gui2tui-live-gtk-cache.txt \
            >/tmp/gui2tui-live-gtk-cache-core.txt
        diff -u /tmp/gui2tui-live-gtk-walk-core.txt \
            /tmp/gui2tui-live-gtk-cache-core.txt
        echo "GTK cache/walk core semantic equivalence passed"

        if "$inspect" --app gui2tui-qt-fixture --bootstrap cache \
            >/tmp/gui2tui-live-qt-cache.txt 2>/tmp/gui2tui-live-qt-cache.err; then
            "$inspect" --app gui2tui-qt-fixture --bootstrap walk \
                >/tmp/gui2tui-live-qt-walk.txt
            normalize_tree /tmp/gui2tui-live-qt-walk.txt \
                >/tmp/gui2tui-live-qt-walk-core.txt
            normalize_tree /tmp/gui2tui-live-qt-cache.txt \
                >/tmp/gui2tui-live-qt-cache-core.txt
            diff -u /tmp/gui2tui-live-qt-walk-core.txt \
                /tmp/gui2tui-live-qt-cache-core.txt
            echo "Qt cache/walk core semantic equivalence passed"
        else
            echo "Qt cache bootstrap unavailable; Auto walk fallback remains required"
            sed -n "1p" /tmp/gui2tui-live-qt-cache.err
        fi
    fi

    if grep -qE "phase-zero-secret|phase-two-secret" \
        /tmp/gui2tui-live-gtk-tree.txt /tmp/gui2tui-live-qt-tree.txt; then
        echo "password sentinel leaked into inspector output" >&2
        exit 1
    fi

    gtk_button=$(sed -n "s/.*Button \"Activate safely\".*id=\([^ ]*\).*/\1/p" \
        /tmp/gui2tui-live-gtk-tree.txt)
    qt_button=$(sed -n "s/.*Button \"Activate safely\".*id=\([^ ]*\).*/\1/p" \
        /tmp/gui2tui-live-qt-tree.txt)
    "$inspect" --action-name "$gtk_button" Click
    "$inspect" --action-name "$qt_button" Press
    sleep 1

    "$inspect" --app gui2tui-live-fixture | grep -q "Status: activated"
    "$inspect" --app gui2tui-qt-fixture | grep -q "Status: activated"
    echo "GTK/Qt live inspector action checks passed"
'
