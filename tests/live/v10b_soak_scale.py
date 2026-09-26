#!/usr/bin/env python3
"""Bounded 1.0B soak/resource campaign on the installed headless package."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time

from v10b_terminal_recovery import CONTROL, Shell, docker_exec, wait_authoritative


def resources(container: str) -> dict[str, object]:
    code = r'''
import json, os, pathlib, subprocess
uid = str(os.geteuid())
pids = [int(value) for value in subprocess.check_output(
    ["pgrep", "-u", uid, "-x", "gui2tui"], text=True).split()]
rows = []
for pid in pids:
    root = pathlib.Path("/proc") / str(pid)
    status = (root / "status").read_text()
    rss = next(int(line.split()[1]) for line in status.splitlines() if line.startswith("VmRSS:"))
    threads = next(int(line.split()[1]) for line in status.splitlines() if line.startswith("Threads:"))
    rows.append({"pid": pid, "rss_kib": rss, "threads": threads,
                 "fds": len(list((root / "fd").iterdir()))})
owned = pathlib.Path("/home/gui2tui/.local/run/gui2tui-owned-1001")
artifacts = list(owned.glob("operation-*/*")) if owned.exists() else []
all_processes = subprocess.check_output(["ps", "-u", uid, "-o", "comm="], text=True).split()
print(json.dumps({"processes": rows, "child_process_count": len(all_processes),
                  "child_process_names": sorted(set(all_processes)),
                  "artifact_files": sum(path.name.startswith("artifact-") for path in artifacts),
                  "operation_namespaces": sum(path.name.startswith("operation-") for path in owned.iterdir())
                  if owned.exists() else 0}))
'''
    result = docker_exec(container, "python3", "-c", code, check=False)
    if result.returncode != 0:
        raise AssertionError(f"resource sample failed: {result.stderr}")
    data = json.loads(result.stdout)
    processes = data.pop("processes")
    data["gui2tui_pids"] = [row["pid"] for row in processes]
    data["rss_kib"] = sum(row["rss_kib"] for row in processes)
    data["fds"] = sum(row["fds"] for row in processes)
    data["threads"] = sum(row["threads"] for row in processes)
    return data


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-root", required=True, type=pathlib.Path)
    parser.add_argument("--evidence", required=True, type=pathlib.Path)
    parser.add_argument("--duration", type=int, default=2700)
    args = parser.parse_args()
    if args.duration < 2700:
        raise SystemExit("1.0B soak evidence requires at least 2700 seconds")
    args.evidence.mkdir(mode=0o700, parents=True, exist_ok=False)

    full_commit = subprocess.check_output(
        ["git", "-C", str(args.project_root), "rev-parse", "HEAD"], text=True
    ).strip()
    short = full_commit[:12]
    image = f"gui2tui-v10b-soak:{short}"
    container = f"gui2tui-v10b-soak-{os.getpid()}"
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="gui2tui-v10b-soak-"))
    key = scratch / "id_ed25519"
    started = False
    image_created = False
    samples: list[dict[str, object]] = []
    counters = {"operations": 0, "refreshes": 0, "overlay_cancels": 0}
    result_file = args.evidence / "soak-results.json"

    def write_partial() -> None:
        result_file.write_text(
            json.dumps({"source_commit": full_commit, "duration_seconds": args.duration,
                        "counters": counters, "samples": samples}, indent=2) + "\n",
            encoding="utf-8",
        )

    def cleanup() -> None:
        if started:
            docker_exec(container, "/usr/local/libexec/gui2tui-v07c-container-control", "stop", check=False)
            subprocess.run(["docker", "rm", "-f", container], check=False, capture_output=True)
        if image_created:
            subprocess.run(["docker", "image", "rm", image], check=False, capture_output=True)
        shutil.rmtree(scratch, ignore_errors=True)

    try:
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(key)], check=True)
        subprocess.run([
            "docker", "build", "--file", str(args.project_root / "tests/live/Dockerfile.v07c-headless"),
            "--build-arg", f"GUI2TUI_SOURCE_COMMIT={full_commit}", "--tag", image, str(args.project_root)
        ], check=True, stdout=subprocess.DEVNULL)
        image_created = True
        subprocess.run([
            "docker", "run", "-d", "--name", container, "--publish", "127.0.0.1::22",
            "--mount", f"type=bind,src={key}.pub,dst=/run/gui2tui-test/authorized_key.pub,readonly", image
        ], check=True, stdout=subprocess.DEVNULL)
        started = True
        time.sleep(0.5)
        control = "/usr/local/libexec/gui2tui-v07c-container-control"
        docker_exec(container, control, "install")
        docker_exec(container, control, "setup")
        docker_exec(container, control, "start-fixture", "selection")

        shell = Shell(container, (48, 160))
        try:
            shell.start()
            shell.start_tui()
            started_at = time.monotonic()
            next_action = started_at
            next_sample = started_at
            cycle = 0
            while time.monotonic() - started_at < args.duration:
                now = time.monotonic()
                if now >= next_action:
                    if cycle == 0:
                        shell.send(b"\r")
                        shell.wait_fresh_text("Esc Cancel", timeout=10)
                        shell.send(b"\x1b")
                        shell.pump(0.25)
                        counters["overlay_cancels"] += 1
                    query, wanted = (
                        ("Reorder current items", "Structure: reordered; selected Gamma")
                        if cycle % 2 == 0 else ("Reset current items", "Selected: Alpha")
                    )
                    shell.send(b":")
                    shell.wait_fresh_text("Command palette", timeout=10)
                    shell.send(query.encode() + b"\r")
                    wait_authoritative(container, wanted)
                    shell.pump(5.0)
                    counters["operations"] += 1
                    if cycle % 5 == 0:
                        shell.send(b"r")
                        shell.pump(5.0)
                        counters["refreshes"] += 1
                    cycle += 1
                    next_action = now + 30.0
                if now >= next_sample:
                    sample = resources(container)
                    sample["elapsed_seconds"] = round(now - started_at, 2)
                    samples.append(sample)
                    write_partial()
                    next_sample = now + 60.0
                time.sleep(0.1)
            shell.send(b":")
            shell.wait_fresh_text("Command palette", timeout=10)
            shell.send(b"Reset current items\r")
            wait_authoritative(container, "Selected: Alpha")
            counters["operations"] += 1
            samples.append({"elapsed_seconds": round(time.monotonic() - started_at, 2), **resources(container)})
            write_partial()
            shell.finish()
        finally:
            if shell.child.isalive():
                shell.child.close(force=True)
    finally:
        cleanup()


if __name__ == "__main__":
    main()
