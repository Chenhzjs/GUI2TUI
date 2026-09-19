#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
target_dir=${CARGO_TARGET_DIR:-/tmp/gui2tui-v07b-target}
result_dir=${RESULT_DIR:-}

if [[ ${1:-} != --inside ]]; then
    if [[ -n $result_dir ]]; then
        [[ $result_dir == /* && ! -e $result_dir ]] || {
            echo 'RESULT_DIR must be an absolute path that does not exist' >&2
            exit 2
        }
        mkdir -m 700 -p -- "$result_dir"
    else
        result_dir=$(mktemp -d /tmp/gui2tui-v07b-install.XXXXXX)
    fi
    export RESULT_DIR="$result_dir"
    exec dbus-run-session -- bash "$0" --inside
fi

((EUID != 0)) || { echo 'run the installation harness as an unprivileged user' >&2; exit 1; }
root=$(mktemp -d /tmp/gui2tui-v07b.XXXXXX)
prefix="$root/user prefix"
bundle="$root/extracted bundle"
xvfb_pid=
app_pid=
managed_pid=

cleanup() {
    if [[ -n $managed_pid ]] && kill -0 "$managed_pid" 2>/dev/null; then
        "$prefix/bin/gui2tui" setup stop >/dev/null 2>&1 || true
    fi
    for pid in "$app_pid" "$xvfb_pid"; do
        if [[ -n $pid ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    rm -rf -- "$root"
}
trap cleanup EXIT

for binary in gui2tui gui2tui-inspect gui2tui-local; do
    [[ -x $target_dir/release/$binary ]] || { echo "missing release build: $target_dir/release/$binary" >&2; exit 1; }
done
mkdir -p "$bundle/bin" "$bundle/libexec/gui2tui"
install -m 755 "$target_dir/release/gui2tui" "$bundle/bin/"
install -m 755 "$target_dir/release/gui2tui-inspect" "$target_dir/release/gui2tui-local" \
    "$bundle/libexec/gui2tui/"
install -m 755 "$project_root/scripts/headless-session" "$bundle/libexec/gui2tui/"
install -m 755 "$project_root/scripts/install-user.sh" "$project_root/scripts/uninstall-user.sh" "$bundle/"

mkdir -p "$root/home" "$root/config" "$root/runtime" "$root/state" "$prefix"
chmod 700 "$root/home" "$root/config" "$root/runtime" "$root/state"
echo unrelated >"$prefix/unrelated.txt"
export HOME="$root/home"
export XDG_CONFIG_HOME="$root/config"
export XDG_RUNTIME_DIR="$root/runtime"
export XDG_STATE_HOME="$root/state"
export XDG_SESSION_TYPE=x11
export NO_AT_BRIDGE=0
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
export LANG=C.UTF-8

"$bundle/install-user.sh" --prefix "$prefix" >"$result_dir/install.txt"
gui=$prefix/bin/gui2tui
inspect=$prefix/libexec/gui2tui/gui2tui-inspect
uninstall=$prefix/libexec/gui2tui/uninstall-user
test -x "$gui" -a -x "$inspect" -a -x "$prefix/libexec/gui2tui/gui2tui-local"
test -x "$prefix/libexec/gui2tui/headless-session" -a -x "$uninstall"
test "$(stat -c %a "$prefix/libexec/gui2tui/install-manifest-v1")" = 600
(cd / && "$gui" --version >"$result_dir/version.txt")
(cd /tmp && "$gui" inspect --help >"$result_dir/inspect-help.txt")
(cd /tmp && "$gui" endpoint --help >"$result_dir/endpoint-help.txt")

Xvfb -displayfd 3 -screen 0 1280x900x24 -dpi 96 \
    3>"$root/display" >"$result_dir/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in {1..100}; do
    [[ -s $root/display ]] && break
    sleep 0.05
done
export DISPLAY=":$(tr -d '[:space:]' <"$root/display")"
dbus-update-activation-environment DISPLAY XDG_SESSION_TYPE XDG_RUNTIME_DIR \
    NO_AT_BRIDGE QT_LINUX_ACCESSIBILITY_ALWAYS_ON
for property in IsEnabled ScreenReaderEnabled; do
    gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
        --method org.freedesktop.DBus.Properties.Set org.a11y.Status \
        "$property" '<true>' >/dev/null
done

"$gui" --session desktop doctor --json >"$result_dir/doctor-zero-apps.json"
python3 - "$result_dir/doctor-zero-apps.json" <<'PY'
import json, sys
report = json.load(open(sys.argv[1]))
checks = {item["name"]: item for item in report["checks"]}
assert checks["installation-entry"]["level"] == "PASS"
assert checks["helper-inspector"]["level"] == "PASS"
assert checks["helper-managed-headless"]["level"] == "PASS"
assert checks["helper-same-host-modality"]["level"] == "PASS"
assert checks["user-prefix-install"]["level"] == "PASS"
assert checks["accessibility-registry"]["level"] == "PASS"
assert checks["accessible-applications"]["level"] == "WARN"
assert "distinct from a session" in checks["application-semantics"]["message"]
assert checks["external-text-handler"]["level"] == "INFO"
PY

if command -v script >/dev/null 2>&1; then
    export GUI2TUI_TEST_GUI="$gui"
    script -qec 'stty rows 40 cols 120; exec "$GUI2TUI_TEST_GUI" --session desktop doctor --json' \
        /dev/null >"$result_dir/doctor-pty.json"
    python3 - "$result_dir/doctor-pty.json" <<'PY'
import json, sys
text = open(sys.argv[1]).read().replace("\r", "")
report = json.loads(text[text.index("{"):])
checks = {item["name"]: item for item in report["checks"]}
assert checks["terminal-interactive"]["level"] == "PASS"
assert checks["terminal-type"]["level"] == "PASS"
assert checks["terminal-utf8"]["level"] == "PASS"
assert checks["terminal-size"]["level"] == "PASS"
assert checks["terminal-modes"]["level"] == "INFO"
PY
    terminal_evidence=PASS
else
    terminal_evidence=NOT_TESTED
fi

mkdir -p "$XDG_CONFIG_HOME/gui2tui"
handler="$root/handler-not-run"
cat >"$handler" <<'SH'
#!/usr/bin/env bash
touch "$HOME/doctor-started-handler"
SH
chmod 755 "$handler"
cat >"$XDG_CONFIG_HOME/gui2tui/config.toml" <<EOF
version = 1
[interaction.complex_text]
program = "$handler"
args = ["--wait", "{file}"]
EOF
chmod 600 "$XDG_CONFIG_HOME/gui2tui/config.toml"
"$gui" --session desktop doctor --json >"$result_dir/doctor-handler-valid.json"
test ! -e "$HOME/doctor-started-handler"
python3 - "$result_dir/doctor-handler-valid.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["external-text-handler"]["level"] == "PASS"
PY
sed -i "s|program = .*|program = \"$root/missing-handler\"|" "$XDG_CONFIG_HOME/gui2tui/config.toml"
"$gui" --session desktop doctor --json >"$result_dir/doctor-handler-missing.json"
python3 - "$result_dir/doctor-handler-missing.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["external-text-handler"]["level"] == "WARN"
PY
rm -f "$XDG_CONFIG_HOME/gui2tui/config.toml"

chmod 644 "$inspect"
if "$gui" --session desktop doctor --json >"$result_dir/doctor-helper-missing.json"; then
    echo 'Doctor unexpectedly accepted a non-executable required inspector' >&2
    exit 1
fi
python3 - "$result_dir/doctor-helper-missing.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["helper-inspector"]["level"] == "FAIL"
PY
chmod 755 "$inspect"

if DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/gui2tui-v07b-bus \
    "$gui" --session desktop doctor --json >"$result_dir/doctor-no-session-bus.json"; then
    echo 'Doctor unexpectedly accepted an unavailable selected session bus' >&2
    exit 1
fi
python3 - "$result_dir/doctor-no-session-bus.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["session-bus"]["level"] == "FAIL"
assert checks["accessibility-bus"]["level"] == "INFO"
assert checks["accessible-applications"]["level"] == "INFO"
PY

python3 "$project_root/tests/fixtures/v05a_gtk_selection_fixture.py" \
    >"$result_dir/app.log" 2>&1 &
app_pid=$!
for _ in {1..120}; do
    if "$inspect" --session desktop --app gui2tui-v05a-gtk-selection >/dev/null 2>&1; then
        break
    fi
    sleep 0.1
done
"$gui" --session desktop doctor --json >"$result_dir/doctor-with-app.json"
python3 - "$result_dir/doctor-with-app.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["accessible-applications"]["level"] == "PASS"
assert checks["application-semantics"]["level"] == "INFO"
assert "limitation" in checks["application-semantics"]["message"]
PY
"$inspect" --session desktop --app gui2tui-v05a-gtk-selection --verbose \
    >"$result_dir/before.txt" 2>/dev/null
button=$(sed -n 's/.*Button "Reorder current items".* id=\([^ ]*\).*/\1/p' \
    "$result_dir/before.txt" | head -1)
test -n "$button"
"$inspect" --session desktop --activate "$button" >/dev/null 2>&1
for _ in {1..50}; do
    "$inspect" --session desktop --app gui2tui-v05a-gtk-selection \
        >"$result_dir/after.txt" 2>/dev/null
    grep -q 'Structure: reordered; selected Gamma' "$result_dir/after.txt" && break
    sleep 0.1
done
grep -q 'Structure: reordered; selected Gamma' "$result_dir/after.txt"

kill "$app_pid" 2>/dev/null || true
wait "$app_pid" 2>/dev/null || true
app_pid=
"$gui" setup persistent >"$result_dir/setup-managed.txt" 2>&1
descriptor=$XDG_STATE_HOME/gui2tui/headless/session.json
managed_pid=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["supervisor_pid"])' "$descriptor")
"$gui" --session managed doctor --json >"$result_dir/doctor-managed.json"
python3 - "$result_dir/doctor-managed.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert "Managed headless" in checks["session-selection"]["message"]
assert checks["session-bus"]["level"] == "PASS"
assert checks["accessibility-registry"]["level"] == "PASS"
PY

if "$uninstall" --prefix "$prefix" >"$result_dir/uninstall-active.out" \
    2>"$result_dir/uninstall-active.err"; then
    echo 'Uninstall unexpectedly removed files while a managed descriptor existed' >&2
    exit 1
fi
grep -q 'setup stop' "$result_dir/uninstall-active.err"
test -x "$gui"
"$gui" setup stop >"$result_dir/stop-managed.txt" 2>&1
if kill -0 "$managed_pid" 2>/dev/null; then
    echo 'Managed supervisor remained after explicit stop' >&2
    exit 1
fi
managed_pid=

mkdir -p "$XDG_CONFIG_HOME/gui2tui"
echo 'version = 1' >"$XDG_CONFIG_HOME/gui2tui/config.toml"
chmod 600 "$XDG_CONFIG_HOME/gui2tui/config.toml"
mv "$prefix/libexec/gui2tui/gui2tui-local" "$root/gui2tui-local.saved"
ln -s "$prefix/unrelated.txt" "$prefix/libexec/gui2tui/gui2tui-local"
if "$uninstall" --prefix "$prefix" >"$result_dir/uninstall-symlink.out" \
    2>"$result_dir/uninstall-symlink.err"; then
    echo 'Uninstall unexpectedly followed or removed a managed-path symlink' >&2
    exit 1
fi
test -f "$prefix/unrelated.txt"
rm "$prefix/libexec/gui2tui/gui2tui-local"
mv "$root/gui2tui-local.saved" "$prefix/libexec/gui2tui/gui2tui-local"
"$uninstall" --prefix "$prefix" >"$result_dir/uninstall.txt"
test ! -e "$prefix/bin/gui2tui"
test ! -e "$prefix/libexec/gui2tui/gui2tui-inspect"
test ! -e "$prefix/libexec/gui2tui/gui2tui-local"
test ! -e "$prefix/libexec/gui2tui/headless-session"
test ! -e "$prefix/libexec/gui2tui/uninstall-user"
test ! -e "$prefix/libexec/gui2tui/install-manifest-v1"
test -f "$prefix/unrelated.txt"
test -f "$XDG_CONFIG_HOME/gui2tui/config.toml"

{
    echo 'fresh_user_prefix_install=PASS'
    echo 'path_with_spaces=PASS'
    echo 'arbitrary_working_directory=PASS'
    echo 'helper_discovery=PASS'
    echo 'desktop_doctor=PASS'
    echo 'managed_doctor=PASS'
    echo 'session_bus_failure=PASS'
    echo 'atspi_bus_failure=NOT_TESTED'
    echo 'zero_app_distinction=PASS'
    echo 'application_semantics_boundary=PASS'
    echo 'handler_not_configured=PASS'
    echo 'handler_executable=PASS'
    echo 'handler_unavailable=PASS'
    echo 'handler_not_executed=PASS'
    echo 'semantic_operation=PASS'
    echo 'active_managed_uninstall_refusal=PASS'
    echo 'symlink_uninstall_refusal=PASS'
    echo 'safe_uninstall=PASS'
    echo 'unrelated_file_preserved=PASS'
    echo 'user_config_preserved=PASS'
    echo 'real_local_x11=NOT_TESTED'
    echo "interactive_terminal=$terminal_evidence"
} >"$result_dir/summary.txt"
cat "$result_dir/summary.txt"
