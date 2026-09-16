#!/usr/bin/env python3
"""Deterministic foreground handler for v0.6D ownership evidence."""

import os
import pathlib
import sys
import termios
import time


candidate = pathlib.Path(sys.argv[1])
attributes = termios.tcgetattr(sys.stdin.fileno())
if not attributes[3] & termios.ICANON or not attributes[3] & termios.ECHO:
    raise SystemExit(9)

pid_file = os.environ.get("V06D_HANDLER_PID")
if pid_file:
    pathlib.Path(pid_file).write_text(str(os.getpid()), encoding="ascii")

replacement = os.environ.get("V06D_HANDLER_REPLACEMENT")
text = (
    replacement
    if replacement is not None
    else candidate.read_text(encoding="utf-8") + "v06d handler candidate\n"
)
with candidate.open("r+", encoding="utf-8") as stream:
    stream.seek(0)
    stream.write(text)
    stream.truncate()
    stream.flush()
    os.fsync(stream.fileno())

ready = os.environ.get("V06D_HANDLER_READY")
resume = os.environ.get("V06D_HANDLER_RESUME")
if ready and resume:
    pathlib.Path(ready).touch()
    deadline = time.monotonic() + 30
    while not pathlib.Path(resume).exists():
        if time.monotonic() >= deadline:
            raise SystemExit(8)
        time.sleep(0.05)
