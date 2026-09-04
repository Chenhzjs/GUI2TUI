#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-121}

export PROJECT_ROOT="$project_root"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI_VALIDATION_DISPLAY=":$display_number"

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v03d-expand.XXXXXX)
    export DISPLAY="$GUI2TUI_VALIDATION_DISPLAY"
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export NO_AT_BRIDGE=0

    xvfb_pid=
    demo_pid=
    watcher_pid=
    cleanup() {
        for pid in "$watcher_pid" "$demo_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x900x24 >/tmp/gui2tui-v03d-expand-xvfb.log 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    gtk4-demo >/tmp/gui2tui-v03d-expand-gtk.log 2>&1 &
    demo_pid=$!
    sleep 3

    "$INSPECT" --app gtk4-demo --verbose >/tmp/gui2tui-v03d-expand-before.tree
    target=$(sed -n "s/.*Button \"Constraints\".*id=\([^ ]*\).*/\1/p" \
        /tmp/gui2tui-v03d-expand-before.tree)
    test -n "$target"
    grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-before.tree
    grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-before.tree \
        | grep -q "expanded"
    grep -q "Simple Constraints" /tmp/gui2tui-v03d-expand-before.tree

    timeout 8 "$INSPECT" --app gtk4-demo --watch-events \
        >/tmp/gui2tui-v03d-expand.events 2>&1 &
    watcher_pid=$!
    sleep 1

    "$INSPECT" --action-name "$target" listitem.collapse
    sleep 1
    "$INSPECT" --app gtk4-demo --verbose >/tmp/gui2tui-v03d-expand-collapsed.tree
    grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-collapsed.tree
    if grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-collapsed.tree \
        | grep -q "expanded"; then
        echo "EXPAND_COLLAPSE_STATE_READBACK=FAIL"
        exit 1
    fi
    if grep -q "Simple Constraints" /tmp/gui2tui-v03d-expand-collapsed.tree; then
        echo "EXPAND_COLLAPSE_CHILD_REFUSAL=FAIL"
        exit 1
    fi

    "$INSPECT" --action-name "$target" listitem.expand
    sleep 1
    "$INSPECT" --app gtk4-demo --verbose >/tmp/gui2tui-v03d-expand-restored.tree
    grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-restored.tree
    grep "Button \"Constraints\"" /tmp/gui2tui-v03d-expand-restored.tree \
        | grep -q "expanded"
    grep -q "Simple Constraints" /tmp/gui2tui-v03d-expand-restored.tree

    wait "$watcher_pid" || true
    watcher_pid=
    grep -Ei "expanded|children" /tmp/gui2tui-v03d-expand.events | head -20 || true
    echo "EXPAND_COLLAPSE_EXPLICIT_ACTION=PASS"
    echo "EXPAND_COLLAPSE_STATE_READBACK=PASS"
    echo "EXPAND_COLLAPSE_REALIZATION=PASS"
    echo "EXPAND_COLLAPSE_RESTORATION=PASS"
'
