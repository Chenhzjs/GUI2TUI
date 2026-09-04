#!/usr/bin/env python3
"""Focused Phase 0.3D Value affordance and authoritative-state probe."""

import os
import subprocess
import time

import pexpect


child = pexpect.spawn(
    os.environ["GUI2TUI"],
    ["--app", "gui2tui-qt-fixture", "--settle-ms", "200"],
    env=os.environ.copy(),
    encoding=None,
    dimensions=(36, 120),
)
time.sleep(2)

for _ in range(32):
    child.send(b"\t")
    try:
        child.expect(b"Adjust", timeout=0.25)
        break
    except pexpect.TIMEOUT:
        continue
else:
    raise RuntimeError("qualified Value did not expose its contextual Adjust hint")

child.send(b"\x1b[A")
time.sleep(0.8)
increased = subprocess.check_output(
    [os.environ["INSPECT"], "--app", "gui2tui-qt-fixture", "--verbose"],
    env=os.environ.copy(),
    text=True,
)
value_line = next(line for line in increased.splitlines() if 'Slider "Probe value"' in line)
assert 'value="5"' in value_line, value_line

child.send(b"\x1b[B")
time.sleep(0.8)
restored = subprocess.check_output(
    [os.environ["INSPECT"], "--app", "gui2tui-qt-fixture", "--verbose"],
    env=os.environ.copy(),
    text=True,
)
value_line = next(line for line in restored.splitlines() if 'Slider "Probe value"' in line)
assert 'value="4"' in value_line, value_line

child.send(b"q")
child.expect(pexpect.EOF, timeout=8)
transcript = child.before.decode("utf-8", "replace")
assert "Probe progress: 4  [" not in transcript, transcript[-4000:]
print("CAPABILITY_UX_VALUE=PASS")
print("CAPABILITY_UX_VALUE_RESTORATION=PASS")
print("CAPABILITY_UX_PROGRESS_READ_ONLY=PASS")
