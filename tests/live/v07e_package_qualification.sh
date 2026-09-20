#!/usr/bin/env bash
# Build and qualify current-source internal packages for both GNU/Linux architectures.
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
    result_dir=$(mktemp -d /tmp/gui2tui-v07e-package-evidence.XXXXXX)
fi

scratch=$(mktemp -d /tmp/gui2tui-v07e-package.XXXXXX)
commit=$(git -C "$project_root" rev-parse HEAD)
short_commit=${commit:0:12}
uid=$(id -u)
gid=$(id -g)
images=()

cleanup() {
    local status=$?
    for image in "${images[@]}"; do
        docker image rm "$image" >/dev/null 2>&1 || true
    done
    rm -rf -- "$scratch"
    if ((status == 0)); then
        printf 'V07E_PACKAGE_EVIDENCE=%s\n' "$result_dir"
    fi
    exit "$status"
}
trap cleanup EXIT

command -v docker >/dev/null
docker info >/dev/null
mkdir -m 700 -- "$result_dir/packages"

for specification in amd64:x86_64 arm64:aarch64; do
    platform=${specification%%:*}
    architecture=${specification##*:}
    image="gui2tui-v07e-package-$platform:$short_commit"
    images+=("$image")
    architecture_dir=$result_dir/$architecture
    mkdir -m 700 -- "$architecture_dir"

    docker build \
        --platform "linux/$platform" \
        --label org.gui2tui.validation=v07e-package \
        --file "$project_root/tests/live/Dockerfile.v07e-package" \
        --tag "$image" \
        "$project_root" >"$architecture_dir/docker-build.log"

    docker run --rm \
        --platform "linux/$platform" \
        --user "$uid:$gid" \
        --env HOME=/tmp/gui2tui-build-home \
        --env CARGO_HOME=/tmp/gui2tui-cargo-home \
        --env CARGO_TARGET_DIR=/tmp/gui2tui-target \
        --env RELEASE_DIR=/out \
        --env "RELEASE_COMMIT=$commit" \
        --env RELEASE_RUNNER_BASELINE=ubuntu-22.04-container \
        --env RELEASE_MAX_GLIBC=2.35 \
        --env CARGO_INCREMENTAL=0 \
        --mount "type=bind,src=$project_root,dst=/src,readonly" \
        --mount "type=bind,src=$result_dir/packages,dst=/out" \
        --workdir /src \
        "$image" \
        bash -c 'mkdir -p "$HOME" "$CARGO_HOME" "$CARGO_TARGET_DIR"; ./scripts/package-linux.sh' \
        >"$architecture_dir/package.txt"

    archive=$result_dir/packages/gui2tui-0.3.0-linux-$architecture.tar.gz
    [[ -f $archive ]]
    docker run --rm \
        --platform "linux/$platform" \
        --user "$uid:$gid" \
        --env HOME=/tmp/gui2tui-validation-home \
        --mount "type=bind,src=$project_root,dst=/src,readonly" \
        --mount "type=bind,src=$result_dir/packages,dst=/packages,readonly" \
        --workdir /src \
        "$image" \
        ./scripts/validate-release.sh "/packages/$(basename "$archive")" --smoke \
        >"$architecture_dir/release-validation.txt"

    docker run --rm \
        --platform "linux/$platform" \
        --user "$uid:$gid" \
        --env HOME=/tmp/gui2tui-qualification-home \
        --mount "type=bind,src=$project_root,dst=/src,readonly" \
        --mount "type=bind,src=$result_dir/packages,dst=/packages,readonly" \
        --mount "type=bind,src=$architecture_dir,dst=/evidence" \
        --workdir /src \
        "$image" \
        ./tests/live/v07e_package_install.sh "/packages/$(basename "$archive")" /evidence/install \
        >"$architecture_dir/fresh-install.txt"

    cat "$architecture_dir/release-validation.txt" "$architecture_dir/fresh-install.txt" \
        >"$result_dir/packages/gui2tui-0.3.0-linux-$architecture.smoke.txt"
    grep -q 'PACKAGED_FRESH_HOME_SMOKE=PASS' \
        "$result_dir/packages/gui2tui-0.3.0-linux-$architecture.smoke.txt"
    (cd "$result_dir/packages" && sha256sum -c "$(basename "$archive").sha256") \
        >"$architecture_dir/checksum.txt"
done

python3 "$project_root/scripts/assemble-release.py" "$result_dir/packages" \
    --version 0.3.0 --commit "$commit" >"$result_dir/assembly.txt"
(cd "$result_dir/packages" && sha256sum -c SHA256SUMS) >"$result_dir/assembled-checksums.txt"

arm_archive=$result_dir/packages/gui2tui-0.3.0-linux-aarch64.tar.gz
mkdir -m 700 -- "$scratch/arm-bundle"
tar -xzf "$arm_archive" -C "$scratch/arm-bundle"
arm_bundle=$(find "$scratch/arm-bundle" -mindepth 1 -maxdepth 1 -type d -print -quit)
[[ -n $arm_bundle ]]
python3 - "$arm_bundle/BUILD-INFO.json" "$commit" <<'PY'
import json, sys
build = json.load(open(sys.argv[1]))
assert build["commit"] == sys.argv[2]
assert build["architecture"] == "aarch64"
PY

GUI2TUI_QUALIFICATION_BUNDLE=$arm_bundle \
RESULT_DIR=$result_dir/headless-ssh \
    "$project_root/tests/live/v07c_headless_qualification.sh" \
    >"$result_dir/headless-ssh.log"

GUI2TUI_QUALIFICATION_BUNDLE=$arm_bundle \
RESULT_DIR=$result_dir/wayland-xwayland \
    "$project_root/tests/live/v07d_wayland_qualification.sh" \
    >"$result_dir/wayland-xwayland.log"

cat >"$result_dir/summary.txt" <<EOF
source_commit=$commit
x86_64_package=QUALIFIED_WITH_EMULATED_RUNTIME_LIMITATION
aarch64_package=QUALIFIED
glibc_gate=2.35
managed_xvfb_package_integration=QUALIFIED_AARCH64
docker_interactive_tty_package_integration=QUALIFIED_AARCH64
same_host_ssh_package_integration=QUALIFIED_AARCH64
native_wayland_package_integration=QUALIFIED_WITH_EXPLICIT_LIMITATIONS_AARCH64
xwayland_package_integration=QUALIFIED_WITH_EXPLICIT_LIMITATIONS_AARCH64
linux_native_vt=NOT_TESTED
ordinary_desktop=NOT_TESTED_OPTIONAL_COMPATIBILITY
wayland_over_ssh=NOT_TESTED
EOF
cat "$result_dir/summary.txt"
