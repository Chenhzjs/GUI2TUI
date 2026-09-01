#!/usr/bin/env python3
"""Exercise one real executable through add -> discovery -> semantic TUI."""

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time

import pexpect


def main() -> int:
    if len(sys.argv) < 3:
        raise SystemExit("usage: launcher_real_app.py GUI2TUI PROGRAM [ARG ...]")
    binary = pathlib.Path(sys.argv[1]).resolve()
    program, *program_args = sys.argv[2:]
    inspector = binary.with_name("gui2tui-inspect")
    launcher_id = pathlib.Path(program).name
    with tempfile.TemporaryDirectory(prefix="gui2tui-real-launcher-") as temp:
        env = os.environ.copy()
        env["XDG_CONFIG_HOME"] = str(pathlib.Path(temp) / "config")
        add = [
            str(binary),
            "app",
            "add",
            program,
            "--wait-ms",
            "12000",
        ]
        if program_args:
            add += ["--", *program_args]
        registered = subprocess.run(add, env=env, text=True, capture_output=True)
        if registered.returncode != 0:
            print(json.dumps({"result": "ADD_FAILED", "stderr": registered.stderr}))
            return 2

        child = pexpect.spawn(
            str(binary),
            ["launch", launcher_id, "--no-mouse"],
            env=env,
            encoding="utf-8",
            timeout=25,
            dimensions=(32, 120),
        )
        transcript = ""
        try:
            applications = []
            listed = None
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline and child.isalive():
                listed = subprocess.run(
                    [str(inspector), "--list"],
                    env=env,
                    text=True,
                    capture_output=True,
                    timeout=10,
                )
                applications = listed.stdout.strip().splitlines()
                if applications:
                    break
                time.sleep(0.2)
            if not applications:
                transcript = child.before or ""
                child.close(force=True)
                print(
                    json.dumps(
                        {"result": "LAUNCH_FAILED", "transcript": transcript[-2000:]},
                        ensure_ascii=False,
                    )
                )
                return 3
            time.sleep(1)
            saved = subprocess.run(
                [str(binary), "app", "list"],
                env=env,
                text=True,
                capture_output=True,
                timeout=10,
            )
            child.send("q")
            child.expect(pexpect.EOF)
            child.close()
            print(
                json.dumps(
                    {
                        "result": "PASS",
                        "program": program,
                        "applications": applications,
                        "launcher": saved.stdout.strip(),
                        "tui_cache_loaded": True,
                    },
                    ensure_ascii=False,
                )
            )
            return 0
        finally:
            if child.isalive():
                child.close(force=True)


if __name__ == "__main__":
    raise SystemExit(main())
