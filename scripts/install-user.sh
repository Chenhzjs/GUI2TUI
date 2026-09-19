#!/usr/bin/env bash
# Install one built/extracted GUI2TUI tree into an unprivileged user prefix.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: install-user.sh [--prefix ABSOLUTE_PATH]

Default prefix: $HOME/.local

Run from an extracted GUI2TUI archive, or from the source checkout after:
  cargo build --release --locked --bins

The installer never uses sudo, never writes system directories, and refuses
to overwrite any existing target. Uninstall before reinstalling.
EOF
}

prefix=
while (($#)); do
    case "$1" in
        --prefix)
            (($# >= 2)) || { echo 'error: --prefix requires a path' >&2; exit 2; }
            prefix=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

[[ $(uname -s) == Linux ]] || { echo 'error: user-prefix installation requires Linux' >&2; exit 1; }
((EUID != 0)) || { echo 'error: do not run the GUI2TUI user installer as root or with sudo' >&2; exit 1; }
if [[ -z $prefix ]]; then
    [[ ${HOME:-} == /* ]] || { echo 'error: set an absolute HOME or pass --prefix' >&2; exit 1; }
    prefix=$HOME/.local
fi
[[ $prefix == /* ]] || { echo 'error: --prefix must be absolute' >&2; exit 2; }
command -v sha256sum >/dev/null 2>&1 || { echo 'error: sha256sum is required for the uninstall manifest' >&2; exit 1; }

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
if [[ -x $script_dir/bin/gui2tui ]]; then
    sources=(
        "$script_dir/bin/gui2tui"
        "$script_dir/libexec/gui2tui/gui2tui-inspect"
        "$script_dir/libexec/gui2tui/gui2tui-local"
        "$script_dir/libexec/gui2tui/headless-session"
        "$script_dir/uninstall-user.sh"
    )
elif [[ -x $script_dir/../target/release/gui2tui ]]; then
    project=$(cd -- "$script_dir/.." && pwd -P)
    sources=(
        "$project/target/release/gui2tui"
        "$project/target/release/gui2tui-inspect"
        "$project/target/release/gui2tui-local"
        "$project/scripts/headless-session"
        "$project/scripts/uninstall-user.sh"
    )
else
    echo 'error: no complete extracted bundle or target/release build found beside this installer' >&2
    echo 'Build first with: cargo build --release --locked --bins' >&2
    exit 1
fi

destinations=(
    bin/gui2tui
    libexec/gui2tui/gui2tui-inspect
    libexec/gui2tui/gui2tui-local
    libexec/gui2tui/headless-session
    libexec/gui2tui/uninstall-user
)

for source in "${sources[@]}"; do
    [[ -f $source && -x $source ]] || { echo 'error: installation payload is incomplete or not executable' >&2; exit 1; }
done

for directory in "$prefix" "$prefix/bin" "$prefix/libexec" "$prefix/libexec/gui2tui"; do
    [[ ! -L $directory ]] || { echo "error: refusing symlink installation directory: $directory" >&2; exit 1; }
    mkdir -p -- "$directory"
    [[ $(stat -c %u -- "$directory") == "$(id -u)" ]] || {
        echo "error: installation directory is not owned by the current user: $directory" >&2
        exit 1
    }
done

manifest=$prefix/libexec/gui2tui/install-manifest-v1
for relative in "${destinations[@]}"; do
    target=$prefix/$relative
    if [[ -e $target || -L $target ]]; then
        echo "error: refusing to overwrite existing path: $target" >&2
        echo 'Uninstall the prior GUI2TUI user-prefix installation first, or choose another prefix.' >&2
        exit 1
    fi
done
if [[ -e $manifest || -L $manifest ]]; then
    echo "error: refusing to overwrite existing installation manifest: $manifest" >&2
    exit 1
fi

created=()
cleanup_partial() {
    local status=$?
    if ((status != 0)); then
        for relative in "${created[@]}"; do
            rm -f -- "$prefix/$relative"
        done
        rm -f -- "${manifest}.tmp"
    fi
    exit "$status"
}
trap cleanup_partial EXIT

for index in "${!sources[@]}"; do
    install -m 755 -- "${sources[$index]}" "$prefix/${destinations[$index]}"
    created+=("${destinations[$index]}")
done

umask 077
{
    echo 'GUI2TUI_USER_INSTALL_V1'
    "$prefix/bin/gui2tui" --version
    for relative in "${destinations[@]}"; do
        hash=$(sha256sum -- "$prefix/$relative")
        printf '%s  %s\n' "${hash%% *}" "$relative"
    done
} >"${manifest}.tmp"
chmod 600 "${manifest}.tmp"
mv -- "${manifest}.tmp" "$manifest"
created+=(libexec/gui2tui/install-manifest-v1)

trap - EXIT
printf 'Installed GUI2TUI into %s\n' "$prefix"
printf 'Add %s/bin to PATH, then run:\n' "$prefix"
printf '  gui2tui --session desktop doctor\n'
printf 'Safe uninstall:\n'
printf '  %s/libexec/gui2tui/uninstall-user --prefix %q\n' "$prefix" "$prefix"
