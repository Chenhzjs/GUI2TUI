#!/usr/bin/env bash
set -euo pipefail

mode=${1:?usage: v03c_run_linux.sh MODE [DISPLAY_NUMBER] [gtk|mousepad|writer]}
display_number=${2:-114}
application_kind=${3:-gtk}
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-live-target}

export PROJECT_ROOT="$project_root"
export GUI2TUI="$target_dir/debug/gui2tui"
export INSPECT="$target_dir/debug/gui2tui-inspect"
export GUI2TUI_VALIDATION_HANDLER_MODE="$mode"
export GUI2TUI_VALIDATION_DISPLAY=":$display_number"
export GUI2TUI_VALIDATION_APPLICATION_KIND="$application_kind"

dbus-run-session -- bash -euo pipefail -c '
    runtime_dir=$(mktemp -d "/tmp/gui2tui-v03c-${GUI2TUI_VALIDATION_HANDLER_MODE}.XXXXXX")
    export DISPLAY="$GUI2TUI_VALIDATION_DISPLAY"
    export XDG_SESSION_TYPE=x11
    export XDG_RUNTIME_DIR="$runtime_dir"
    export NO_AT_BRIDGE=0
    export GUI2TUI_VALIDATION_HANDLER_READY="$runtime_dir/handler-ready"
    export GUI2TUI_VALIDATION_HANDLER_RESUME="$runtime_dir/handler-resume"

    xvfb_pid=
    application_pid=
    cleanup() {
        for pid in "$application_pid" "$xvfb_pid"; do
            if [[ -n "$pid" ]]; then
                kill "$pid" 2>/dev/null || true
                wait "$pid" 2>/dev/null || true
            fi
        done
        rm -r -- "$runtime_dir"
    }
    trap cleanup EXIT

    Xvfb "$DISPLAY" -screen 0 1280x800x24 \
        >"/tmp/gui2tui-v03c-${GUI2TUI_VALIDATION_HANDLER_MODE}-xvfb.log" 2>&1 &
    xvfb_pid=$!
    dbus-update-activation-environment \
        DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR NO_AT_BRIDGE
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status IsEnabled "<true>" >/dev/null
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set \
        org.a11y.Status ScreenReaderEnabled "<true>" >/dev/null

    case "$GUI2TUI_VALIDATION_APPLICATION_KIND" in
        gtk)
            export GUI2TUI_VALIDATION_APP=gui2tui-live-fixture
            python3 "$PROJECT_ROOT/tests/fixtures/gtk4_live_fixture.py" \
                >"/tmp/gui2tui-v03c-${GUI2TUI_VALIDATION_HANDLER_MODE}-gtk.log" 2>&1 &
            application_pid=$!
            ;;
        mousepad)
            export GUI2TUI_VALIDATION_APP=mousepad
            cp "$PROJECT_ROOT/tests/fixtures/spatial_review.txt" "$runtime_dir/mousepad.txt"
            cp "$runtime_dir/mousepad.txt" "$runtime_dir/mousepad.expected"
            mkdir -p "$runtime_dir/app-data" "$runtime_dir/app-config" "$runtime_dir/app-cache"
            XDG_DATA_HOME="$runtime_dir/app-data" \
                XDG_CONFIG_HOME="$runtime_dir/app-config" \
                XDG_CACHE_HOME="$runtime_dir/app-cache" \
                GSETTINGS_BACKEND=memory \
                mousepad --disable-server "$runtime_dir/mousepad.txt" \
                >"/tmp/gui2tui-v03c-${GUI2TUI_VALIDATION_HANDLER_MODE}-mousepad.log" 2>&1 &
            application_pid=$!
            ;;
        writer)
            export GUI2TUI_VALIDATION_APP=soffice
            cp "$PROJECT_ROOT/tests/fixtures/libreoffice_content_fixture.fodt" \
                "$runtime_dir/writer.fodt"
            libreoffice --writer --nologo --nodefault --norestore --nolockcheck \
                "$runtime_dir/writer.fodt" \
                >"/tmp/gui2tui-v03c-${GUI2TUI_VALIDATION_HANDLER_MODE}-writer.log" 2>&1 &
            application_pid=$!
            ;;
        *)
            echo "unsupported validation application: $GUI2TUI_VALIDATION_APPLICATION_KIND" >&2
            exit 2
            ;;
    esac
    sleep 2
    python3 "$PROJECT_ROOT/tests/live/v03c_tui_probe.py"
    if [[ "$GUI2TUI_VALIDATION_APPLICATION_KIND" == mousepad ]]; then
        cmp "$runtime_dir/mousepad.expected" "$runtime_dir/mousepad.txt"
        echo "EXTERNAL_TEXT_BACKING_FILE_BYPASS=ABSENT"
    fi
'
