#!/usr/bin/env bash
# Build a native Linux tarball; cross-architecture artifacts are never relabelled.
set -euo pipefail
[[ $(uname -s) == Linux ]] || { echo 'Run this builder on Linux' >&2; exit 1; }
project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$project"
target=${CARGO_TARGET_DIR:-$project/target}
output=${RELEASE_DIR:-$project/dist}
mkdir -p "$output"
output=$(cd "$output" && pwd)
# Remove checkout paths from embedded panic/source locations as well as metadata.
build_flags=$(python3 - "$project" <<'PY'
import os, sys
existing = os.environ.get("CARGO_ENCODED_RUSTFLAGS")
flags = existing.split("\x1f") if existing else os.environ.get("RUSTFLAGS", "").split()
flags.append(f"--remap-path-prefix={sys.argv[1]}=gui2tui-source")
print("\x1f".join(flags), end="")
PY
)
CARGO_ENCODED_RUSTFLAGS="$build_flags" cargo build --locked --release --bins
version=$("$target/release/gui2tui" --version | awk '{print $2}')
architecture=$(uname -m)
case "$architecture" in x86_64|aarch64) ;; *) echo "Unsupported release architecture: $architecture" >&2; exit 1;; esac
name="gui2tui-${version}-linux-${architecture}"
stage=$(mktemp -d "$output/package-XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/$name/bin" "$stage/$name/libexec/gui2tui" "$stage/$name/smoke"
install -m 755 "$target/release/gui2tui" "$stage/$name/bin/"
install -m 755 "$target/release/gui2tui-inspect" "$target/release/gui2tui-local" "$stage/$name/libexec/gui2tui/"
install -m 755 scripts/headless-session "$stage/$name/libexec/gui2tui/"
cp README.md LICENSE-MIT LICENSE-APACHE config.example.toml "$stage/$name/"
cp -R docs "$stage/$name/docs"
install -m 755 scripts/release-smoke.sh "$stage/$name/smoke/run.sh"
cp tests/live/release_smoke.py tests/fixtures/release_smoke_gtk.py "$stage/$name/smoke/"
commit=${RELEASE_COMMIT:-$(git rev-parse HEAD)}
baseline=${RELEASE_RUNNER_BASELINE:-$(. /etc/os-release; printf '%s %s' "$ID" "$VERSION_ID")}
VERSION="$version" COMMIT="$commit" ARCHITECTURE="$architecture" BASELINE="$baseline" \
python3 - "$stage/$name/BUILD-INFO.json" <<'PY'
import json, os, sys
json.dump({"schema_version": 1, "version": os.environ["VERSION"], "commit": os.environ["COMMIT"],
           "architecture": os.environ["ARCHITECTURE"], "runner_baseline": os.environ["BASELINE"],
           "profile": "release", "cargo_locked": True}, open(sys.argv[1], "w"), indent=2, sort_keys=True)
open(sys.argv[1], "a").write("\n")
PY
abi_args=()
[[ -n "${RELEASE_MAX_GLIBC:-}" ]] && abi_args+=(--max-glibc "$RELEASE_MAX_GLIBC")
python3 scripts/release-abi.py "$stage/$name" "$stage/$name/ABI.json" "${abi_args[@]}"
cp "$stage/$name/ABI.json" "$output/$name.abi.json"
python3 - "$stage/$name/ABI.json" "$stage/$name/DEPENDENCIES.txt" <<'PY'
import json, sys
d=json.load(open(sys.argv[1])); lines=[f"architecture={d['architecture']}", f"elf_machine={d['elf_machine']}",
f"runner_baseline={d['runner_baseline']}", f"glibc_max={d['glibc_max']}", f"glibcxx_max={d['glibcxx_max'] or 'none'}"]
for b in d['binaries']: lines.append(f"{b['name']}: " + ", ".join(b['dependencies']))
open(sys.argv[2], 'w').write("\n".join(lines)+"\n")
PY
epoch=${SOURCE_DATE_EPOCH:-$(git show -s --format=%ct HEAD)}
tar --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner -C "$stage" -cf - "$name" | gzip -n > "$output/$name.tar.gz"
(cd "$output" && sha256sum "$name.tar.gz") > "$output/$name.tar.gz.sha256"
echo "ARCHIVE=$output/$name.tar.gz"
echo "ABI_REPORT=$output/$name.abi.json"
