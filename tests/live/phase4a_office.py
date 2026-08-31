"""Safe command and GUI-exit regression in the isolated LibreOffice fixture."""
import os
import pathlib
import re
import signal
import subprocess
import time

import pexpect
import pyte

binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
screen = pyte.Screen(140, 70)
stream = pyte.Stream(screen)
child = pexpect.spawn(str(binary / "gui2tui"), ["--app", "soffice"],
                      env=os.environ.copy(), encoding=None, dimensions=(70, 140))
def pump(seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)

def inspect(*args):
    return subprocess.check_output([str(binary / "gui2tui-inspect"), *args],
                                   text=True, stderr=subprocess.DEVNULL, timeout=20)
try:
    pump(4)
    commands = inspect("--app", "soffice", "--dump-commands", "--command-query", "About LibreOffice")
    assert "About LibreOffice" in commands
    child.send(b":About LibreOffice\r")
    frame = pump(4)
    tree = inspect("--app", "soffice")
    assert 'Dialog "About LibreOffice"' in tree, frame
    close = next(line for line in tree.splitlines() if 'Button "Close"' in line)
    locator = re.search(r"atspi1_[A-Za-z0-9_-]+", close).group()
    inspect("--activate", locator)
    pump(3)
    assert 'Dialog "About LibreOffice"' not in inspect("--app", "soffice")
    print("LIBREOFFICE_TUI_COMMAND_ABOUT_CLOSE=PASS")
    os.kill(int(os.environ["APP_PID"]), signal.SIGTERM)
    assert "Application is no longer available" in pump(5)
    assert child.isalive()
    print("LIBREOFFICE_EXIT_HANDLING=PASS")
finally:
    if child.isalive():
        child.send(b"\x03")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)
