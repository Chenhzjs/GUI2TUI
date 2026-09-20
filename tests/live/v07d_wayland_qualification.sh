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
    result_dir=$(mktemp -d /tmp/gui2tui-v07d-wayland-evidence.XXXXXX)
fi

commit=$(git -C "$project_root" rev-parse HEAD)
short_commit=${commit:0:12}
image=${GUI2TUI_V07D_IMAGE:-gui2tui-v07d-wayland:$short_commit}
container="gui2tui-v07d-wayland-$short_commit-$$"
container_started=false
image_created=false
control=/usr/local/libexec/gui2tui-v07d-container-control

container_exec() {
    docker exec --user gui2tui "$container" "$@"
}

cleanup() {
    local status=$?
    if $container_started && docker inspect "$container" >/dev/null 2>&1; then
        container_exec "$control" stop-session >/dev/null 2>&1 || true
        docker rm -f "$container" >/dev/null 2>&1 || true
    fi
    if $image_created; then
        docker image rm "$image" >/dev/null 2>&1 || true
    fi
    if ((status == 0)); then
        printf 'V07D_WAYLAND_EVIDENCE=%s\n' "$result_dir"
    fi
    exit "$status"
}
trap cleanup EXIT

command -v docker >/dev/null
docker info >/dev/null

if [[ -z ${GUI2TUI_V07D_IMAGE:-} ]]; then
    docker build \
        --label org.gui2tui.validation=v07d-wayland \
        --build-arg "GUI2TUI_SOURCE_COMMIT=$commit" \
        --file "$project_root/tests/live/Dockerfile.v07d-wayland" \
        --tag "$image" \
        "$project_root" >"$result_dir/docker-build.log"
    image_created=true
else
    [[ $(docker image inspect "$image" --format '{{index .Config.Labels "org.gui2tui.validation"}}') == v07d-wayland ]]
fi

docker run -d \
    --init \
    --name "$container" \
    --label org.gui2tui.validation=v07d-wayland \
    "$image" >"$result_dir/container-id.txt"
container_started=true

[[ $(docker inspect --format '{{.HostConfig.Privileged}}' "$container") == false ]]
[[ -z $(docker inspect --format '{{.HostConfig.PidMode}}' "$container") ]]
[[ -z $(docker inspect --format '{{range .Mounts}}{{.Destination}}{{end}}' "$container") ]]
[[ -z $(docker inspect --format '{{range $key, $value := .NetworkSettings.Ports}}{{$key}}{{end}}' "$container") ]]
[[ $(container_exec id -u) != 0 ]]

container_exec "$control" install >"$result_dir/install.txt"
container_exec "$control" configure-vim
container_exec "$control" start-session >"$result_dir/session-start.txt"

container_exec bash -c '
    printf "os="; . /etc/os-release; printf "%s %s\n" "$NAME" "$VERSION_ID"
    printf "arch="; uname -m
    printf "glibc="; getconf GNU_LIBC_VERSION
    printf "weston="; weston --version
    printf "xwayland="; Xwayland -version 2>&1 | sed -n "1p"
    printf "atspi="; dpkg-query -W -f="\${Version}\n" at-spi2-core
    printf "gtk="; dpkg-query -W -f="\${Version}\n" gir1.2-gtk-4.0
    printf "qt="; dpkg-query -W -f="\${Version}\n" python3-pyqt6
' >"$result_dir/environment.txt"

python3 "$project_root/tests/live/v07d_wayland_driver.py" \
    --container "$container" \
    --evidence "$result_dir/results.json"

container_exec "$control" stop-session >"$result_dir/session-stop.txt"
if container_exec ps -u "$(container_exec id -u)" -o comm= \
    | grep -Eq '^(weston|weston-keyboard|weston-desktop-|Xwayland|at-spi-bus-laun|at-spi2-registr|dbus-daemon|python3)$'; then
    echo 'owned Wayland qualification process remained after bounded stop' >&2
    exit 1
fi

docker cp "$container:/home/gui2tui/evidence/wayland-info.txt" \
    "$result_dir/wayland-info.txt"
docker cp "$container:/home/gui2tui/evidence/weston.log" \
    "$result_dir/weston.log"

python3 - "$result_dir/results.json" "$result_dir/summary.txt" <<'PY'
import json
import pathlib
import sys

results = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
required = {
    "installed_binary",
    "headless_compositor",
    "session_topology",
    "doctor",
    "native_registry_enumeration",
    "native_application_identity",
    "native_gtk_geometry",
    "native_gtk_layout",
    "native-wayland_tui",
    "native-wayland_semantic_operation",
    "native-wayland_authoritative_readback",
    "native-wayland_dynamic_refresh",
    "native-wayland_terminal_restore",
    "native_qt_identity",
    "native_qt_semantics",
    "native_qt_authoritative_readback",
    "native_modal_scope",
    "native_modal_app_restart_old_locator",
    "native_external_vim",
    "native_external_writeback_readback",
    "xwayland_identity",
    "xwayland_gtk_geometry",
    "xwayland_gtk_layout",
    "xwayland_tui",
    "xwayland_semantic_operation",
    "xwayland_authoritative_readback",
    "xwayland_dynamic_refresh",
    "xwayland_terminal_restore",
    "native_app_restart_old_locator",
    "xwayland_app_restart_old_locator",
    "compositor_session_restart_old_locator",
    "lifecycle_recovery",
    "old_generation_migration",
    "wayland_static_capture",
}
missing = required.difference(results)
if missing:
    raise SystemExit(f"missing qualification results: {sorted(missing)}")
pathlib.Path(sys.argv[2]).write_text(
    "NATIVE_WAYLAND_HEADLESS=QUALIFIED_WITH_EXPLICIT_LIMITATIONS\n"
    "XWAYLAND_HEADLESS=QUALIFIED_WITH_EXPLICIT_LIMITATIONS\n"
    "WAYLAND_STATIC_CAPTURE=UNSUPPORTED_DEFERRED\n"
    "WAYLAND_OVER_REAL_SSH_PTY=NOT_TESTED\n"
    "NATIVE_LINUX_VT=NOT_TESTED\n"
    "ORDINARY_FULL_DESKTOP_WAYLAND=NOT_TESTED\n",
    encoding="utf-8",
)
PY
