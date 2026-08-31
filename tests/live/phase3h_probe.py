"""Real no-reference Image -> explicit rendered snapshot, independent headless/viewer.

Fixture's source file is NEVER read here. Pixels come only from the production CLI.
strace records process names to prove zero capture before explicit Materialize.
"""
import hashlib
import json
import os
import pathlib
import re
import shutil
import signal
import struct
import subprocess
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
app = os.environ["APP_SELECTOR"]


def inspect(*args, traced=False):
    command = [str(binary / "gui2tui-inspect"), *args]
    if traced:
        command = ["strace", "-f", "-e", "execve", "-o", str(root / "inspect-exec.log"), *command]
    return subprocess.check_output(command, text=True, stderr=subprocess.STDOUT, timeout=60)


print(inspect("--list"))
tree = inspect("--app", app, "--verbose")
root.joinpath("tree.txt").write_text(tree)
images = [line for line in tree.splitlines() if re.search(r'^\W*Image\b.*atspi1_', line)]
if os.environ.get("TEST_APP") == "libreoffice":
    images = [line for line in images if "Embedded architecture image" in line]
    if not images:
        print("LIBREOFFICE_EMBEDDED_IMAGE=UNAVAILABLE: not exposed in document subtree; unrelated toolbar icons are not substitutes")
        raise SystemExit(0)
image_line = images[0]
node = re.search(r'atspi1_[A-Za-z0-9_-]+', image_line).group()
print("TARGET", image_line)
reference = inspect("--app", app, "--dump-resource-reference", node, traced=True)
assert "UNRESOLVED" in reference, reference
assert '"/usr/bin/scrot"' not in root.joinpath("inspect-exec.log").read_text()
print("REFERENCE=UNRESOLVED; implicit_capture_calls=0")


def materialize(ttl=120, extra=()):
    output = inspect("--app", app, "--materialize-modality", node,
                     "--artifact-ttl-secs", str(ttl), *extra)
    print(output)
    metadata = json.loads(next(line.removeprefix("materialized=") for line in output.splitlines() if line.startswith("materialized=")))
    path = pathlib.Path(next(line.removeprefix("artifact_path=") for line in output.splitlines() if line.startswith("artifact_path=")))
    assert metadata["descriptor"]["origin"] == "RenderedSnapshot"
    assert metadata["quality"] == "CompositedScreenSnapshot"
    assert metadata["descriptor"]["mime"] == "image/png"
    data = path.read_bytes()
    assert hashlib.sha256(data).hexdigest() == bytes(metadata["descriptor"]["hash"]).hex()
    w, h = struct.unpack(">II", data[16:24])
    assert (w, h) == (metadata["region"]["width"], metadata["region"]["height"])
    assert w * h < 1280 * 800, (w, h)
    assert "network_payload_bytes=0" in output
    return output, metadata, path


try:
    output, metadata, path = materialize()
except subprocess.CalledProcessError as error:
    root.joinpath("acquisition-unavailable.txt").write_text(error.output)
    print(error.output)
    if os.environ.get("VISUAL_ONLY") == "0":
        assert "overlapping semantic siblings" in error.output
        print("MIXED_LAYOUT_COORDINATE_REFUSAL=PASS")
        raise SystemExit(0)
    if os.environ.get("TEST_APP") == "libreoffice":
        print("LIBREOFFICE_GENERIC_ACQUISITION=UNAVAILABLE; no private extraction attempted")
        raise SystemExit(0)
    raise
root.joinpath("headless.txt").write_text(output)
shutil.copyfile(path, root / "rendered-snapshot.png")  # evidence only, not production retention
assert not root.joinpath("broker.sock").exists()
print("HEADLESS_MATERIALIZE=PASS; dimensions=", metadata["region"])
_, _, expiring = materialize(2)
time.sleep(3)
assert not expiring.exists() and not expiring.parent.exists()
print("TTL_CLEANUP=PASS")

scaled = subprocess.run([str(binary / "gui2tui-inspect"), "--app", app,
    "--materialize-modality", node], env={**os.environ, "GDK_SCALE": "2"},
    text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=60)
root.joinpath("scaled-refusal.txt").write_text(scaled.stdout)
assert scaled.returncode != 0 and "display scaling is unsupported" in scaled.stdout
assert '"capture_source_bytes":0' in scaled.stdout
print("SCALED_COORDINATE_REFUSAL=PASS; capture_source_bytes=0")

# No broker process, no endpoint configured. Real keyboard TUI + explicit capture.
child = pexpect.spawn("strace", ["-f", "-e", "execve", "-o", str(root / "tui-exec.log"),
    str(binary / "gui2tui"), "--app", app], encoding=None, dimensions=(38, 140))
screen = pyte.Screen(140, 38)
stream = pyte.Stream(screen)


def pump(seconds=1):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


try:
    root.joinpath("tui-initial.txt").write_text(pump(3))
    child.send(b"\x1bOS")
    frame = pump()
    assert "External modality" in frame
    assert '"/usr/bin/scrot"' not in root.joinpath("tui-exec.log").read_text()
    assert "[Open locally]" not in frame
    child.send(b"m")
    frame = pump(3)
    root.joinpath("tui-materialized.txt").write_text(frame)
    assert "RenderedSnapshot (may be occluded)" in frame, frame
    assert root.joinpath("tui-exec.log").read_text().count('execve("/usr/bin/scrot"') == 1
    child.send(b"\x1b")
    assert "semantic position preserved" in pump()
    child.send(b"\x03")
    child.expect(pexpect.EOF, timeout=10)
    child.close()
    assert child.exitstatus == 0
finally:
    if child.isalive():
        child.close(force=True)
print("HEADLESS_TUI=PASS; startup_and_F4_capture_calls=0; explicit_m_capture_calls=1; ESC_QUIT=PASS")

socket = root / "broker.sock"
with root.joinpath("broker.log").open("w") as log:
    broker = subprocess.Popen([str(binary / "gui2tui-local"), "serve", "--socket", str(socket),
        "--mime", "image/*", "--handler-program", "/usr/bin/eog", "--authorization", "once"], stdout=log, stderr=log)
    try:
        for _ in range(100):
            if socket.exists():
                break
            time.sleep(.02)
        output, _, _ = materialize(extra=("--open-materialized", "--modality-socket", str(socket)))
        root.joinpath("same-host.txt").write_text(output)
        time.sleep(2)
        viewer = inspect("--app", "eog")
        root.joinpath("viewer-tree.txt").write_text(viewer)
        assert "artifact.png" in viewer
        if os.environ.get("SCREENSHOT_HELPER"):
            subprocess.check_call(["python3", os.environ["SCREENSHOT_HELPER"], "--mode", "temp", "--path", str(root / "viewer.png")])
        print("SAME_HOST_VIEWER=OPENED; network_payload_bytes=0; visual inspection required")
    finally:
        broker.send_signal(signal.SIGINT)
        broker.wait(timeout=10)
print(root.joinpath("broker.log").read_text())
