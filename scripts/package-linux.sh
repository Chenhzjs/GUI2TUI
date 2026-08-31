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
cargo build --locked --release --bins
version=$("$target/release/gui2tui" --version | awk '{print $2}')
name="gui2tui-${version}-linux-$(uname -m)"
stage=$(mktemp -d "$output/package-XXXXXX")
trap 'rmdir "$stage" 2>/dev/null || true' EXIT
mkdir -p "$stage/$name/bin" "$stage/$name/smoke"
install -m 755 "$target/release/gui2tui" "$target/release/gui2tui-inspect" "$target/release/gui2tui-local" "$stage/$name/bin/"
cp README.md LICENSE-MIT LICENSE-APACHE config.example.toml "$stage/$name/"
cp -R docs "$stage/$name/docs"
install -m 755 scripts/release-smoke.sh "$stage/$name/smoke/run.sh"
cp tests/live/release_smoke.py tests/fixtures/release_smoke_gtk.py "$stage/$name/smoke/"
ldd "$stage/$name/bin/gui2tui" "$stage/$name/bin/gui2tui-inspect" "$stage/$name/bin/gui2tui-local" > "$stage/$name/DEPENDENCIES.txt"
tar -C "$stage" -czf "$output/$name.tar.gz" "$name"
(cd "$output" && sha256sum "$name.tar.gz") > "$output/$name.tar.gz.sha256"
echo "ARCHIVE=$output/$name.tar.gz"
echo "STAGING=$stage/$name (retained for inspection)"
