"""Real SIGKILL/stale socket/partial artifact recovery, independent processes.

The isolated TMPDIR contains only this test's brokers. Active leases must
survive the other broker's scavenger. No GUI or synthetic capture claim.
"""
import hashlib
import json
import os
import pathlib
import socket
import struct
import subprocess
import sys
import tempfile
import time

binary = str(pathlib.Path(sys.argv[1]).resolve())


def send(stream, value):
    body = json.dumps(value).encode()
    stream.sendall(struct.pack(">I", len(body)) + body)


def receive(stream):
    def exact(n):
        data = b""
        while len(data) < n:
            part = stream.recv(n - len(data))
            if not part:
                raise EOFError()
            data += part
        return data
    return json.loads(exact(struct.unpack(">I", exact(4))[0]))


with tempfile.TemporaryDirectory(prefix="g4a-") as directory:
    root = pathlib.Path(directory)
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    env = {**os.environ, "TMPDIR": directory, "XDG_RUNTIME_DIR": str(runtime)}
    processes = []

    def start(name, mime):
        process = subprocess.Popen([binary, "serve", "--socket", str(root / name),
            "--mime", mime, "--recording-handler", "--authorization", "once"],
            env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        processes.append(process)
        # A stale socket may preexist, so existence alone is not readiness.
        assert "broker ready" in process.stdout.readline()
        return process

    def connect(name):
        stream = socket.socket(socket.AF_UNIX)
        stream.settimeout(5)
        stream.connect(str(root / name))
        return stream

    try:
        first = start("a", "image/*")
        other = start("b", "video/*")
        owned = runtime / "gui2tui" / f"gui2tui-owned-{os.geteuid()}"
        active_before = set(owned.iterdir())
        assert len(active_before) == 2
        stream = connect("a")
        payload = b"x" * (2 * 1024 * 1024)
        send(stream, {"request": "Artifact", "descriptor": {
            "id": 99, "kind": "Image", "mime": "image/png", "size": len(payload),
            "hash": list(hashlib.sha256(payload).digest()), "display_name": "test", "lifetime": "Session"}})
        assert receive(stream)["status"] == "Approved"
        stream.sendall(payload[:4096])
        deadline = time.monotonic() + 5
        partial = None
        while time.monotonic() < deadline:
            partials = [p for p in owned.glob("operation-*/artifact-*") if p.stat().st_size == 4096]
            if partials:
                partial = partials[0]
                break
            time.sleep(.02)
        assert partial is not None
        abandoned = partial.parent
        first.kill()
        first.wait(timeout=5)
        assert (root / "a").exists(), "expected actual stale socket after SIGKILL"
        assert partial.exists(), "expected actual crash residue"
        stream.close()
        restarted = start("a", "application/pdf")
        assert not abandoned.exists(), "startup did not recover partial artifact"
        still_active = active_before - {abandoned}
        assert all(p.exists() for p in still_active), "another live broker namespace removed"
        with connect("a") as stream:
            send(stream, {"request": "Capabilities"})
            caps = receive(stream)["capabilities"]
            assert caps["mime_patterns"] == ["application/pdf"], caps
        print("BROKER_SIGKILL=PASS partial_bytes=4096 recovered_namespaces=1 stale_socket_rebound=true")
        print("ACTIVE_BROKER_ISOLATION=PASS CAPABILITY_RENEGOTIATION=PASS")
        for process in (other, restarted):
            process.terminate()
            process.wait(timeout=5)
            assert process.returncode == 0, process.stderr.read()
        assert not (root / "a").exists() and not (root / "b").exists()
        print("SIGTERM_SOCKET_ARTIFACT_CLEANUP=PASS")
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
