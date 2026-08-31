"""Reader semantic-position detach/fallback and controlled terminal panic."""
import json
import os
import pathlib
import re
import signal
import subprocess
import termios
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
selector = "Google Chrome"


def inspect(*args, check=True):
    return subprocess.run([str(binary / "gui2tui-inspect"), *args], text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          check=check, timeout=30)


deadline = time.monotonic() + 12
while time.monotonic() < deadline:
    if selector in inspect("--list", check=False).stdout:
        break
    time.sleep(.1)
else:
    raise AssertionError("Chrome accessibility tree unavailable")

screen = pyte.Screen(140, 60)
stream = pyte.Stream(screen)


def pump(child, seconds=.4):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def status(child):
    child.send(b"\x1b[24~")
    frame = pump(child, .3)
    clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
    value = json.loads(clean[clean.index("{"):clean.rindex("}") + 1])
    child.send(b"\x1b")
    pump(child, .1)
    return value


child = pexpect.spawn(str(binary / "gui2tui"), ["--app", selector], env=os.environ.copy(),
                     encoding=None, dimensions=(60, 140))
try:
    pump(child, 2)
    child.send(b"\r")  # focused document summary -> Reader
    pump(child, 1)
    child.send(b"\x1b[B\x1b[B\x1b[B")
    pump(child, .8)
    before = status(child)
    assert before["reader"] is not None, before
    stable_position = before["reader"]["position"]
    tree = inspect("--app", selector).stdout
    insert_line = next(line for line in tree.splitlines() if 'Button "Insert article heading"' in line)
    insert_node = re.search(r"atspi1_[A-Za-z0-9_-]+", insert_line).group()

    os.kill(child.pid, signal.SIGUSR1)
    time.sleep(.3)
    # Chrome currently exposes anonymous actions. Explicit inspector index is
    # diagnostic-only; semantic TUI still refuses anonymous activation.
    inspect("--action", insert_node, "--index", "0")
    time.sleep(1)
    os.kill(child.pid, signal.SIGUSR2)
    pump(child, 1)
    resumed = status(child)
    assert resumed["reader"]["position"] == stable_position, (before, resumed)
    print(f"READER_DETACH_RESUME=PASS content_block_id={stable_position}")

    # Resolve the mutable paragraph by semantic content search, not terminal row.
    child.send(b"/Status: original article paragraph\r")
    pump(child, 1)
    target = status(child)
    stale_position = target["reader"]["position"]
    tree = inspect("--app", selector).stdout
    remove_line = next(line for line in tree.splitlines() if 'Button "Remove article paragraph"' in line)
    remove_node = re.search(r"atspi1_[A-Za-z0-9_-]+", remove_line).group()
    os.kill(child.pid, signal.SIGUSR1)
    time.sleep(.3)
    inspect("--action", remove_node, "--index", "0")
    time.sleep(1)
    os.kill(child.pid, signal.SIGUSR2)
    pump(child, 1)
    fallback = status(child)
    assert fallback["reader"] is not None, fallback
    assert fallback["reader"]["position"] != stale_position, (target, fallback)
    assert fallback["reader_stale_fallbacks"] == 1, fallback
    print(f"READER_STALE_FALLBACK=PASS old={stale_position} new={fallback['reader']['position']}")
finally:
    if child.isalive():
        child.send(b"q")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)

# Debug-only panic after raw/alternate-screen attach. The panic hook must emit
# restoration escapes and restore the PTY's canonical/echo flags.
panic_child = pexpect.spawn(str(binary / "gui2tui"),
    ["--app", selector, "--test-panic-after-attach"], env=os.environ.copy(),
    encoding=None, dimensions=(30, 100))
panic_child.expect(pexpect.EOF, timeout=10)
capture = panic_child.before
flags = termios.tcgetattr(panic_child.child_fd)[3]
panic_child.close()
assert panic_child.exitstatus != 0
assert flags & termios.ECHO and flags & termios.ICANON, flags
assert b"\x1b[?1049l" in capture, capture[-1000:]
assert b"\x1b[?25h" in capture, capture[-1000:]
print("CONTROLLED_PANIC_TERMINAL_RESTORE=PASS ICANON=true ECHO=true alternate_screen_left=true cursor_visible=true")
