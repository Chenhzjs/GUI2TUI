"""Reader/Table help live test using the existing isolated browser fixture."""
import os
import pathlib
import time
import pexpect
import pyte

binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug/gui2tui"
root = pathlib.Path(os.environ["RESULT_DIR"])
child = pexpect.spawn(str(binary), ["--app", "Google Chrome"], encoding=None, dimensions=(38, 130))
screen = pyte.Screen(130, 38)
stream = pyte.Stream(screen)
def pump(seconds=.4):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    text = "\n".join(screen.display)
    assert "browser-phase-secret" not in text
    return text
try:
    deadline = time.monotonic() + 10
    while "Loaded " not in pump(.15):
        assert time.monotonic() < deadline
    child.send(b"\r")
    assert "Reader" in pump(1)
    child.send(b"?")
    frame = pump()
    assert "GUI2TUI Help" in frame and "move semantic blocks" in frame
    root.joinpath("help-reader.txt").write_text(frame)
    child.send(b"\x1b")
    pump(.2)
    child.send(b"/Evaluation scores")
    pump(.5)
    child.send(b"\x1bOP")
    frame = pump()
    assert "Reader search" in frame and "Ctrl-F" in frame
    root.joinpath("help-search.txt").write_text(frame)
    child.send(b"\x1b")
    pump(.2)
    child.send(b"\r")
    pump(.5)
    child.send(b"\r")
    assert "Table" in pump(.5)
    child.send(b"?")
    frame = pump()
    assert "GUI2TUI Help" in frame and "semantic row/column" in frame
    root.joinpath("help-table.txt").write_text(frame)
    child.send(b"\x1b")
    pump(.2)
    child.send(b"l")
    assert "Table" in pump(.3)
    print("READER_SEARCH_TABLE_CONTEXT_HELP=PASS underlying_navigation_preserved=true")
finally:
    child.send(b"\x03")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
