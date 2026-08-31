"""Qt observed Text quarantine belongs to one application generation only."""
import json
import os
import pathlib
import signal
import subprocess
import time

import pexpect
import pyte

binary = pathlib.Path(os.environ["GUI2TUI_BIN"]) if "GUI2TUI_BIN" in os.environ else pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
selector = "gui2tui-qt-rich-text-fixture"
fixture = None


def start_fixture():
    global fixture
    fixture = subprocess.Popen(["python3", "tests/fixtures/qt6_rich_text_fixture.py"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        listing = subprocess.run([str(binary / "gui2tui-inspect"), "--list"], text=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=10).stdout
        if selector in listing:
            return
        time.sleep(.1)
    raise AssertionError("Qt rich fixture unavailable")


screen = pyte.Screen(140, 80)
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
    child.send(b"\x1b"); pump(child, .1)
    return value


start_fixture()
child = pexpect.spawn(str(binary / "gui2tui"), ["--app", selector], env=os.environ.copy(),
                     encoding=None, dimensions=(80, 140))
try:
    pump(child, 2)
    initial = status(child)
    assert initial["text_capabilities"]["declared"] >= 1, initial
    child.send(b"\r")
    pump(child, 2)
    observed = status(child)
    assert observed["text_capabilities"]["quarantined"] >= 1, observed

    if fixture.poll() is None:
        os.killpg(fixture.pid, signal.SIGTERM)
        fixture.wait(timeout=5)
    deadline = time.monotonic() + 8
    while "Application is no longer available" not in pump(child, .2):
        assert time.monotonic() < deadline
    start_fixture()
    child.send(b"\x1b[15~")
    pump(child, 2)
    fresh = status(child)
    assert fresh["generation"] == 2, fresh
    assert fresh["text_capabilities"]["quarantined"] == 0, fresh
    assert fresh["text_capabilities"]["declared"] >= 1, fresh
    print("QT_TEXT_QUARANTINE_GENERATION_RESET=PASS old=Quarantined new=Declared")
finally:
    if child.isalive():
        child.send(b"q")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)
    if fixture is not None and fixture.poll() is None:
        os.killpg(fixture.pid, signal.SIGTERM)
        fixture.wait(timeout=5)
