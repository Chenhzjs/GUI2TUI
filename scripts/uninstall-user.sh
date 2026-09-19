#!/usr/bin/env bash
# Remove only files recorded by one GUI2TUI user-prefix installation.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: uninstall-user [--prefix ABSOLUTE_PATH]

Without --prefix, infer the prefix from this installed script. Configuration,
Managed Headless state, runtime artifacts and unrelated prefix files are kept.
EOF
}

((EUID != 0)) || { echo 'error: do not run the GUI2TUI user uninstaller as root or with sudo' >&2; exit 1; }
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
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

if [[ -z $prefix ]]; then
    prefix=$(cd -- "$script_dir/../../.." && pwd -P)
fi
[[ $prefix == /* ]] || { echo 'error: --prefix must be absolute' >&2; exit 2; }
for directory in "$prefix" "$prefix/bin" "$prefix/libexec" "$prefix/libexec/gui2tui"; do
    [[ ! -L $directory ]] || { echo "error: refusing symlink uninstall directory: $directory" >&2; exit 1; }
done
[[ -d $prefix ]] || { echo "error: prefix does not exist: $prefix" >&2; exit 1; }
prefix=$(cd -- "$prefix" && pwd -P)
[[ $(stat -c %u -- "$prefix") == "$(id -u)" ]] || {
    echo 'error: prefix is not owned by the current user' >&2
    exit 1
}

state_root=
if [[ ${XDG_STATE_HOME:-} == /* ]]; then
    state_root=$XDG_STATE_HOME/gui2tui/headless
elif [[ ${HOME:-} == /* ]]; then
    state_root=$HOME/.local/state/gui2tui/headless
fi
if [[ -n $state_root && ( -e $state_root/session.json || -L $state_root/session.json ) ]]; then
    echo 'error: a Managed Headless descriptor still exists; stop or inspect that owned session before uninstalling' >&2
    printf 'Run: %s/bin/gui2tui setup stop\n' "$prefix" >&2
    exit 1
fi

manifest=$prefix/libexec/gui2tui/install-manifest-v1
[[ ! -L $manifest && -f $manifest ]] || {
    echo 'error: safe uninstall manifest is missing or is a symlink; no files were removed' >&2
    exit 1
}
[[ $(stat -c %u -- "$manifest") == "$(id -u)" && $(stat -c %a -- "$manifest") == 600 \
    && $(stat -c %h -- "$manifest") == 1 ]] || {
    echo 'error: safe uninstall manifest ownership or permissions are invalid; no files were removed' >&2
    exit 1
}
IFS= read -r header <"$manifest"
[[ $header == GUI2TUI_USER_INSTALL_V1 ]] || {
    echo 'error: unrecognized safe uninstall manifest; no files were removed' >&2
    exit 1
}

destinations=(
    bin/gui2tui
    libexec/gui2tui/gui2tui-inspect
    libexec/gui2tui/gui2tui-local
    libexec/gui2tui/headless-session
    libexec/gui2tui/uninstall-user
)
checksum_lines=$(awk 'NF == 2 && length($1) == 64 { count++ } END { print count + 0 }' "$manifest")
[[ $checksum_lines == ${#destinations[@]} ]] || {
    echo 'error: safe uninstall manifest has an unexpected file set; no files were removed' >&2
    exit 1
}

for relative in "${destinations[@]}"; do
    target=$prefix/$relative
    expected=$(awk -v wanted="$relative" '$2 == wanted { print $1 }' "$manifest")
    [[ ${#expected} == 64 ]] || { echo 'error: safe uninstall manifest is incomplete; no files were removed' >&2; exit 1; }
    [[ ! -L $target ]] || { echo "error: refusing symlink managed path: $target" >&2; exit 1; }
    [[ -e $target ]] || continue
    [[ -f $target ]] || { echo "error: managed path is not a regular file: $target" >&2; exit 1; }
    actual=$(sha256sum -- "$target")
    actual=${actual%% *}
    [[ $actual == "$expected" ]] || {
        echo "error: installed file changed since installation; refusing to remove: $target" >&2
        exit 1
    }
done

for relative in "${destinations[@]}"; do
    rm -f -- "$prefix/$relative"
done
rm -f -- "$manifest"
rmdir -- "$prefix/libexec/gui2tui" 2>/dev/null || true

printf 'Removed the recorded GUI2TUI installation from %s\n' "$prefix"
echo 'User configuration, Managed Headless state, runtime data and unrelated prefix files were preserved.'
