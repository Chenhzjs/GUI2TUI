#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-124}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI="$target_dir/debug/gui2tui"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v04a.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    xvfb_pid=
    qt_pid=
    gtk_fixture_pid=
    demo_pid=
    watcher_pid=
    cleanup() {
        for pid in "$watcher_pid" "$demo_pid" "$gtk_fixture_pid" "$qt_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x900x24 >/tmp/gui2tui-v04a-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/fixtures/qt6_live_fixture.py" \
        >/tmp/gui2tui-v04a-qt.log 2>&1 &
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
        >/tmp/gui2tui-v04a-gtk-fixture.log 2>&1 &
    gtk_fixture_pid=$!
    deadline=$((SECONDS + 12))
    until "$INSPECT" --app gui2tui-live-fixture >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "GTK fixture did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    python3 "$PROJECT_ROOT/tests/live/v04a_transition_observation.py"

    gtk4-demo >/tmp/gui2tui-v04a-gtk.log 2>&1 &
    demo_pid=$!
    deadline=$((SECONDS + 12))
    until "$INSPECT" --app gtk4-demo --verbose \
        >/tmp/gui2tui-v04a-disclosure-before.tree 2>/dev/null; do
        if (( SECONDS >= deadline )); then
            echo "GTK Demo did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done
    target=$(sed -n "s/.*Button \"Constraints\".*id=\([^ ]*\).*/\1/p" \
        /tmp/gui2tui-v04a-disclosure-before.tree)
    test -n "$target"
    grep "Button \"Constraints\"" /tmp/gui2tui-v04a-disclosure-before.tree \
        | grep -q "expanded"

    timeout 8 "$INSPECT" --app gtk4-demo --watch-events \
        >/tmp/gui2tui-v04a-disclosure.events 2>&1 &
    watcher_pid=$!
    "$INSPECT" --action-name "$target" listitem.collapse >/dev/null
    deadline=$((SECONDS + 5))
    while :; do
        "$INSPECT" --app gtk4-demo --verbose \
            >/tmp/gui2tui-v04a-disclosure-collapsed.tree
        if ! grep "Button \"Constraints\"" \
            /tmp/gui2tui-v04a-disclosure-collapsed.tree | grep -q "expanded" \
            && ! grep -q "Simple Constraints" \
                /tmp/gui2tui-v04a-disclosure-collapsed.tree; then
            break
        fi
        if (( SECONDS >= deadline )); then
            echo "Disclosure collapse was not authoritatively confirmed" >&2
            exit 1
        fi
        sleep 0.05
    done

    "$INSPECT" --action-name "$target" listitem.expand >/dev/null
    deadline=$((SECONDS + 5))
    while :; do
        "$INSPECT" --app gtk4-demo --verbose \
            >/tmp/gui2tui-v04a-disclosure-restored.tree
        if grep "Button \"Constraints\"" \
            /tmp/gui2tui-v04a-disclosure-restored.tree | grep -q "expanded" \
            && grep -q "Simple Constraints" \
                /tmp/gui2tui-v04a-disclosure-restored.tree; then
            break
        fi
        if (( SECONDS >= deadline )); then
            echo "Disclosure restoration was not authoritatively confirmed" >&2
            exit 1
        fi
        sleep 0.05
    done
    kill "$watcher_pid" 2>/dev/null || true
    wait "$watcher_pid" 2>/dev/null || true
    watcher_pid=
    grep -Eqi "expanded|children" /tmp/gui2tui-v04a-disclosure.events
    echo "TRANSITION_DISCLOSURE_STATE_CONFIRMATION=PASS"

    cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" \
        --lib tui::transition::tests
'
