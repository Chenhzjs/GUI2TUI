#!/usr/bin/env python3
"""Fail-closed version/provenance checks for a release package."""

import argparse
import hashlib
import json
import platform
import pathlib
import re
import subprocess
import tarfile
import tempfile
import tomllib


parser = argparse.ArgumentParser()
parser.add_argument("--version", required=True)
parser.add_argument("--commit", required=True)
parser.add_argument("--package-dir", type=pathlib.Path, required=True)
parser.add_argument("--release-notes", type=pathlib.Path, required=True)
args = parser.parse_args()

root = pathlib.Path(__file__).resolve().parents[1]
with (root / "Cargo.toml").open("rb") as stream:
    cargo_version = tomllib.load(stream)["package"]["version"]
if cargo_version != args.version:
    raise SystemExit(f"version mismatch: Cargo.toml={cargo_version} expected={args.version}")

with (root / "Cargo.lock").open("rb") as stream:
    lock = tomllib.load(stream)
lock_versions = [
    package["version"]
    for package in lock["package"]
    if package["name"] == "gui2tui"
]
if lock_versions != [args.version]:
    raise SystemExit(f"version mismatch: Cargo.lock={lock_versions} expected={[args.version]}")

notes = args.release_notes.read_text()
if f"v{args.version}" not in notes:
    raise SystemExit("release notes do not identify the expected RC version")

package_dir = args.package_dir.resolve()
archives = sorted(package_dir.glob(f"gui2tui-{args.version}-linux-*.tar.gz"))
if {archive.name for archive in archives} != {
    f"gui2tui-{args.version}-linux-aarch64.tar.gz",
    f"gui2tui-{args.version}-linux-x86_64.tar.gz",
}:
    raise SystemExit("package directory does not contain exactly both expected release archives")

expected_digests = {}
checksum_file = package_dir / "SHA256SUMS"
if checksum_file.is_file():
    for line in checksum_file.read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        expected_digests[name] = digest

manifest_path = package_dir / "RELEASE-MANIFEST.json"
manifest = json.loads(manifest_path.read_text()) if manifest_path.is_file() else None
if manifest is not None:
    if manifest["gui2tui_version"] != args.version or manifest["commit"] != args.commit:
        raise SystemExit("release manifest version or commit mismatch")

with tempfile.TemporaryDirectory(prefix="gui2tui-version-audit-") as scratch:
    for archive in archives:
        match = re.fullmatch(rf"gui2tui-{re.escape(args.version)}-linux-(aarch64|x86_64)\.tar\.gz", archive.name)
        if match is None:
            raise SystemExit(f"unexpected archive name: {archive.name}")
        architecture = match.group(1)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if expected_digests and expected_digests.get(archive.name) != digest:
            raise SystemExit(f"checksum mismatch for {archive.name}")
        if manifest is not None:
            entries = [item for item in manifest["artifacts"] if item["name"] == archive.name]
            if len(entries) != 1 or entries[0]["sha256"] != digest:
                raise SystemExit(f"release manifest artifact mismatch for {archive.name}")

        destination = pathlib.Path(scratch) / architecture
        destination.mkdir()
        with tarfile.open(archive, "r:gz") as payload:
            members = payload.getmembers()
            roots = {pathlib.PurePosixPath(member.name).parts[0] for member in members}
            if len(roots) != 1:
                raise SystemExit(f"archive root mismatch for {archive.name}")
            for member in members:
                path = pathlib.PurePosixPath(member.name)
                if path.is_absolute() or ".." in path.parts:
                    raise SystemExit(f"unsafe archive path in {archive.name}")
                if not (member.isdir() or member.isfile()):
                    raise SystemExit(f"unsafe archive member in {archive.name}: {member.name}")
            payload.extractall(destination)
        bundle = destination / next(iter(roots))
        build = json.loads((bundle / "BUILD-INFO.json").read_text())
        abi = json.loads((bundle / "ABI.json").read_text())
        if build["version"] != args.version or build["commit"] != args.commit:
            raise SystemExit(f"BUILD-INFO mismatch for {archive.name}")
        if abi["gui2tui_version"] != args.version or abi["commit"] != args.commit:
            raise SystemExit(f"ABI metadata mismatch for {archive.name}")
        host_architecture = {
            "aarch64": "aarch64",
            "arm64": "aarch64",
            "amd64": "x86_64",
            "x86_64": "x86_64",
        }.get(platform.machine().lower())
        if platform.system() == "Linux" and host_architecture == architecture:
            actual = subprocess.run(
                [str(bundle / "bin/gui2tui"), "--version"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            if actual != f"gui2tui {args.version}":
                raise SystemExit(f"CLI version mismatch for {archive.name}: {actual!r}")
        else:
            print(
                "CLI_VERSION_CHECK=DEFERRED "
                f"archive={archive.name} host={platform.system()}/{platform.machine()}"
            )

print(f"VERSION_CONSISTENCY=PASS version={args.version} commit={args.commit} archives=2")
