#!/usr/bin/env bash
# Fresh-install qualification for one already-built archive. Runs as non-root.
set -euo pipefail

archive=${1:?usage: v07e_package_install.sh ARCHIVE RESULT_DIR}
result_dir=${2:?usage: v07e_package_install.sh ARCHIVE RESULT_DIR}
[[ $(uname -s) == Linux && $EUID != 0 ]]
[[ $archive == /* && -f $archive ]]
[[ $result_dir == /* ]]
mkdir -m 700 -p -- "$result_dir"

work=$(mktemp -d /tmp/gui2tui-v07e-install.XXXXXX)
isolated_bus_pid=
cleanup() {
    local status=$?
    if [[ -n $isolated_bus_pid ]]; then
        kill "$isolated_bus_pid" 2>/dev/null || true
        wait "$isolated_bus_pid" 2>/dev/null || true
    fi
    if [[ -x ${default_prefix:-}/bin/gui2tui ]]; then
        "${default_prefix}/bin/gui2tui" setup stop >/dev/null 2>&1 || true
    fi
    rm -rf -- "$work"
    exit "$status"
}
trap cleanup EXIT

tar -xzf "$archive" -C "$work"
bundle=$(find "$work" -mindepth 1 -maxdepth 1 -type d -name 'gui2tui-*-linux-*' -print -quit)
[[ -n $bundle ]]
cp -- "$bundle/BUILD-INFO.json" "$result_dir/build-info.json"
package_commit=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["commit"])' \
    "$bundle/BUILD-INFO.json")

export HOME=$work/home
export XDG_CONFIG_HOME=$HOME/.config
export XDG_STATE_HOME=$HOME/.local/state
export XDG_RUNTIME_DIR=$HOME/.local/run
export LANG=C.UTF-8
export TERM=xterm-256color
mkdir -m 700 -p -- "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR"

default_prefix=$HOME/.local
mkdir -p -- "$default_prefix/share"
printf 'unrelated\n' >"$default_prefix/share/unrelated.txt"

"$bundle/install-user.sh" >"$result_dir/install-default.txt"
gui=$default_prefix/bin/gui2tui
inspector=$default_prefix/libexec/gui2tui/gui2tui-inspect
local_helper=$default_prefix/libexec/gui2tui/gui2tui-local
uninstall=$default_prefix/libexec/gui2tui/uninstall-user
manifest=$default_prefix/libexec/gui2tui/install-manifest-v1
[[ -x $gui && -x $inspector && -x $local_helper && -x $uninstall ]]
[[ $(stat -c %a "$manifest") == 600 ]]

mkdir -m 700 -- "$work/elsewhere"
(cd "$work/elsewhere" && "$gui" --version) >"$result_dir/version.txt"

if env -u DBUS_SESSION_BUS_ADDRESS "$gui" --session desktop doctor --json \
    >"$result_dir/doctor-no-session-bus.json"; then
    echo 'Doctor unexpectedly accepted a missing desktop session bus' >&2
    exit 1
fi

# A private Session D-Bus with no activation directories reliably represents
# "session bus works, AT-SPI is absent" without touching a real user session.
isolated_bus_socket=$work/isolated-session-bus
cat >"$work/isolated-dbus.conf" <<EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=$isolated_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF
dbus-daemon --nofork --config-file="$work/isolated-dbus.conf" \
    >"$result_dir/isolated-session-bus.log" 2>&1 &
isolated_bus_pid=$!
for _ in {1..100}; do
    if DBUS_SESSION_BUS_ADDRESS="unix:path=$isolated_bus_socket" \
        gdbus call --session --dest org.freedesktop.DBus \
        --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.ListNames >/dev/null 2>&1; then
        break
    fi
    sleep 0.02
done
if DBUS_SESSION_BUS_ADDRESS="unix:path=$isolated_bus_socket" \
    "$gui" --session desktop doctor --json >"$result_dir/doctor-no-atspi.json"; then
    echo 'Doctor unexpectedly accepted a session without AT-SPI' >&2
    exit 1
fi
python3 - "$result_dir/doctor-no-atspi.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["session-bus"]["level"] == "PASS"
assert checks["accessibility-bus"]["level"] == "FAIL"
assert checks["accessibility-registry"]["level"] == "INFO"
assert checks["accessible-applications"]["level"] == "INFO"
PY
kill "$isolated_bus_pid"
wait "$isolated_bus_pid" || true
isolated_bus_pid=

chmod 0644 "$local_helper"
env -u DBUS_SESSION_BUS_ADDRESS "$gui" --session desktop doctor --json \
    >"$result_dir/doctor-optional-helper.json" || true
python3 - "$result_dir/doctor-optional-helper.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["helper-same-host-modality"]["level"] == "WARN"
PY
chmod 0755 "$local_helper"

chmod 0644 "$inspector"
env -u DBUS_SESSION_BUS_ADDRESS "$gui" --session desktop doctor --json \
    >"$result_dir/doctor-required-helper.json" || true
python3 - "$result_dir/doctor-required-helper.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["helper-inspector"]["level"] == "FAIL"
PY
chmod 0755 "$inspector"

mkdir -m 700 -p -- "$XDG_CONFIG_HOME/gui2tui"
cat >"$XDG_CONFIG_HOME/gui2tui/config.toml" <<'EOF'
version = 1
[interaction.complex_text]
program = "/definitely/missing/gui2tui-editor"
args = ["{file}"]
EOF
chmod 0600 "$XDG_CONFIG_HOME/gui2tui/config.toml"
env -u DBUS_SESSION_BUS_ADDRESS "$gui" --session desktop doctor --json \
    >"$result_dir/doctor-handler-unavailable.json" || true
python3 - "$result_dir/doctor-handler-unavailable.json" <<'PY'
import json, sys
checks = {item["name"]: item for item in json.load(open(sys.argv[1]))["checks"]}
assert checks["external-text-handler"]["level"] == "WARN"
PY

rm -f -- "$XDG_CONFIG_HOME/gui2tui/config.toml"
"$gui" setup persistent >"$result_dir/setup-persistent.txt"
"$gui" --session managed doctor --json >"$result_dir/doctor-managed.json"
descriptor=$XDG_STATE_HOME/gui2tui/headless/session.json
cp -- "$descriptor" "$work/valid-session.json"

export GUI2TUI_TEST_GUI=$gui
script -qec 'stty rows 40 cols 120; exec "$GUI2TUI_TEST_GUI" --session managed doctor --json' \
    /dev/null >"$result_dir/doctor-terminal-healthy.json"
python3 - "$result_dir/doctor-terminal-healthy.json" <<'PY'
import json, sys
text = open(sys.argv[1]).read().replace("\r", "")
report, _ = json.JSONDecoder().raw_decode(text[text.index("{"):])
checks = {item["name"]: item for item in report["checks"]}
assert checks["terminal-interactive"]["level"] == "PASS"
assert checks["terminal-type"]["level"] == "PASS"
assert checks["terminal-utf8"]["level"] == "PASS"
assert checks["terminal-size"]["level"] == "PASS"
assert checks["terminal-modes"]["level"] == "INFO"
PY

if script -qec 'stty rows 40 cols 120; exec env TERM=dumb LANG=C LC_ALL= LC_CTYPE= "$GUI2TUI_TEST_GUI" --session managed doctor --json' \
    /dev/null >"$result_dir/doctor-terminal-invalid-env.json"; then
    echo 'Doctor unexpectedly accepted an unusable interactive terminal environment' >&2
    exit 1
fi
python3 - "$result_dir/doctor-terminal-invalid-env.json" <<'PY'
import json, sys
text = open(sys.argv[1]).read().replace("\r", "")
report, _ = json.JSONDecoder().raw_decode(text[text.index("{"):])
checks = {item["name"]: item for item in report["checks"]}
assert checks["terminal-interactive"]["level"] == "PASS"
assert checks["terminal-type"]["level"] == "FAIL"
assert checks["terminal-utf8"]["level"] == "FAIL"
PY

if script -qec 'stty rows 0 cols 0; exec "$GUI2TUI_TEST_GUI" --session managed doctor --json' \
    /dev/null >"$result_dir/doctor-terminal-zero-size.json"; then
    echo 'Doctor unexpectedly accepted a zero-size interactive terminal' >&2
    exit 1
fi
python3 - "$result_dir/doctor-terminal-zero-size.json" <<'PY'
import json, sys
text = open(sys.argv[1]).read().replace("\r", "")
report, _ = json.JSONDecoder().raw_decode(text[text.index("{"):])
checks = {item["name"]: item for item in report["checks"]}
assert checks["terminal-size"]["level"] == "FAIL"
PY

if "$uninstall" >"$result_dir/uninstall-active-managed.txt" 2>&1; then
    echo 'Uninstall unexpectedly accepted an active Managed descriptor' >&2
    exit 1
fi
"$gui" setup stop >"$result_dir/setup-stop.txt"

if "$gui" --session managed doctor --json >"$result_dir/doctor-managed-missing.out" \
    2>"$result_dir/doctor-managed-missing.err"; then
    echo 'Doctor unexpectedly accepted a missing Managed descriptor' >&2
    exit 1
fi
grep -q 'descriptor does not exist' "$result_dir/doctor-managed-missing.err"

cp -- "$work/valid-session.json" "$descriptor"
chmod 0600 "$descriptor"
if "$gui" --session managed doctor --json >"$result_dir/doctor-managed-stopped.out" \
    2>"$result_dir/doctor-managed-stopped.err"; then
    echo 'Doctor unexpectedly accepted a stopped Managed supervisor' >&2
    exit 1
fi
grep -q 'supervisor is not running' "$result_dir/doctor-managed-stopped.err"

printf '{invalid\n' >"$descriptor"
chmod 0600 "$descriptor"
if "$gui" --session managed doctor --json >"$result_dir/doctor-managed-invalid.out" \
    2>"$result_dir/doctor-managed-invalid.err"; then
    echo 'Doctor unexpectedly accepted an invalid Managed descriptor' >&2
    exit 1
fi
grep -q 'descriptor is invalid' "$result_dir/doctor-managed-invalid.err"

cp -- "$work/valid-session.json" "$descriptor"
chmod 0644 "$descriptor"
if "$gui" --session managed doctor --json >"$result_dir/doctor-managed-unsafe.out" \
    2>"$result_dir/doctor-managed-unsafe.err"; then
    echo 'Doctor unexpectedly accepted unsafe Managed descriptor permissions' >&2
    exit 1
fi
grep -q 'current-user owned, private' "$result_dir/doctor-managed-unsafe.err"

python3 - "$descriptor" "$$" <<'PY'
import json, os, sys
path, pid = sys.argv[1:]
with open(path, "w") as handle:
    json.dump({"schema_version": 1, "supervisor_pid": int(pid), "display": ":99",
               "session_bus_address": "unix:path=/nonexistent/gui2tui-v07e-bus"}, handle)
    handle.write("\n")
os.chmod(path, 0o600)
PY
if "$gui" --session managed doctor --json >"$result_dir/doctor-managed-unreachable.json"; then
    echo 'Doctor unexpectedly accepted an unreachable Managed session bus' >&2
    exit 1
fi
python3 - "$result_dir/doctor-managed-unreachable.json" <<'PY'
import json, sys
text = open(sys.argv[1]).read()
assert "/nonexistent/gui2tui-v07e-bus" not in text
checks = {item["name"]: item for item in json.loads(text)["checks"]}
assert checks["session-selection"]["level"] == "INFO"
assert checks["session-bus"]["level"] == "FAIL"
assert checks["accessibility-bus"]["level"] == "INFO"
PY
rm -f -- "$descriptor"

GUI2TUI_RUNTIME_PREFIX=$default_prefix \
    "$bundle/smoke/run.sh" >"$result_dir/installed-smoke.txt"
grep -q 'PACKAGED_FRESH_HOME_SMOKE=PASS' "$result_dir/installed-smoke.txt"

mkdir -m 700 -p -- "$XDG_CONFIG_HOME/gui2tui"
printf 'version = 1\n' >"$XDG_CONFIG_HOME/gui2tui/config.toml"
chmod 0600 "$XDG_CONFIG_HOME/gui2tui/config.toml"
"$uninstall" >"$result_dir/uninstall-default.txt"
[[ ! -e $gui && ! -e $manifest ]]
[[ -f $default_prefix/share/unrelated.txt ]]
[[ -f $XDG_CONFIG_HOME/gui2tui/config.toml ]]

space_prefix=$HOME/'prefix with spaces'
"$bundle/install-user.sh" --prefix "$space_prefix" >"$result_dir/install-space-prefix.txt"
(cd /tmp && "$space_prefix/bin/gui2tui" --version) >"$result_dir/space-prefix-version.txt"
"$space_prefix/libexec/gui2tui/uninstall-user" --prefix "$space_prefix" \
    >"$result_dir/uninstall-space-prefix.txt"
[[ ! -e $space_prefix/bin/gui2tui ]]

cat >"$result_dir/summary.txt" <<EOF
architecture=$(uname -m)
package_commit=$package_commit
default_user_prefix=PASS
space_containing_prefix=PASS
arbitrary_working_directory=PASS
required_helper_diagnostic=PASS
optional_helper_degradation=PASS
external_handler_diagnostic=PASS
managed_setup_doctor_stop=PASS
managed_descriptor_missing=PASS
managed_descriptor_stopped=PASS
managed_descriptor_invalid=PASS
managed_descriptor_unsafe=PASS
managed_session_bus_unreachable=PASS
session_bus_reachable_atspi_absent=PASS
registry_zero_applications=PASS
application_semantics_boundary=PASS_NOT_SCANNED_BY_DOCTOR
interactive_terminal=PASS
terminal_unusable_term=PASS_REJECTED
terminal_non_utf8=PASS_REJECTED
terminal_zero_size=PASS_REJECTED
active_managed_uninstall_refusal=PASS
installed_semantic_smoke=PASS
safe_uninstall_preservation=PASS
EOF
cat "$result_dir/summary.txt"
