#!/usr/bin/env python3
"""Bounded 1.0B PTY size/resize and recovery evidence.

This validation-only harness drives the installed binary in the disposable
v0.7 headless container.  It intentionally uses the ordinary TUI command
palette for the semantic operation and Inspector only for independent
authoritative readback.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import pathlib
import pty
import re
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time

from v07c_headless_driver import ANSI, PtyProcess, ScreenDriver


HOME_ENV = {
    "HOME": "/home/gui2tui",
    "XDG_CONFIG_HOME": "/home/gui2tui/.config",
    "XDG_STATE_HOME": "/home/gui2tui/.local/state",
    "XDG_RUNTIME_DIR": "/home/gui2tui/.local/run",
    "LANG": "C.UTF-8",
    "TERM": "xterm-256color",
}
GUI = "/home/gui2tui/.local/bin/gui2tui"
CONTROL = "/usr/local/libexec/gui2tui-v07c-container-control"
INSPECT = "/home/gui2tui/.local/libexec/gui2tui/gui2tui-inspect"
APP = "gui2tui-v05a-gtk-selection"


def docker_exec(container: str, *command: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    args = ["docker", "exec", "--user", "gui2tui"]
    for key, value in HOME_ENV.items():
        args.extend(["--env", f"{key}={value}"])
    args.extend([container, *command])
    return subprocess.run(args, check=check, text=True, capture_output=True)


def resize(child: PtyProcess, rows: int, columns: int) -> None:
    fcntl.ioctl(
        child.child_fd,
        termios.TIOCSWINSZ,
        struct.pack("HHHH", rows, columns, 0, 0),
    )


class Shell(ScreenDriver):
    def __init__(self, container: str, dimensions: tuple[int, int]) -> None:
        command = ["docker", "exec", "-it", "--user", "gui2tui"]
        for key, value in HOME_ENV.items():
            command.extend(["--env", f"{key}={value}"])
        command.extend([container, "bash", "--noprofile", "--norc"])
        super().__init__(PtyProcess(command, dimensions))

    def start(self) -> None:
        self.send(
            b"export HOME=/home/gui2tui XDG_CONFIG_HOME=/home/gui2tui/.config "
            b"XDG_STATE_HOME=/home/gui2tui/.local/state "
            b"XDG_RUNTIME_DIR=/home/gui2tui/.local/run LANG=C.UTF-8 TERM=xterm-256color; "
            b"PS1='V10B_PROMPT> '; printf V10B_READY\n"
        )
        self.wait_text("V10B_READY", timeout=15)

    def sendline(self, data: bytes) -> None:
        self.send(data + b"\n")

    def wait_fresh_text(self, wanted: str, timeout: float = 10) -> str:
        start = len(self.transcript)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.pump(0.08)
            fresh = ANSI.sub(b"", bytes(self.transcript[start:])).decode("utf-8", "replace")
            if wanted in fresh or re.sub(r"\s+", "", wanted) in re.sub(r"\s+", "", fresh):
                return fresh
        raise AssertionError(f"terminal did not newly show {wanted!r}")

    def start_tui(self) -> None:
        self.sendline(
            f"{GUI} --session managed --app {APP} --layout spatial --settle-ms 300 --no-mouse".encode()
        )
        self.wait_text("Reorder current items", timeout=20)

    def finish(self) -> None:
        self.send(b"q")
        self.wait_fresh_text("V10B_PROMPT", timeout=15)
        self.sendline(b"exit")
        # Docker's interactive exec can retain the PTY until it observes an
        # input EOF even after bash accepted `exit`; make the validation
        # cleanup deterministic without changing the product path.
        self.send(b"\x04")
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline and self.child.isalive():
            self.pump(0.05)
        if self.child.isalive():
            self.child.close(force=True)
            raise AssertionError("terminal shell did not exit after TUI quit")


def wait_authoritative(container: str, wanted: str) -> str:
    deadline = time.monotonic() + 15
    last = ""
    while time.monotonic() < deadline:
        result = docker_exec(container, INSPECT, "--session", "managed", "--app", APP, check=False)
        last = result.stdout
        if result.returncode == 0 and wanted in last:
            return last
        time.sleep(0.1)
    raise AssertionError(f"authoritative readback missing {wanted!r}: {last[-1000:]}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--project-root", required=True, type=pathlib.Path)
    parser.add_argument("--evidence", required=True, type=pathlib.Path)
    args = parser.parse_args()
    args.evidence.mkdir(mode=0o700, parents=True, exist_ok=False)

    full_commit = subprocess.check_output(
        ["git", "-C", str(args.project_root), "rev-parse", "HEAD"], text=True
    ).strip()
    short = full_commit[:12]
    image = f"gui2tui-v10b-terminal:{short}"
    container = f"gui2tui-v10b-terminal-{os.getpid()}"
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="gui2tui-v10b-terminal-"))
    key = scratch / "id_ed25519"
    started = False
    image_created = False
    results: dict[str, object] = {"source_commit": full_commit}

    def cleanup() -> None:
        if started:
            docker_exec(container, CONTROL, "stop", check=False)
            subprocess.run(["docker", "rm", "-f", container], check=False, capture_output=True)
        if image_created:
            subprocess.run(["docker", "image", "rm", image], check=False, capture_output=True)
        shutil.rmtree(scratch, ignore_errors=True)

    try:
        subprocess.run(
            ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(key)], check=True
        )
        subprocess.run(
            [
                "docker", "build", "--file", str(args.project_root / "tests/live/Dockerfile.v07c-headless"),
                "--build-arg", f"GUI2TUI_SOURCE_COMMIT={full_commit}",
                "--tag", image, str(args.project_root),
            ], check=True, stdout=subprocess.DEVNULL
        )
        image_created = True
        subprocess.run(
            [
                "docker", "run", "-d", "--name", container, "--publish", "127.0.0.1::22",
                "--mount", f"type=bind,src={key}.pub,dst=/run/gui2tui-test/authorized_key.pub,readonly",
                image,
            ], check=True, stdout=subprocess.DEVNULL
        )
        started = True
        time.sleep(0.5)
        docker_exec(container, CONTROL, "install")
        docker_exec(container, CONTROL, "setup")
        docker_exec(container, CONTROL, "start-fixture", "selection")

        sizes = [(48, 160), (32, 100), (24, 72), (16, 60), (12, 50)]
        observations = []
        shell = Shell(container, sizes[0])
        try:
            shell.start()
            shell.start_tui()
            for rows, columns in sizes:
                resize(shell.child, rows, columns)
                shell.pump(0.35)
                frame = ANSI.sub(b"", bytes(shell.transcript[-12000:])).decode("utf-8", "replace")
                compact = re.sub(r"\s+", "", frame)
                observations.append({
                    "rows": rows,
                    "columns": columns,
                    "alive": shell.child.isalive(),
                    "has_reachable_action_text": "Reordercurrentitems" in compact,
                    "has_panic": "panicked at" in frame or "thread 'main' panicked" in frame,
                    "frame_tail": frame[-500:],
                })
                if not shell.child.isalive():
                    raise AssertionError(f"TUI exited after resize to {rows}x{columns}")

            # Resize while a presentation-only Choice overlay is open, cancel it,
            # then resize again before a real command-palette operation.
            shell.send(b"\r")
            shell.wait_text("Esc Cancel", timeout=10)
            resize(shell.child, 20, 70)
            shell.pump(0.35)
            shell.send(b"\x1b")
            shell.pump(0.35)
            shell.send(b":")
            shell.wait_text("Command palette", timeout=10)
            shell.send(b"Reorder current items\r")
            readback = wait_authoritative(container, "Structure: reordered; selected Gamma")
            shell.pump(0.5)
            shell.pump(5.0)
            shell.send(b"r")
            shell.pump(5.0)
            shell.send(b":")
            shell.wait_fresh_text("Command palette", timeout=15)
            shell.send(b"\x1b")
            shell.pump(0.5)
            resize(shell.child, 48, 160)
            shell.pump(0.35)
            shell.send(b":")
            shell.wait_text("Command palette", timeout=10)
            shell.send(b"Reset current items\r")
            reset = wait_authoritative(container, "Selected: Alpha")
            shell.pump(0.5)
            results.update({
                "resize_matrix": observations,
                "modal_overlay_resize_cancel": "PASS",
                "semantic_operation_after_resize": "PASS",
                "authoritative_reorder_readback": "PASS",
                "authoritative_reset_readback": "PASS",
                "reorder_readback_tail": readback[-600:],
                "reset_readback_tail": reset[-600:],
            })
            shell.finish()
        finally:
            if shell.child.isalive():
                shell.child.close(force=True)

        if any(item["has_panic"] for item in observations):
            raise AssertionError("panic text appeared in a resized terminal frame")
        (args.evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    finally:
        cleanup()


if __name__ == "__main__":
    main()
