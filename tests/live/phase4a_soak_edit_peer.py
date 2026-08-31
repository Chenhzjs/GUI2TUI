"""Bounded EditableText peer workload in an already isolated soak session.

Uses a second real TUI, not toolkit APIs. Records separately: it must never be
counted as the long-lived TUI's own EditSession activity.
"""
import json
import os
import pathlib
import subprocess
import sys
import time

import pexpect
import pyte

root = pathlib.Path(sys.argv[1]).resolve()
assert root.name.startswith("gui2tui-p4a-soak-")
session_env = None
for process in pathlib.Path("/proc").iterdir():
    if not process.name.isdigit():
        continue
    try:
        argv = (process / "cmdline").read_bytes().split(b"\0")
        if not any(arg.endswith(b"/phase4a_soak.py") for arg in argv):
            continue
        candidate = dict(entry.decode().split("=", 1) for entry in
                         (process / "environ").read_bytes().split(b"\0") if b"=" in entry)
        if candidate.get("RESULT_DIR") == str(root) and candidate.get("DBUS_SESSION_BUS_ADDRESS"):
            session_env = candidate
            break
    except (OSError, UnicodeError):
        continue
assert session_env is not None, "live isolated soak process not found"
binary = pathlib.Path(session_env["TARGET_DIR"]) / "debug"
selector = "gui2tui-live-fixture"
records = []
for iteration in range(8):
    screen = pyte.Screen(140, 70)
    stream = pyte.Stream(screen)
    child = pexpect.spawn(str(binary / "gui2tui"), ["--app", selector],
                          env=session_env, encoding=None, dimensions=(70, 140))
    def pump(seconds):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
            except pexpect.TIMEOUT:
                pass
        return "\n".join(screen.display)
    try:
        pump(1)
        for _ in range(40):
            frame = pump(.08)
            if any(line.strip("│ ").startswith("> Username:") for line in frame.splitlines()):
                break
            child.send(b"\t")
        else:
            raise AssertionError("Username focus unavailable")
        child.send(b"\r")
        assert "[editing]" in pump(.2)
        child.send(b"-peer\r")
        pump(.7)
        tree = subprocess.check_output([str(binary / "gui2tui-inspect"), "--app", selector],
                                      env=session_env, text=True, stderr=subprocess.DEVNULL, timeout=15)
        assert '-peer"' in tree
        records.append({"iteration": iteration, "unix_seconds": time.time(),
                        "editable_text_confirmed": True, "client": "independent TUI peer"})
        with root.joinpath("edit-peer.jsonl").open("a") as output:
            output.write(json.dumps(records[-1]) + "\n")
            output.flush()
    except (AssertionError, pexpect.EOF, subprocess.CalledProcessError):
        # The main soak deliberately replaces the application every 75 s.
        # A peer never retries a write against an old identity: close it and
        # discover a fresh application in the next independent iteration.
        with root.joinpath("edit-peer-skips.jsonl").open("a") as output:
            output.write(json.dumps({"iteration": iteration, "unix_seconds": time.time(),
                                     "result": "target unavailable or confirmation missing"}) + "\n")
    finally:
        if child.isalive():
            child.send(b"\x03")
            try:
                child.expect(pexpect.EOF, timeout=5)
            except pexpect.TIMEOUT:
                child.close(force=True)
    if iteration < 7:
        time.sleep(90)
assert len(records) >= 3, records
print(json.dumps({"confirmed_peer_edits": len(records)}))
