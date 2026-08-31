#!/usr/bin/env python3
"""Assemble exactly two validated native archives into checksums and a manifest."""
import argparse
import hashlib
import json
import pathlib
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=pathlib.Path)
parser.add_argument("--version", required=True)
parser.add_argument("--commit", required=True)
args = parser.parse_args()
root = args.directory.resolve()
artifacts = []
expected = {"x86_64", "aarch64"}
expected_archives = {f"gui2tui-{args.version}-linux-{arch}.tar.gz" for arch in expected}
if {path.name for path in root.glob("*.tar.gz")} != expected_archives:
    raise SystemExit("assembly gate failed: expected exactly the two native archives")
for architecture in sorted(expected):
    archive = root / f"gui2tui-{args.version}-linux-{architecture}.tar.gz"
    abi_path = root / f"gui2tui-{args.version}-linux-{architecture}.abi.json"
    smoke_path = root / f"gui2tui-{args.version}-linux-{architecture}.smoke.txt"
    for path in (archive, abi_path, smoke_path):
        if not path.is_file():
            raise SystemExit(f"assembly gate failed: missing {path.name}")
    if "PACKAGED_FRESH_HOME_SMOKE=PASS" not in smoke_path.read_text():
        raise SystemExit(f"assembly gate failed: smoke did not pass for {architecture}")
    abi = json.loads(abi_path.read_text())
    if abi["architecture"] != architecture or abi["gui2tui_version"] != args.version or abi["commit"] != args.commit:
        raise SystemExit(f"assembly gate failed: ABI metadata mismatch for {architecture}")
    prefix = f"gui2tui-{args.version}-linux-{architecture}"
    with tarfile.open(archive, "r:gz") as tar:
        build = json.load(tar.extractfile(f"{prefix}/BUILD-INFO.json"))
        bundled_abi = json.load(tar.extractfile(f"{prefix}/ABI.json"))
    if build["version"] != args.version or build["commit"] != args.commit or build["architecture"] != architecture:
        raise SystemExit(f"assembly gate failed: bundle metadata mismatch for {architecture}")
    if bundled_abi != abi:
        raise SystemExit(f"assembly gate failed: external/bundled ABI mismatch for {architecture}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    artifacts.append({"name": archive.name, "architecture": architecture, "sha256": digest,
                      "size_bytes": archive.stat().st_size, "glibc_max": abi["glibc_max"],
                      "elf_machine": abi["elf_machine"]})
if {item["architecture"] for item in artifacts} != expected:
    raise SystemExit("assembly gate failed: architecture set mismatch")
(root / "SHA256SUMS").write_text("".join(f"{item['sha256']}  {item['name']}\n" for item in artifacts))
manifest = {"schema_version": 1, "gui2tui_version": args.version, "commit": args.commit, "artifacts": artifacts}
(root / "RELEASE-MANIFEST.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
print("RELEASE_ASSEMBLY=PASS architectures=x86_64,aarch64 artifacts=2")
