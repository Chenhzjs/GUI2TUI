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
python3 - "$archive" <<'PY'
import pathlib
import sys
import tarfile

archive = pathlib.Path(sys.argv[1])
with tarfile.open(archive, "r:gz") as payload:
    members = payload.getmembers()
    names = [member.name for member in members]
    if len(names) != len(set(names)):
        raise SystemExit("archive safety gate failed: duplicate member")
    roots = {pathlib.PurePosixPath(name).parts[0] for name in names if name}
    if len(roots) != 1:
        raise SystemExit("archive safety gate failed: expected one top-level directory")
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts:
            raise SystemExit("archive safety gate failed: unsafe member path")
        if any(part in {".DS_Store", "__MACOSX"} or part.startswith("._") for part in path.parts):
            raise SystemExit(
                f"archive hygiene gate failed: host metadata is forbidden: {member.name}"
            )
        if not (member.isdir() or member.isfile()):
            raise SystemExit(
                f"archive safety gate failed: links and special files are forbidden: {member.name}"
            )
        if member.mode & 0o022:
            raise SystemExit(
                f"archive safety gate failed: group/world-writable member: {member.name}"
            )
PY
tar -xzf "$archive" -C "$temp"
bundle="$temp/$name"
for file in bin/gui2tui libexec/gui2tui/gui2tui-inspect libexec/gui2tui/gui2tui-local libexec/gui2tui/headless-session install-user.sh uninstall-user.sh README.md LICENSE-MIT LICENSE-APACHE config.example.toml DEPENDENCIES.txt BUILD-INFO.json ABI.json smoke/run.sh; do
    test -e "$bundle/$file" || { echo "missing bundle entry: $file" >&2; exit 1; }
done
version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$bundle/BUILD-INFO.json")
test "$("$bundle/bin/gui2tui" --version)" = "gui2tui $version"
"$bundle/libexec/gui2tui/headless-session" --help >/dev/null
python3 "$(dirname -- "$0")/release-abi.py" "$bundle" "$temp/actual-abi.json"
cmp "$bundle/ABI.json" "$temp/actual-abi.json"
# Scan binary bytes too; -l reports only filenames, never embedded user content.
if grep -R -a -l -E '/Users/chenhz/|/home/runner/work/' "$bundle"; then echo 'developer path leaked into bundle' >&2; exit 1; fi
if grep -R -I -n -E 'browser-phase-secret|phase-two-secret|phase-zero-secret|firefox-phase-secret' "$bundle" --exclude='release_smoke_gtk.py'; then echo 'test sentinel leaked outside smoke fixture' >&2; exit 1; fi
if [[ "$smoke" == --smoke ]]; then "$bundle/smoke/run.sh"; fi
echo "RELEASE_VALIDATION=PASS archive=$(basename -- "$archive") version=$version smoke=$([[ "$smoke" == --smoke ]] && echo true || echo false)"
