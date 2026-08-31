#!/usr/bin/env python3
"""Inspect the final release binaries and emit a stable, path-free ABI report."""
import argparse
import json
import os
import pathlib
import re
import subprocess
import sys


def command(*args: str) -> str:
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def versions(text: str, namespace: str) -> list[tuple[int, ...]]:
    return [tuple(map(int, match.split("."))) for match in re.findall(rf"{namespace}_([0-9]+(?:\.[0-9]+)+)", text)]


def format_version(value: tuple[int, ...] | None) -> str | None:
    return ".".join(map(str, value)) if value else None


parser = argparse.ArgumentParser()
parser.add_argument("bundle", type=pathlib.Path)
parser.add_argument("output", type=pathlib.Path)
parser.add_argument("--max-glibc")
args = parser.parse_args()
bundle = args.bundle.resolve()
build = json.loads((bundle / "BUILD-INFO.json").read_text())
binaries = []
architectures = set()
all_glibc: list[tuple[int, ...]] = []
all_glibcxx: list[tuple[int, ...]] = []
for name in ("gui2tui", "gui2tui-inspect", "gui2tui-local"):
    path = bundle / "bin" / name
    header = command("readelf", "-h", str(path))
    machine = next(line.split(":", 1)[1].strip() for line in header.splitlines() if "Machine:" in line)
    architectures.add(machine)
    dynamic = command("objdump", "-p", str(path))
    needed = sorted(set(re.findall(r"^\s*NEEDED\s+(\S+)", dynamic, re.MULTILINE)))
    version_info = command("readelf", "--version-info", str(path))
    glibc = versions(version_info, "GLIBC")
    glibcxx = versions(version_info, "GLIBCXX")
    all_glibc.extend(glibc)
    all_glibcxx.extend(glibcxx)
    binaries.append({
        "name": name,
        "elf_machine": machine,
        "dependencies": needed,
        "glibc_max": format_version(max(glibc, default=None)),
        "glibcxx_max": format_version(max(glibcxx, default=None)),
    })
if len(architectures) != 1:
    raise SystemExit(f"ABI gate failed: mixed ELF machines: {sorted(architectures)}")
glibc_max = max(all_glibc, default=None)
if args.max_glibc and glibc_max > tuple(map(int, args.max_glibc.split("."))):
    raise SystemExit(f"ABI gate failed: GLIBC {format_version(glibc_max)} exceeds {args.max_glibc}")
report = {
    "schema_version": 1,
    "gui2tui_version": build["version"],
    "commit": build["commit"],
    "architecture": build["architecture"],
    "elf_machine": next(iter(architectures)),
    "runner_baseline": build["runner_baseline"],
    "glibc_max": format_version(glibc_max),
    "glibcxx_max": format_version(max(all_glibcxx, default=None)),
    "binaries": binaries,
}
args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
print(f"ABI_GATE=PASS architecture={report['architecture']} glibc_max={report['glibc_max']} elf={report['elf_machine']}")
