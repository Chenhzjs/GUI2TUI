#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-v07c-target}
result_dir=${RESULT_DIR:-}

if [[ ${1:-} != --inside ]]; then
    if [[ -n $result_dir ]]; then
        [[ $result_dir == /* && ! -e $result_dir ]] || {
            echo 'RESULT_DIR must be an absolute path that does not exist' >&2
            exit 2
        }
        mkdir -m 700 -p -- "$result_dir"
    else
        result_dir=$(mktemp -d /tmp/gui2tui-v07c-managed-evidence.XXXXXX)
    fi
    export RESULT_DIR="$result_dir"
    exec dbus-run-session -- bash "$0" --inside
fi

((EUID != 0)) || { echo 'run the qualification harness as an unprivileged user' >&2; exit 1; }
root=$(mktemp -d /tmp/gui2tui-v07c.XXXXXX)
prefix="$root/user prefix"
bundle="$root/extracted bundle"
desktop_xvfb_pid=
desktop_unique_pid=
desktop_same_name_pid=
managed_app_pid=
managed_external_pid=
managed_supervisor_pid=

stop_exact() {
    local pid=${1:-}
    [[ -n $pid ]] || return 0
    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
}

cleanup() {
    if [[ -n $managed_supervisor_pid ]] && kill -0 "$managed_supervisor_pid" 2>/dev/null; then
        "$prefix/bin/gui2tui" setup stop >/dev/null 2>&1 || true
    fi
    stop_exact "$managed_external_pid"
    stop_exact "$managed_app_pid"
    stop_exact "$desktop_same_name_pid"
    stop_exact "$desktop_unique_pid"
    stop_exact "$desktop_xvfb_pid"
    rm -rf -- "$root"
}
trap cleanup EXIT

for binary in gui2tui gui2tui-inspect gui2tui-local; do
    [[ -x $target_dir/release/$binary ]] || {
        echo "missing release build: $target_dir/release/$binary" >&2
        exit 1
    }
done

mkdir -p "$bundle/bin" "$bundle/libexec/gui2tui"
install -m 755 "$target_dir/release/gui2tui" "$bundle/bin/"
install -m 755 "$target_dir/release/gui2tui-inspect" "$target_dir/release/gui2tui-local" \
    "$bundle/libexec/gui2tui/"
install -m 755 "$project_root/scripts/headless-session" "$bundle/libexec/gui2tui/"
install -m 755 "$project_root/scripts/install-user.sh" \
    "$project_root/scripts/uninstall-user.sh" "$bundle/"

mkdir -p "$root/home" "$root/config" "$root/runtime" "$root/state" "$prefix"
chmod 700 "$root/home" "$root/config" "$root/runtime" "$root/state"
echo preserved >"$prefix/unrelated.txt"
export HOME="$root/home"
export XDG_CONFIG_HOME="$root/config"
export XDG_RUNTIME_DIR="$root/runtime"
export XDG_STATE_HOME="$root/state"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export LANG=C.UTF-8
export TERM=xterm-256color

"$bundle/install-user.sh" --prefix "$prefix" >"$result_dir/install.txt"
gui=$prefix/bin/gui2tui
inspect=$prefix/libexec/gui2tui/gui2tui-inspect
uninstall=$prefix/libexec/gui2tui/uninstall-user
(cd /tmp && "$gui" --version >"$result_dir/version.txt")

# This is a controlled Xvfb registry used only as the explicit Desktop side of
# the isolation test. It is not evidence for an ordinary local X11 desktop.
Xvfb -displayfd 3 -screen 0 1280x900x24 -dpi 96 \
    3>"$root/desktop-display" >"$result_dir/desktop-xvfb.log" 2>&1 &
desktop_xvfb_pid=$!
for _ in {1..100}; do
    [[ -s $root/desktop-display ]] && break
    sleep 0.05
done
export DISPLAY=":$(tr -d '[:space:]' <"$root/desktop-display")"
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR \
    NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status \
        "$property" '<true>' >/dev/null
done

python3 "$project_root/tests/fixtures/gtk4_live_fixture.py" \
    >"$result_dir/desktop-unique.log" 2>&1 &
desktop_unique_pid=$!
python3 "$project_root/tests/fixtures/v05a_gtk_selection_fixture.py" \
    >"$result_dir/desktop-same-name.log" 2>&1 &
desktop_same_name_pid=$!
for _ in {1..120}; do
    if "$inspect" --session desktop --app gui2tui-live-fixture >/dev/null 2>&1 && \
       "$inspect" --session desktop --app gui2tui-v05a-gtk-selection >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done

"$gui" setup persistent >"$result_dir/setup-first.txt" 2>&1
descriptor=$XDG_STATE_HOME/gui2tui/headless/session.json
managed_supervisor_pid=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["supervisor_pid"])' "$descriptor")
managed_display=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["display"])' "$descriptor")
managed_bus=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["session_bus_address"])' "$descriptor")
test "$(stat -c %a "$descriptor")" = 600
test "$(stat -c %a "$(dirname "$descriptor")")" = 700
managed_children=$(tr ' ' '\n' <"/proc/$managed_supervisor_pid/task/$managed_supervisor_pid/children" | sed '/^$/d')

DISPLAY="$managed_display" DBUS_SESSION_BUS_ADDRESS="$managed_bus" \
    XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 \
    python3 "$project_root/tests/fixtures/v05a_gtk_selection_fixture.py" \
    >"$result_dir/managed-app-first.log" 2>&1 &
managed_app_pid=$!
for _ in {1..120}; do
    if "$inspect" --session managed --app gui2tui-v05a-gtk-selection >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done

"$inspect" --session desktop --list >"$result_dir/desktop-list.txt" 2>"$result_dir/desktop-session.txt"
"$inspect" --session managed --list >"$result_dir/managed-list.txt" 2>"$result_dir/managed-session.txt"
grep -q 'gui2tui-live-fixture' "$result_dir/desktop-list.txt"
grep -q 'gui2tui-v05a-gtk-selection' "$result_dir/desktop-list.txt"
grep -q 'gui2tui-v05a-gtk-selection' "$result_dir/managed-list.txt"
! grep -q 'gui2tui-live-fixture' "$result_dir/managed-list.txt"
test "$(grep -c 'gui2tui-v05a-gtk-selection' "$result_dir/managed-list.txt")" = 1

"$gui" --session desktop doctor --json >"$result_dir/desktop-doctor.json"
"$gui" --session managed doctor --json >"$result_dir/managed-doctor.json"
python3 - "$result_dir/desktop-doctor.json" "$result_dir/managed-doctor.json" <<'PY'
import json, sys
desktop, managed = (json.load(open(path)) for path in sys.argv[1:])
def checks(report):
    return {item["name"]: item for item in report["checks"]}
d, m = checks(desktop), checks(managed)
assert "Current desktop" in d["session-selection"]["message"]
assert "Managed headless" in m["session-selection"]["message"]
assert d["session-bus"]["level"] == m["session-bus"]["level"] == "PASS"
assert d["accessibility-registry"]["level"] == m["accessibility-registry"]["level"] == "PASS"
assert d["accessible-applications"]["level"] == m["accessible-applications"]["level"] == "PASS"
PY

export PROJECT_ROOT="$project_root" GUI2TUI="$gui" INSPECT="$inspect"
python3 - >"$result_dir/installed-tui.txt" <<'PY'
import os
import subprocess
import termios
import time

import pexpect
import pyte

GUI = os.environ["GUI2TUI"]
INSPECT = os.environ["INSPECT"]
APP = "gui2tui-v05a-gtk-selection"

def tree(session):
    return subprocess.check_output(
        [INSPECT, "--session", session, "--app", APP],
        env=os.environ,
        text=True,
        stderr=subprocess.DEVNULL,
    )

def wait_for(session, text, timeout=12):
    deadline = time.monotonic() + timeout
    last = ""
    while time.monotonic() < deadline:
        try:
            last = tree(session)
            if text in last:
                return
        except subprocess.CalledProcessError:
            pass
        time.sleep(0.05)
    raise AssertionError(f"missing {text!r} in {session} tree\n{last}")

def restored(child):
    local = termios.tcgetattr(child.child_fd)[3]
    return bool(local & termios.ICANON) and bool(local & termios.ECHO)

class Tui:
    def __init__(self):
        self.child = pexpect.spawn(
            GUI,
            [
                "--session", "managed",
                "--app", APP,
                "--layout", "flat",
                "--no-mouse",
                "--log-level", "debug",
            ],
            cwd="/tmp",
            env=os.environ.copy(),
            encoding=None,
            dimensions=(48, 160),
        )
        self.screen = pyte.Screen(160, 48)
        self.stream = pyte.Stream(self.screen)
        self.wait_text("Reorder current items", timeout=15)

    def pump(self, seconds=0.1):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline and self.child.isalive():
            try:
                data = self.child.read_nonblocking(65536, timeout=0.03)
                self.stream.feed(data.decode("utf-8", errors="replace"))
            except pexpect.TIMEOUT:
                pass
            except pexpect.EOF:
                break
        return "\n".join(self.screen.display)

    def wait_text(self, wanted, timeout=8):
        deadline = time.monotonic() + timeout
        frame = self.pump(0.05)
        while time.monotonic() < deadline:
            if wanted in frame:
                return
            frame = self.pump(0.08)
        raise AssertionError(f"terminal did not show {wanted!r}\n{frame}")

    def command(self, query):
        self.child.send(b":")
        self.wait_text("Command palette")
        self.child.send(query.encode() + b"\r")

def select_fresh():
    return Tui()

first = select_fresh()
first.command("Reorder current items")
wait_for("managed", "Structure: reordered; selected Gamma")
assert "Selected: Alpha" in tree("desktop")
first.command("Reset current items")
wait_for("managed", "Selected: Alpha")
first.child.send(b"q")
first.child.expect(pexpect.EOF, timeout=10)
assert restored(first.child)
first.child.close()
assert first.child.exitstatus == 0
print("INSTALLED_TUI_FRESH_SELECTION=PASS")
print("DYNAMIC_REFRESH_AND_CONTINUATION=PASS")
print("SEMANTIC_OPERATION_READBACK=PASS")
print("TERMINAL_NORMAL_EXIT_RESTORED=PASS")

# A second installed process must enumerate and select afresh. Stopping its
# selected Managed transport may end the process or leave it unavailable, but
# it must never redirect the old view to the same-named Desktop application.
second = select_fresh()
subprocess.run([GUI, "setup", "stop"], env=os.environ, check=True, stdout=subprocess.DEVNULL)
deadline = time.monotonic() + 4
while second.child.isalive() and time.monotonic() < deadline:
    second.pump(0.1)
if second.child.isalive():
    second.child.send(b":Reorder current items\r")
    time.sleep(1)
    second.child.send(b"\x1b")
    second.pump(0.2)
    second.child.send(b"q")
    second.child.expect(pexpect.EOF, timeout=8)
assert restored(second.child)
second.child.close()
assert "Selected: Alpha" in tree("desktop")
print("RECONNECT_REQUIRES_FRESH_SELECTION=PASS")
print("STOPPED_MANAGED_BINDING_DID_NOT_MIGRATE=PASS")
PY

for _ in {1..100}; do
    kill -0 "$managed_supervisor_pid" 2>/dev/null || break
    sleep 0.05
done
if kill -0 "$managed_supervisor_pid" 2>/dev/null; then
    echo 'managed supervisor remained after explicit stop' >&2
    exit 1
fi
test ! -e "$descriptor"
for pid in $managed_children; do
    test ! -e "/proc/$pid"
done
for _ in {1..100}; do
    kill -0 "$managed_app_pid" 2>/dev/null || break
    sleep 0.05
done
stop_exact "$managed_app_pid"
managed_app_pid=
managed_supervisor_pid=

"$gui" setup persistent >"$result_dir/setup-second.txt" 2>&1
managed_supervisor_pid=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["supervisor_pid"])' "$descriptor")
managed_display=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["display"])' "$descriptor")
managed_bus=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["session_bus_address"])' "$descriptor")
DISPLAY="$managed_display" DBUS_SESSION_BUS_ADDRESS="$managed_bus" \
    XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 \
    python3 "$project_root/tests/fixtures/v05a_gtk_selection_fixture.py" \
    >"$result_dir/managed-app-second.log" 2>&1 &
managed_app_pid=$!
for _ in {1..120}; do
    if "$inspect" --session managed --app gui2tui-v05a-gtk-selection --verbose \
        >"$result_dir/recreated-before.txt" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
reorder=$(sed -n 's/.*Button "Reorder current items".* id=\([^ ]*\).*/\1/p' \
    "$result_dir/recreated-before.txt" | head -1)
test -n "$reorder"
"$inspect" --session managed --activate "$reorder" >/dev/null 2>&1
for _ in {1..50}; do
    "$inspect" --session managed --app gui2tui-v05a-gtk-selection \
        >"$result_dir/recreated-after.txt" 2>/dev/null
    grep -q 'Structure: reordered; selected Gamma' "$result_dir/recreated-after.txt" && break
    sleep 0.1
done
grep -q 'Structure: reordered; selected Gamma' "$result_dir/recreated-after.txt"

# Reuse the existing bounded complex-text probe against the installed binary.
# This proves one real foreground handler handoff and authoritative AT-SPI
# writeback without expanding the phase into the v0.6 lifecycle campaign.
DISPLAY="$managed_display" DBUS_SESSION_BUS_ADDRESS="$managed_bus" \
    XDG_SESSION_TYPE=x11 NO_AT_BRIDGE=0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 \
    python3 "$project_root/tests/fixtures/gtk4_live_fixture.py" \
    >"$result_dir/managed-external-app.log" 2>&1 &
managed_external_pid=$!
for _ in {1..120}; do
    if "$inspect" --session managed --app gui2tui-live-fixture >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done
GUI2TUI_VALIDATION_HANDLER_MODE=positive \
GUI2TUI_VALIDATION_APP=gui2tui-live-fixture \
    python3 "$project_root/tests/live/v03c_tui_probe.py" \
    >"$result_dir/external-handler.txt"
grep -q 'EXTERNAL_TEXT_END_TO_END=PASS' "$result_dir/external-handler.txt"

"$gui" setup stop >"$result_dir/stop-second.txt" 2>&1
for _ in {1..100}; do
    kill -0 "$managed_supervisor_pid" 2>/dev/null || break
    sleep 0.05
done
test ! -e "/proc/$managed_supervisor_pid"
test ! -e "$descriptor"
stop_exact "$managed_external_pid"
managed_external_pid=
stop_exact "$managed_app_pid"
managed_app_pid=
managed_supervisor_pid=

"$uninstall" --prefix "$prefix" >"$result_dir/uninstall.txt"
test ! -e "$prefix/bin/gui2tui"
test -f "$prefix/unrelated.txt"

{
    echo 'installed_release_binary=PASS'
    echo 'arbitrary_working_directory=PASS'
    echo 'desktop_managed_registry_isolation=PASS'
    echo 'same_name_cross_session_isolation=PASS'
    echo 'desktop_doctor=PASS_CONTROLLED_XVFB'
    echo 'managed_doctor=PASS'
    echo 'fresh_application_selection=PASS'
    echo 'dynamic_refresh=PASS'
    echo 'semantic_operation_readback=PASS'
    echo 'terminal_navigation=PASS_COMMAND_PALETTE'
    echo 'terminal_normal_exit_restored=PASS'
    echo 'managed_reconnect=PASS'
    echo 'stopped_binding_did_not_migrate=PASS'
    echo 'managed_stop_cleanup=PASS'
    echo 'managed_recreate_operation=PASS'
    echo 'managed_external_handler_handoff=PASS'
    echo 'safe_uninstall=PASS'
    echo 'real_local_x11=NOT_TESTED'
    echo 'ssh_existing_desktop=NOT_TESTED'
    echo 'ssh_managed_headless=NOT_TESTED'
    echo 'ssh_external_handler=NOT_TESTED'
} >"$result_dir/summary.txt"
cat "$result_dir/summary.txt"
