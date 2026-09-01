#!/usr/bin/env python3
"""Prove a selector launch wait remains redrawable and cancellable."""

import os
import pathlib
import subprocess
import sys
import tempfile
import time

import pexpect


def main() -> int:
    binary = pathlib.Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="gui2tui-launch-cancel-") as temp:
        env = os.environ.copy()
        env["XDG_CONFIG_HOME"] = str(pathlib.Path(temp) / "config")
        subprocess.run(
            [
                str(binary),
                "app",
                "add",
                "/bin/true",
                "--wait-ms",
                "120000",
            ],
            env=env,
            check=True,
            capture_output=True,
        )
        child = pexpect.spawn(
            str(binary),
            ["--no-mouse"],
            env=env,
            encoding="utf-8",
            timeout=8,
            dimensions=(28, 100),
        )
        try:
            child.expect("true")
            child.send("\r")
            child.expect("remaining")
            started = time.monotonic()
            child.send("\x1b")
            child.expect("cancelled")
            elapsed = time.monotonic() - started
            assert elapsed < 2.0, elapsed
            child.send("q")
            child.expect(pexpect.EOF)
            child.close()
            assert child.exitstatus == 0
        finally:
            if child.isalive():
                child.close(force=True)
    print(f"LAUNCH_CANCEL_LIVE=PASS elapsed_ms={elapsed * 1000:.1f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
