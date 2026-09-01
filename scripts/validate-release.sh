#!/usr/bin/env bash
# Validate one final archive. Smoke always executes the extracted bundle's harness.
set -euo pipefail
archive=${1:?usage: validate-release.sh ARCHIVE [--smoke]}
smoke=${2:-}
archive=$(cd -- "$(dirname -- "$archive")" && pwd)/$(basename -- "$archive")
name=$(basename -- "$archive" .tar.gz)
case "$name" in gui2tui-*-linux-x86_64|gui2tui-*-linux-aarch64) ;; *) echo "unexpected release name: $name" >&2; exit 1;; esac
temp=$(mktemp -d)
trap 'rm -rf -- "$temp"' EXIT
tar -tzf "$archive" >"$temp/layout.txt"
if grep -Eq '^/|(^|/)\.\.(/|$)' "$temp/layout.txt"; then echo 'unsafe archive path' >&2; exit 1; fi
tar -xzf "$archive" -C "$temp"
bundle="$temp/$name"
for file in bin/gui2tui bin/gui2tui-inspect bin/gui2tui-local README.md LICENSE-MIT LICENSE-APACHE config.example.toml DEPENDENCIES.txt BUILD-INFO.json ABI.json smoke/run.sh; do
    test -e "$bundle/$file" || { echo "missing bundle entry: $file" >&2; exit 1; }
done
version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$bundle/BUILD-INFO.json")
test "$("$bundle/bin/gui2tui" --version)" = "gui2tui $version"
if [[ -e $bundle/bin/gui2tui-headless ]]; then
    [[ -x $bundle/bin/gui2tui-headless ]] || {
        echo "headless helper is not executable" >&2
        exit 1
    }
    "$bundle/bin/gui2tui-headless" --help >/dev/null
fi
python3 "$(dirname -- "$0")/release-abi.py" "$bundle" "$temp/actual-abi.json"
cmp "$bundle/ABI.json" "$temp/actual-abi.json"
# Scan binary bytes too; -l reports only filenames, never embedded user content.
if grep -R -a -l -E '/Users/chenhz/|/home/runner/work/' "$bundle"; then echo 'developer path leaked into bundle' >&2; exit 1; fi
if grep -R -I -n -E 'browser-phase-secret|phase-two-secret|phase-zero-secret|firefox-phase-secret' "$bundle" --exclude='release_smoke_gtk.py'; then echo 'test sentinel leaked outside smoke fixture' >&2; exit 1; fi
if [[ "$smoke" == --smoke ]]; then "$bundle/smoke/run.sh"; fi
echo "RELEASE_VALIDATION=PASS archive=$(basename -- "$archive") version=$version smoke=$([[ "$smoke" == --smoke ]] && echo true || echo false)"
