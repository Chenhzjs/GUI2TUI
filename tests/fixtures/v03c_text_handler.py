#!/usr/bin/env python3
"""Deterministic validation-only configured text interaction handler."""

import os
import pathlib
import sys
import termios
import time


path = pathlib.Path(sys.argv[1])
mode = os.environ.get("GUI2TUI_VALIDATION_HANDLER_MODE", "positive")
terminal = termios.tcgetattr(sys.stdin.fileno())
if not terminal[3] & termios.ICANON or not terminal[3] & termios.ECHO:
    raise SystemExit(9)
if mode == "fail":
    raise SystemExit(7)

original = path.read_text(encoding="utf-8")
candidate = original + "handler candidate C\n"
with path.open("r+", encoding="utf-8") as stream:
    stream.seek(0)
    stream.write(candidate)
    stream.truncate()
    stream.flush()
    os.fsync(stream.fileno())

if mode == "conflict":
    ready = pathlib.Path(os.environ["GUI2TUI_VALIDATION_HANDLER_READY"])
    resume = pathlib.Path(os.environ["GUI2TUI_VALIDATION_HANDLER_RESUME"])
    ready.touch()
    deadline = time.monotonic() + 10
    while not resume.exists():
        if time.monotonic() >= deadline:
            raise SystemExit(8)
        time.sleep(0.05)
