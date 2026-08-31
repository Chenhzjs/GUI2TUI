"""Final controls regression via real terminal keys and independent AT-SPI reads."""
import os
import pathlib
import subprocess
import time

import pexpect
import pyte

binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
toolkit = os.environ["TEST_TOOLKIT"]
selector = "gui2tui-qt-fixture" if toolkit == "qt" else "gui2tui-live-fixture"
sentinel = "phase-two-secret" if toolkit == "qt" else "phase-zero-secret"
fixture = subprocess.Popen(["python3", f"tests/fixtures/{'qt6' if toolkit == 'qt' else 'gtk4'}_live_fixture.py"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def inspect():
    return subprocess.check_output([str(binary / "gui2tui-inspect"), "--app", selector],
                                   text=True, stderr=subprocess.DEVNULL, timeout=15)

screen = pyte.Screen(140, 70)
stream = pyte.Stream(screen)

def pump(seconds=.3):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    frame = "\n".join(screen.display)
    assert sentinel not in frame
    return frame

def focus(label):
    for _ in range(40):
        frame = pump(.06)
        if any(line.strip("│ ").startswith("> ") and label in line for line in frame.splitlines()):
            return
        child.send(b"\t")
    raise AssertionError(f"focus target unavailable: {label}\n{frame}")

def help_frame(name, required, key=b"\x1bOP"):
    child.send(key)
    frame = pump(.3)
    assert "GUI2TUI Help" in frame and required in frame, frame
    pathlib.Path(os.environ["RESULT_DIR"]).joinpath(f"help-{name}.txt").write_text(frame)
    child.send(b"\x1b")
    pump(.2)

child = None
try:
    time.sleep(2)
    child = pexpect.spawn(str(binary / "gui2tui"), ["--app", selector],
                          env=os.environ.copy(), encoding=None, dimensions=(70, 140))
    pump(2)
    focus("Username")
    child.send(b"\r")
    pump(.2)
    help_frame("edit", "Plain text editing")
    child.send(b"-p4a\r")
    pump(1)
    assert 'value="alice-p4a"' in inspect()
    child.send(b"\r-cancelled\x1b")
    pump(.5)
    assert 'value="alice-p4a"' in inspect() and "cancelled" not in inspect()
    print(f"{toolkit.upper()}_EDIT_COMMIT_CANCEL=PASS")
    focus("Password")
    child.send(b"\r")
    assert "Password editing is disabled" in pump(.5)
    focus("Activate safely")
    child.send(b"\r")
    assert "Status: activated" in pump(1)
    tree = inspect()
    assert 'CheckBox "Enable feature" [checked' in tree
    assert sentinel not in tree
    print(f"{toolkit.upper()}_BUTTON_PASSWORD=PASS")
    if toolkit == "qt":
        focus("Enable feature")
        child.send(b"\r")
        pump(.5)
        assert 'CheckBox "Enable feature" [checked' not in inspect()
        focus("Choice:")
        child.send(b"\r")
        assert "Beta" in pump(.4)
        help_frame("choice", "Choice", b"?")
        child.send(b"\x1b[B\r")
        assert "Choice: Beta" in pump(1)
        choice_tree = inspect()
        pathlib.Path(os.environ["RESULT_DIR"]).joinpath("choice-confirmation.txt").write_text(choice_tree)
        # Qt keeps the owner's cached accessible name "Alpha" while its
        # selected child changes. Selection state, not owner name, is authority.
        combo = choice_tree[choice_tree.index('ComboBox "'):]
        assert 'ListItem "Beta" [selected,transient]' in combo
        print("QT_CHOICE_CHECKBOX=PASS no_gui_popup_required=true")
    else:
        focus("Enable feature")
        child.send(b"\r")
        assert "No compatible" in pump(.5)
        assert 'CheckBox "Enable feature" [checked' in inspect()
        print("GTK_CHECKBOX_READ_ONLY=PASS")
    child.send(b":")
    pump(.2)
    help_frame("commands", "toggle all-scope search")
    child.send(b"\x1b")
    pump(.2)
    print(f"{toolkit.upper()}_CONTEXT_HELP=PASS edit_command_choice_where_available=true")
finally:
    if child is not None and child.isalive():
        child.send(b"\x03")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)
    fixture.terminate()
    fixture.wait(timeout=5)
