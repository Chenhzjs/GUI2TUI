#!/usr/bin/env bash
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
result_dir=${RESULT_DIR:-}
if [[ -n $result_dir ]]; then
    [[ $result_dir == /* && ! -e $result_dir ]] || {
        echo 'RESULT_DIR must be an absolute path that does not exist' >&2
        exit 2
    }
    mkdir -m 700 -p -- "$result_dir"
else
    result_dir=$(mktemp -d /tmp/gui2tui-v07c-headless-evidence.XXXXXX)
fi

scratch=$(mktemp -d /tmp/gui2tui-v07c-headless.XXXXXX)
commit=$(git -C "$project_root" rev-parse HEAD)
short_commit=${commit:0:12}
image="gui2tui-v07c-headless:$short_commit"
container="gui2tui-v07c-headless-$short_commit-$$"
private_key=$scratch/id_ed25519
known_hosts=$scratch/known_hosts
container_started=false
image_created=false
bundle_dir=${GUI2TUI_QUALIFICATION_BUNDLE:-}
bundle_mount=()
if [[ -n $bundle_dir ]]; then
    [[ $bundle_dir == /* && -d $bundle_dir ]] || {
        echo 'GUI2TUI_QUALIFICATION_BUNDLE must be an absolute extracted bundle directory' >&2
        exit 2
    }
    for required in bin/gui2tui libexec/gui2tui/gui2tui-inspect \
        libexec/gui2tui/gui2tui-local libexec/gui2tui/headless-session \
        install-user.sh uninstall-user.sh BUILD-INFO.json ABI.json; do
        [[ -f $bundle_dir/$required ]] || {
            echo "qualification bundle is missing: $required" >&2
            exit 2
        }
    done
    bundle_mount=(--mount "type=bind,src=$bundle_dir,dst=/opt/gui2tui-bundle,readonly")
fi

container_exec() {
    docker exec --user gui2tui \
        --env HOME=/home/gui2tui \
        --env XDG_CONFIG_HOME=/home/gui2tui/.config \
        --env XDG_STATE_HOME=/home/gui2tui/.local/state \
        --env XDG_RUNTIME_DIR=/home/gui2tui/.local/run \
        --env LANG=C.UTF-8 \
        --env TERM=xterm-256color \
        "$container" "$@"
}

cleanup() {
    local status=$?
    if $container_started && docker inspect "$container" >/dev/null 2>&1; then
        container_exec /usr/local/libexec/gui2tui-v07c-container-control stop \
            >/dev/null 2>&1 || true
        docker rm -f "$container" >/dev/null 2>&1 || true
    fi
    if $image_created; then
        docker image rm "$image" >/dev/null 2>&1 || true
    fi
    rm -rf -- "$scratch"
    if ((status == 0)); then
        printf 'V07C_HEADLESS_EVIDENCE=%s\n' "$result_dir"
    fi
    exit "$status"
}
trap cleanup EXIT

command -v docker >/dev/null
command -v ssh >/dev/null
command -v ssh-keygen >/dev/null
command -v ssh-keyscan >/dev/null
docker info >/dev/null

ssh-keygen -q -t ed25519 -N '' -f "$private_key"
chmod 600 "$private_key"

docker build \
    --label org.gui2tui.validation=v07c-headless \
    --build-arg "GUI2TUI_SOURCE_COMMIT=$commit" \
    --file "$project_root/tests/live/Dockerfile.v07c-headless" \
    --tag "$image" \
    "$project_root" >"$result_dir/docker-build.log"
image_created=true

docker run -d \
    --name "$container" \
    --label org.gui2tui.validation=v07c-headless \
    --publish 127.0.0.1::22 \
    --mount "type=bind,src=$private_key.pub,dst=/run/gui2tui-test/authorized_key.pub,readonly" \
    "${bundle_mount[@]}" \
    "$image" >"$result_dir/container-id.txt"
container_started=true

port=$(docker port "$container" 22/tcp | awk -F: '$1 == "127.0.0.1" { print $NF }')
[[ $port =~ ^[0-9]+$ ]]
[[ $(docker inspect --format '{{.HostConfig.Privileged}}' "$container") == false ]]
[[ -z $(docker inspect --format '{{.HostConfig.PidMode}}' "$container") ]]
[[ $(docker inspect --format '{{(index (index .NetworkSettings.Ports "22/tcp") 0).HostIp}}' "$container") == 127.0.0.1 ]]
mounts=$(docker inspect --format '{{range .Mounts}}{{println .Destination .RW}}{{end}}' "$container")
grep -qx '/run/gui2tui-test/authorized_key.pub false' <<<"$mounts"
if [[ -n $bundle_dir ]]; then
    grep -qx '/opt/gui2tui-bundle false' <<<"$mounts"
    [[ $(grep -c . <<<"$mounts") == 2 ]]
else
    [[ $(grep -c . <<<"$mounts") == 1 ]]
fi

for _ in {1..100}; do
    if ssh-keyscan -p "$port" 127.0.0.1 >"$known_hosts" 2>/dev/null; then
        break
    fi
    sleep 0.1
done
[[ -s $known_hosts ]]
chmod 600 "$known_hosts"

ssh_options=(
    -F /dev/null
    -o IdentityAgent=none
    -o BatchMode=yes
    -o IdentitiesOnly=yes
    -o PasswordAuthentication=no
    -o StrictHostKeyChecking=yes
    -o "UserKnownHostsFile=$known_hosts"
    -i "$private_key"
    -p "$port"
)
ssh "${ssh_options[@]}" gui2tui@127.0.0.1 \
    'test "$(id -u)" != 0 && test "$(uname -m)" = aarch64' \
    >"$result_dir/ssh-auth.txt"

container_exec /usr/local/libexec/gui2tui-v07c-container-control install \
    >"$result_dir/install.txt"
container_exec /usr/local/libexec/gui2tui-v07c-container-control setup \
    >"$result_dir/setup.txt"
container_exec /usr/local/libexec/gui2tui-v07c-container-control configure-vim
container_exec /usr/local/libexec/gui2tui-v07c-container-control start-fixture selection
container_exec /usr/local/libexec/gui2tui-v07c-container-control start-fixture live

python3 "$project_root/tests/live/v07c_headless_driver.py" \
    --container "$container" \
    --port "$port" \
    --identity "$private_key" \
    --known-hosts "$known_hosts" \
    --evidence "$result_dir/results.json"

container_exec /usr/local/libexec/gui2tui-v07c-container-control stop \
    >"$result_dir/stop.txt"
container_exec test ! -e /home/gui2tui/.local/state/gui2tui/headless/session.json

container_exec /home/gui2tui/.local/libexec/gui2tui/uninstall-user \
    --prefix /home/gui2tui/.local >"$result_dir/uninstall.txt"
container_exec test ! -e /home/gui2tui/.local/bin/gui2tui
container_exec test -f /home/gui2tui/.config/gui2tui/config.toml

python3 - "$result_dir/results.json" "$result_dir/summary.txt" <<'PY'
import json
import pathlib
import sys

results = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
required = {
    "installed_binary",
    "fresh_registry_enumeration",
    "docker_doctor",
    "docker_exec_semantic_operation",
    "docker_exec_authoritative_readback",
    "docker_exec_terminal_restore",
    "ssh_doctor",
    "ssh_semantic_operation",
    "ssh_authoritative_readback",
    "ssh_terminal_restore",
    "ssh_external_vim",
    "ssh_external_writeback_readback",
    "ssh_tui_disconnect",
    "ssh_handler_disconnect",
    "ssh_handler_disconnect_artifacts",
    "ssh_reconnect_authority",
    "container_restart_old_session_rejected",
    "container_restart_old_authority_not_restored",
    "container_restart_fresh_selection",
}
missing = required.difference(results)
if missing:
    raise SystemExit(f"missing qualification results: {sorted(missing)}")
pathlib.Path(sys.argv[2]).write_text(
    "DOCKER_HEADLESS_INTERACTIVE_TTY=QUALIFIED\n"
    "SAME_HOST_SSH_MANAGED=QUALIFIED\n"
    "SSH_EXTERNAL_HANDLER=QUALIFIED\n"
    "SSH_DISCONNECT_BOUNDARY=QUALIFIED_WITH_EXPLICIT_LIMITATIONS\n"
    "LINUX_NATIVE_VT=NOT_TESTED\n"
    "ORDINARY_LOCAL_X11=NOT_TESTED_OPTIONAL_COMPATIBILITY\n",
    encoding="utf-8",
)
PY
