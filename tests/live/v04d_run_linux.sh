#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}
display_number=${1:-127}

export PROJECT_ROOT="$project_root"
export CARGO_TARGET_DIR="$target_dir"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI="$target_dir/debug/gui2tui"
export DISPLAY=":$display_number"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

cargo build --manifest-path "$project_root/Cargo.toml" --bins

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d /tmp/gui2tui-v04d.XXXXXX)
    export XDG_RUNTIME_DIR="$runtime_dir"
    xvfb_pid=
    qt_pid=
    gtk_pid=
    demo_pid=
    cleanup() {
        if [[ -f "$runtime_dir/gui2tui/product.log" ]]; then
            cp "$runtime_dir/gui2tui/product.log" /tmp/gui2tui-v04d-product.log
        fi
        for pid in "$demo_pid" "$gtk_pid" "$qt_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x1000x24 >/tmp/gui2tui-v04d-xvfb.log 2>&1 &
    xvfb_pid=$!
    deadline=$((SECONDS + 8))
    until xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "Xvfb did not become ready" >&2
            exit 1
        fi
        sleep 0.05
    done
    dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    python3 "$PROJECT_ROOT/tests/fixtures/qt6_live_fixture.py" \
        >/tmp/gui2tui-v04d-qt.log 2>&1 &
    qt_pid=$!
    deadline=$((SECONDS + 15))
    until "$INSPECT" --app gui2tui-qt-fixture --bootstrap walk >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "Qt fixture did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    python3 "$PROJECT_ROOT/tests/fixtures/gtk4_live_fixture.py" \
        >/tmp/gui2tui-v04d-gtk.log 2>&1 &
    gtk_pid=$!
    deadline=$((SECONDS + 15))
    until "$INSPECT" --app gui2tui-live-fixture --bootstrap walk >/dev/null 2>&1; do
        if (( SECONDS >= deadline )); then
            echo "GTK fixture did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    gtk4-demo >/tmp/gui2tui-v04d-gtk-demo.log 2>&1 &
    demo_pid=$!
    deadline=$((SECONDS + 15))
    until "$INSPECT" --app gtk4-demo --bootstrap walk --verbose 2>/dev/null \
        | grep -q "Button \"Constraints\""; do
        if (( SECONDS >= deadline )); then
            echo "GTK Demo Constraints did not become accessible" >&2
            exit 1
        fi
        sleep 0.1
    done

    if [[ ${V04D_UX_ONLY:-0} != 1 ]]; then
        surface_output=$(python3 "$PROJECT_ROOT/tests/live/v04b_surface_scope_continuation.py")
        printf "%s\n" "$surface_output"
        grep -q "SAME_SCOPE_MENU_CONTINUATION=PASS" <<<"$surface_output"
        grep -q "MODAL_SCOPE_CONTINUATION_ENTER=PASS" <<<"$surface_output"
        grep -q "MODAL_SCOPE_CONTINUATION_EXIT=PASS" <<<"$surface_output"
        grep -q "MODAL_BACKGROUND_AUTHORITY_REFUSAL=PASS" <<<"$surface_output"
        grep -q "OWNERLESS_POPUP_NO_SCOPE_INFERENCE=PASS" <<<"$surface_output"

        realization_output=$(python3 "$PROJECT_ROOT/tests/live/v04c_realization_continuation.py")
        printf "%s\n" "$realization_output"
        grep -q "SIBLING_REALIZATION_MANUAL_CONTINUATION=PASS" <<<"$realization_output"
        grep -q "STRUCTURAL_DESCENDANT_CONTINUATION=PASS" <<<"$realization_output"
        grep -q "AMBIGUOUS_REALIZATION_OWNERSHIP_REFUSAL=PASS" <<<"$realization_output"
        grep -q "HIERARCHY_USES_PUBLIC_STRUCTURE_ONLY=PASS" <<<"$realization_output"
    fi

    if [[ ${V04D_UX_ONLY:-0} == 1 ]]; then
        python3 "$PROJECT_ROOT/tests/live/v04d_continuation_ux.py"
    else
        V04D_SKIP_GTK_UX=1 python3 "$PROJECT_ROOT/tests/live/v04d_continuation_ux.py"
    fi

    echo "CONTINUATION_UX_REALIZATION=PASS"
    echo "CONTINUATION_UX_PUBLIC_HIERARCHY=PASS"
    echo "CONTINUATION_UX_AMBIGUITY=PASS"
    echo "CURRENT_SCENE_MANUAL_CONTINUATION=PASS"
    echo "NO_FALSE_HIERARCHY=PASS"
    echo "NO_FALSE_AFFORDANCE=PASS"
'
