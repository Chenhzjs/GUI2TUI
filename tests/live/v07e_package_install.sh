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
cleanup() {
    local status=$?
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
assert checks["complex-text-handler"]["level"] == "WARN"
PY

rm -f -- "$XDG_CONFIG_HOME/gui2tui/config.toml"
"$gui" setup persistent >"$result_dir/setup-persistent.txt"
"$gui" --session managed doctor --json >"$result_dir/doctor-managed.json"
if "$uninstall" >"$result_dir/uninstall-active-managed.txt" 2>&1; then
    echo 'Uninstall unexpectedly accepted an active Managed descriptor' >&2
    exit 1
fi
"$gui" setup stop >"$result_dir/setup-stop.txt"

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
default_user_prefix=PASS
space_containing_prefix=PASS
arbitrary_working_directory=PASS
required_helper_diagnostic=PASS
optional_helper_degradation=PASS
external_handler_diagnostic=PASS
managed_setup_doctor_stop=PASS
active_managed_uninstall_refusal=PASS
installed_semantic_smoke=PASS
safe_uninstall_preservation=PASS
EOF
cat "$result_dir/summary.txt"
