#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-v07a-target}
result_dir=${RESULT_DIR:-}

if [[ ${1:-} != --inside ]]; then
    if [[ -n $result_dir ]]; then
        [[ $result_dir == /* && ! -e $result_dir ]] || {
            echo 'RESULT_DIR must be an absolute path that does not exist' >&2
            exit 2
        }
        mkdir -m 700 -p -- "$result_dir"
    else
        result_dir=$(mktemp -d /tmp/gui2tui-v07a-session-selection.XXXXXX)
    fi
    export RESULT_DIR="$result_dir"
    exec dbus-run-session -- bash "$0" --inside
fi

gui=$target_dir/debug/gui2tui
inspect=$target_dir/debug/gui2tui-inspect
session_root=$(mktemp -d /tmp/gui2tui-v07a.XXXXXX)
desktop_xvfb_pid=
desktop_app_pid=
managed_app_pid=
managed_supervisor_pid=

cleanup() {
    if [[ -n $managed_supervisor_pid ]] && kill -0 "$managed_supervisor_pid" 2>/dev/null; then
        "$gui" setup stop >/dev/null 2>&1 || true
    fi
    for pid in "$managed_app_pid" "$desktop_app_pid" "$desktop_xvfb_pid"; do
        if [[ -n $pid ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    rm -rf -- "$session_root"
}
trap cleanup EXIT

reject_text() {
    local pattern=$1 file=$2
    if grep -q -- "$pattern" "$file"; then
        echo "unexpected text '$pattern' in $file" >&2
        exit 1
    fi
}

mkdir -p "$session_root/home" "$session_root/config" "$session_root/runtime" "$session_root/state"
chmod 700 "$session_root/home" "$session_root/config" "$session_root/runtime" "$session_root/state"
export HOME="$session_root/home"
export XDG_CONFIG_HOME="$session_root/config"
export XDG_RUNTIME_DIR="$session_root/runtime"
export XDG_STATE_HOME="$session_root/state"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1

if [[ ! -e $target_dir/debug/headless-session ]]; then
    ln -s "$project_root/scripts/headless-session" "$target_dir/debug/headless-session"
fi

Xvfb -displayfd 3 -screen 0 1280x900x24 -dpi 96 \
    3>"$session_root/desktop-display" >"$result_dir/desktop-xvfb.log" 2>&1 &
desktop_xvfb_pid=$!
for _ in {1..100}; do
    [[ -s $session_root/desktop-display ]] && break
    sleep 0.05
done
export DISPLAY=":$(tr -d '[:space:]' <"$session_root/desktop-display")"
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR \
    NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status \
        "$property" '<true>' >/dev/null
done

python3 "$project_root/tests/fixtures/gtk4_live_fixture.py" \
    >"$result_dir/desktop-app.log" 2>&1 &
desktop_app_pid=$!
for _ in {1..120}; do
    if "$inspect" --session desktop --app gui2tui-live-fixture >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done
"$inspect" --session desktop --app gui2tui-live-fixture >/dev/null

"$gui" setup persistent >"$result_dir/setup.txt" 2>&1
descriptor="$XDG_STATE_HOME/gui2tui/headless/session.json"
managed_supervisor_pid=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["supervisor_pid"])' "$descriptor")
managed_display=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["display"])' "$descriptor")
managed_bus=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["session_bus_address"])' "$descriptor")

DISPLAY="$managed_display" DBUS_SESSION_BUS_ADDRESS="$managed_bus" \
    XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 \
    python3 "$project_root/tests/fixtures/v05a_gtk_selection_fixture.py" \
    >"$result_dir/managed-app.log" 2>&1 &
managed_app_pid=$!
for _ in {1..120}; do
    if "$inspect" --session managed --app gui2tui-v05a-gtk-selection >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done
"$inspect" --session managed --app gui2tui-v05a-gtk-selection >/dev/null

"$inspect" --session desktop --list >"$result_dir/desktop-list.txt" \
    2>"$result_dir/desktop-status.txt"
"$inspect" --session managed --list >"$result_dir/managed-list.txt" \
    2>"$result_dir/managed-status.txt"
"$inspect" --list >"$result_dir/default-list.txt" 2>"$result_dir/default-status.txt"
GUI2TUI_NO_MANAGED_SESSION=1 "$inspect" --list \
    >"$result_dir/opt-out-list.txt" 2>"$result_dir/opt-out-status.txt"

grep -q 'gui2tui-live-fixture' "$result_dir/desktop-list.txt"
reject_text 'gui2tui-v05a-gtk-selection' "$result_dir/desktop-list.txt"
grep -q 'gui2tui-v05a-gtk-selection' "$result_dir/managed-list.txt"
reject_text 'gui2tui-live-fixture' "$result_dir/managed-list.txt"
cmp "$result_dir/managed-list.txt" "$result_dir/default-list.txt"
cmp "$result_dir/desktop-list.txt" "$result_dir/opt-out-list.txt"
grep -q 'Session: Current desktop (explicit --session desktop)' "$result_dir/desktop-status.txt"
grep -q 'Session: Managed headless (explicit --session managed)' "$result_dir/managed-status.txt"
grep -q 'compatible default; existing managed descriptor' "$result_dir/default-status.txt"

"$gui" --session desktop doctor --json >"$result_dir/desktop-doctor.json"
"$gui" --session managed doctor --json >"$result_dir/managed-doctor.json"
python3 - "$result_dir/desktop-doctor.json" "$result_dir/managed-doctor.json" <<'PY'
import json, sys
desktop, managed = (json.load(open(path)) for path in sys.argv[1:])
def message(report, name):
    return next(item["message"] for item in report["checks"] if item["name"] == name)
assert "Current desktop (explicit --session desktop)" in message(desktop, "session-selection")
assert "Managed headless (explicit --session managed)" in message(managed, "session-selection")
assert message(desktop, "accessible-applications").startswith("1 accessible")
assert message(managed, "accessible-applications").startswith("1 accessible")
PY

"$inspect" --session managed --app gui2tui-v05a-gtk-selection --verbose \
    >"$result_dir/managed-before.txt" 2>/dev/null
reorder=$(sed -n 's/.*Button "Reorder current items".* id=\([^ ]*\).*/\1/p' \
    "$result_dir/managed-before.txt" | head -1)
test -n "$reorder"
"$inspect" --session managed --activate "$reorder" \
    >"$result_dir/managed-operation.txt" 2>"$result_dir/managed-operation-status.txt"
for _ in {1..50}; do
    "$inspect" --session managed --app gui2tui-v05a-gtk-selection \
        >"$result_dir/managed-after.txt" 2>/dev/null
    grep -q 'Structure: reordered; selected Gamma' "$result_dir/managed-after.txt" && break
    sleep 0.1
done
grep -q 'Structure: reordered; selected Gamma' "$result_dir/managed-after.txt"

cp "$descriptor" "$session_root/valid-session.json"
"$gui" setup stop >"$result_dir/stop.txt" 2>&1
wait "$managed_app_pid" 2>/dev/null || true
managed_app_pid=
if kill -0 "$managed_supervisor_pid" 2>/dev/null; then
    echo 'managed supervisor remained alive after stop' >&2
    exit 1
fi
test ! -e "$descriptor"

if "$inspect" --session managed --list >"$result_dir/missing.out" \
    2>"$result_dir/missing.err"; then
    echo 'explicit managed unexpectedly accepted a missing descriptor' >&2
    exit 1
fi
grep -q 'descriptor does not exist' "$result_dir/missing.err"

cp "$session_root/valid-session.json" "$descriptor"
chmod 600 "$descriptor"
if "$inspect" --session managed --list >"$result_dir/stopped.out" \
    2>"$result_dir/stopped.err"; then
    echo 'explicit managed unexpectedly accepted a stopped supervisor' >&2
    exit 1
fi
grep -q 'supervisor is not running' "$result_dir/stopped.err"

"$inspect" --list >"$result_dir/stale-default-list.txt" \
    2>"$result_dir/stale-default-status.txt"
cmp "$result_dir/desktop-list.txt" "$result_dir/stale-default-list.txt"
grep -q 'Current desktop (compatible fallback; managed descriptor unusable)' \
    "$result_dir/stale-default-status.txt"
test -e "$descriptor"

printf '{invalid\n' >"$descriptor"
chmod 600 "$descriptor"
if "$inspect" --session managed --list >"$result_dir/invalid.out" \
    2>"$result_dir/invalid.err"; then
    echo 'explicit managed unexpectedly accepted invalid JSON' >&2
    exit 1
fi
grep -q 'descriptor is invalid' "$result_dir/invalid.err"

cp "$session_root/valid-session.json" "$descriptor"
chmod 644 "$descriptor"
if "$inspect" --session managed --list >"$result_dir/unsafe.out" \
    2>"$result_dir/unsafe.err"; then
    echo 'explicit managed unexpectedly accepted unsafe descriptor permissions' >&2
    exit 1
fi
grep -q 'current-user owned, private' "$result_dir/unsafe.err"

python3 - "$descriptor" "$$" "$DISPLAY" <<'PY'
import json, os, sys
path, pid, display = sys.argv[1:]
with open(path, "w") as handle:
    json.dump({"schema_version": 1, "supervisor_pid": int(pid), "display": display,
               "session_bus_address": "unix:path=/nonexistent/gui2tui-v07a-bus"}, handle)
    handle.write("\n")
os.chmod(path, 0o600)
PY
if "$inspect" --session managed --list >"$result_dir/unavailable.out" \
    2>"$result_dir/unavailable.err"; then
    echo 'explicit managed unexpectedly connected to an unavailable bus' >&2
    exit 1
fi
grep -q 'Session: Managed headless (explicit --session managed)' "$result_dir/unavailable.err"
grep -q 'DBUS_SESSION_BUS_ADDRESS=<set>' "$result_dir/unavailable.err"
reject_text '/nonexistent/gui2tui-v07a-bus' "$result_dir/unavailable.err"

rm -f "$descriptor"
{
    echo 'desktop_selection=PASS'
    echo 'managed_selection=PASS'
    echo 'descriptor_precedence=PASS'
    echo 'missing_descriptor=PASS'
    echo 'invalid_descriptor=PASS'
    echo 'stopped_descriptor=PASS'
    echo 'unsafe_descriptor=PASS'
    echo 'unavailable_connection=PASS'
    echo 'zero_app_distinction=PASS'
    echo 'fresh_application_selection=PASS'
    echo 'semantic_operation=PASS'
    echo 'cleanup=PASS'
    echo 'real_local_x11=NOT_TESTED'
} >"$result_dir/summary.txt"
cat "$result_dir/summary.txt"
