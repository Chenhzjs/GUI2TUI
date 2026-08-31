"""AT-SPI-only modality discovery and real terminal-to-broker handoff.

Recording handler proves dispatch, not visual rendering or network download.
"""
import os
import pathlib
import re
import signal
import subprocess
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
app = os.environ["APP_SELECTOR"]


def inspect(*args):
    return subprocess.check_output(
        [str(binary / "gui2tui-inspect"), *args], text=True, timeout=60
    )


print(inspect("--list"))
tree = inspect("--app", app, "--verbose")
root.joinpath("tree.txt").write_text(tree)
modalities = inspect("--app", app, "--dump-modalities")
root.joinpath("modalities.txt").write_text(modalities)
print(modalities)
assert "browser-phase-secret" not in tree + modalities

# Phase 3H: a materialization request must keep a trustworthy reference, even
# without any broker. No screenshot/download/endpoint is needed for this path.
for line in tree.splitlines():
    if re.search(r'^\W*Image\b.*atspi1_', line) and "Architecture diagram" in line:
        node = re.search(r'atspi1_[A-Za-z0-9_-]+', line).group()
        reference = inspect("--app", app, "--dump-resource-reference", node)
        if "UNRESOLVED" not in reference:
            headless = inspect("--app", app, "--materialize-modality", node)
            assert "payload_bytes=0" in headless and '"snapshot_attempt":0' in headless
            root.joinpath("headless-reference.txt").write_text(headless)
            print("HEADLESS_REFERENCE=PASS; snapshot_attempt=0; payload_bytes=0")
        break

socket = root / "broker.sock"
viewer = os.environ.get("VIEWER_PROGRAM")
with root.joinpath("broker.log").open("w") as log:
    broker = subprocess.Popen([
        str(binary / "gui2tui-local"), "serve", "--socket", str(socket),
        "--mime", "image/*", "--mime", "application/pdf", "--mime", "video/*",
        "--mime", "model/*", *( ["--handler-program", viewer] if viewer else ["--recording-handler"] ), "--authorization", "once",
    ], stdout=log, stderr=log)
    try:
        for _ in range(100):
            if socket.exists():
                break
            time.sleep(.02)
        child = pexpect.spawn(str(binary / "gui2tui"), ["--app", app,
            "--modality-socket", str(socket)], encoding=None, dimensions=(38, 120))
        screen = pyte.Screen(120, 38)
        stream = pyte.Stream(screen)

        def pump(seconds=.5):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                try:
                    stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
                except pexpect.TIMEOUT:
                    pass
            return "\n".join(screen.display)

        deadline = time.monotonic() + 20
        while "Loaded " not in pump(.15):
            assert time.monotonic() < deadline, "TUI initial frame did not complete"
        child.send(b"\x1bOS")  # xterm F4
        frame = pump(1)
        assert "External modality" in frame, frame
        success = False
        seen = set()
        for _ in range(50):
            selected = next((line for line in screen.display if re.search(r"> (Image|Unknown):", line)), "")
            target = next((name for name in ["Open PDF manual", "Open demo video", "Open portable model"] if name in selected), None)
            if "Architecture diagram" in selected and "> Image:" in selected:
                target = "Image"
            if target and target not in seen:
                seen.add(target)
                root.joinpath("tui-modality.txt").write_text(frame)
                print("RESOURCE_FRAME " + target + "\n" + frame)
                if "[Open locally]" in frame:
                    child.send(b"\r")
                    frame = pump(3 if viewer else 1)
                    root.joinpath("tui-handoff.txt").write_text(frame)
                    print("HANDOFF_FRAME\n" + frame)
                    assert "Local handler accepted resource" in frame, frame
                    success = success or target == "Image"
                    if viewer and target == "Image":
                        root.joinpath("viewer-tree.txt").write_text(inspect("--app", "eog"))
                        if os.environ.get("SCREENSHOT_HELPER"):
                            subprocess.check_call(["python3", os.environ["SCREENSHOT_HELPER"],
                                "--mode", "temp", "--path", str(root / "viewer.png")])
                        break
            if len(seen) == 4 or not modalities.strip():
                break
            child.send(b"\x1b[B")
            frame = pump(.15)
        child.send(b"\x1b")
        frame = pump(.5)
        assert "semantic position preserved" in frame
        child.send(b"\x03")
        child.expect(pexpect.EOF, timeout=10)
        child.close()
        assert child.exitstatus == 0
        print(f"TUI_MODALITY_HANDOFF={success}; ESC_RETURN=True; QUIT=True")
    finally:
        broker.send_signal(signal.SIGINT)
        broker.wait(timeout=10)
print(root.joinpath("broker.log").read_text())
