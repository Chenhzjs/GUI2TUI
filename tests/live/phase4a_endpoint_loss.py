"""Kill a same-host endpoint during a real TUI materialized-artifact handoff."""
import json
import os
import pathlib
import signal
import subprocess
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
selector = "python3"
socket = root / "broker.sock"
handler = root / "slow-handler.sh"
handler.write_text("#!/bin/sh\nprintf started >\"$GUI2TUI_HANDLER_MARKER\"\nsleep 30\n")
handler.chmod(0o700)
marker = root / "handler-started"
broker_env = {**os.environ, "TMPDIR": str(root), "GUI2TUI_HANDLER_MARKER": str(marker)}
brokers = []


def start_broker(mime, slow):
    command = [str(binary / "gui2tui-local"), "serve", "--socket", str(socket),
               "--mime", mime, "--authorization", "once"]
    command += ["--handler-program", str(handler)] if slow else ["--recording-handler"]
    process = subprocess.Popen(command, env=broker_env, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, text=True)
    brokers.append(process)
    lines = []
    for _ in range(4):
        line = process.stdout.readline()
        lines.append(line)
        if "broker ready" in line:
            break
    else:
        raise AssertionError("broker did not become ready: " + "".join(lines))
    return process


screen = pyte.Screen(140, 80)
stream = pyte.Stream(screen)


def pump(child, seconds=.5):
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


transfer = os.environ.get("TEST_PAYLOAD_TRANSFER") == "1"
progress = root / "producer-progress"
child_env = os.environ.copy()
if transfer:
    child_env["GUI2TUI_TEST_TRANSFER_PROGRESS"] = str(progress)
first = start_broker("image/*", not transfer)
child = pexpect.spawn(str(binary / "gui2tui"),
    ["--app", selector, "--modality-socket", str(socket)], env=child_env,
    encoding=None, dimensions=(80, 140))
try:
    pump(child, 2)
    child.send(b"\x1bOS")  # F4
    assert "External modality" in pump(child, 1)
    child.send(b"m")
    frame = pump(child, 3)
    assert "RenderedSnapshot (may be occluded)" in frame, frame
    before = status(child)
    child.send(b"o")
    deadline = time.monotonic() + 5
    def started():
        if not transfer:
            return marker.exists()
        try:
            return int(progress.read_text()) >= 64
        except (OSError, ValueError):
            return False
    while not started() and time.monotonic() < deadline:
        pump(child, .03)
    assert started(), "handoff did not start"
    first.kill()
    first.wait(timeout=5)
    frame = pump(child, 2)
    lost = status(child)
    assert child.isalive()
    assert lost["endpoint"] == "Disconnected", lost
    assert lost["active_operations"] == 0, lost
    assert lost["focused_runtime"] == before["focused_runtime"], (before, lost)
    assert "viewer accepted" not in frame.lower(), frame
    assert any(word in frame.lower() for word in ["unavailable", "no pixels transported", "endpointlost"]), frame
    if transfer:
        stopped_bytes = progress.read_text()
        pump(child, .5)
        assert progress.read_text() == stopped_bytes, "producer continued reading after endpoint loss"
        print(f"TUI_ENDPOINT_MID_TRANSFER_LOSS=PASS payload_read={stopped_bytes} producer_stopped=true")
    print("TUI_ENDPOINT_MID_OPERATION_LOSS=PASS responsive=true false_opened=false semantic_scene_valid=true")

    marker.unlink(missing_ok=True)
    restarted = start_broker("image/png", False)
    child.send(b"\x1b")
    pump(child, .2)
    child.send(b"\x1bOS")
    assert "External modality" in pump(child, 1)
    child.send(b"m")
    assert "RenderedSnapshot (may be occluded)" in pump(child, 3)
    child.send(b"o")
    success_frame = pump(child, 15 if transfer else 2)
    success = status(child)
    assert "viewer accepted" in success_frame.lower(), success_frame
    assert success["metrics"]["endpoint_reconnects"] >= 1, success
    assert success["metrics"]["rejected_late_results"] == 0, success
    print("ENDPOINT_RESTART=PASS capabilities=image/png old_grant_reused=false new_handoff=true")
finally:
    if child.isalive():
        child.send(b"\x03")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)
    for broker in brokers:
        if broker.poll() is None:
            broker.send_signal(signal.SIGTERM)
            broker.wait(timeout=5)
