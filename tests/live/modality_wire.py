"""Independent-process broker regression, no AT-SPI or viewer required.

Usage: python3 tests/live/modality_wire.py /path/to/gui2tui-local
All handlers are explicitly recording-only. Resources are synthetic descriptors.
"""
import hashlib
import json
import pathlib
import signal
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
    def exact(size):
        value = b""
        while len(value) < size:
            part = stream.recv(size - len(value))
            if not part:
                raise EOFError("incomplete broker response")
            value += part
        return value
    return json.loads(exact(struct.unpack(">I", exact(4))[0]))


def descriptor(payload):
    return {"id": 1, "kind": "Image", "mime": "image/svg+xml", "size": len(payload),
            "hash": list(hashlib.sha256(payload).digest()),
            "display_name": "../../unsafe.sh", "lifetime": "Session"}


with tempfile.TemporaryDirectory(prefix="g2t-wire-") as directory:
    root = pathlib.Path(directory)
    # macOS Unix sockets have a short path limit: TemporaryDirectory's normal
    # platform path is still short enough with this compact prefix/name.
    path = str(root / "s")
    local_image = root / "a.svg"
    local_image.write_bytes(b'<svg xmlns="http://www.w3.org/2000/svg"/>')

    for policy in ["once", "session", "deny"]:
        process = subprocess.Popen([binary, "serve", "--socket", path,
            "--mime", "image/*", "--mime", "application/pdf", "--mime", "video/*", "--mime", "model/*",
            "--recording-handler", "--authorization", policy, "--timeout-secs", "1",
            "--map", f"/srv/shared={root}"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            for _ in range(100):
                if pathlib.Path(path).exists():
                    break
                time.sleep(.02)

            def connect():
                stream = socket.socket(socket.AF_UNIX)
                stream.settimeout(5)
                stream.connect(path)
                return stream

            with connect() as stream:
                send(stream, {"request": "Capabilities"})
                caps = receive(stream)
                assert "executable" not in json.dumps(caps)

            for kind, mime, reference in [
                ("Image", "image/svg+xml", {"NetworkUri": "https://example.invalid/a.svg?token=wire-secret"}),
                ("Document", "application/pdf", {"NetworkUri": "https://example.invalid/a.pdf"}),
                ("Video", "video/mp4", {"NetworkUri": "https://example.invalid/a.mp4"}),
                ("PortableModel", "model/gltf+json", {"NetworkUri": "https://example.invalid/a.gltf"}),
                ("Image", "image/svg+xml", {"MappedPath": {"remote": "/srv/shared/a.svg"}}),
            ]:
                with connect() as stream:
                    send(stream, {"request": "Reference", "kind": kind, "resource": {
                        "reference": reference, "mime": mime, "display_name": "fixture",
                        "provenance": "UserConfiguredMapping"}})
                    result = receive(stream)
                    assert result["artifact_bytes"] == 0
                    assert result["status"] == ("Failed" if policy == "deny" else "Opened")

            payload = local_image.read_bytes()
            with connect() as stream:
                send(stream, {"request": "Artifact", "descriptor": descriptor(payload)})
                result = receive(stream)
                if policy == "deny":
                    assert result["status"] == "Failed" and result["artifact_bytes"] == 0
                else:
                    assert result["status"] == "Approved"
                    stream.sendall(payload)
                    stream.shutdown(socket.SHUT_WR)
                    result = receive(stream)
                    assert result["status"] == "Opened" and result["artifact_bytes"] == len(payload)

            if policy == "once":
                # Three separate failures: bad hash, disconnect midway, and a
                # peer that keeps the stream open without sending payload.
                for case in ["hash", "cancel", "timeout"]:
                    with connect() as stream:
                        send(stream, {"request": "Artifact", "descriptor": descriptor(payload)})
                        assert receive(stream)["status"] == "Approved"
                        if case == "hash":
                            stream.sendall(b"X" * len(payload))
                            stream.shutdown(socket.SHUT_WR)
                        elif case == "cancel":
                            stream.sendall(payload[:4])
                            stream.shutdown(socket.SHUT_WR)
                        start = time.monotonic()
                        result = receive(stream)
                        assert result["status"] == "Failed"
                        assert time.monotonic() - start < 3
                    print(f"WIRE_FAILURE_RECOVERY {case}: PASS bytes={result['artifact_bytes']}")
        finally:
            process.send_signal(signal.SIGINT)
            out, err = process.communicate(timeout=10)
            assert process.returncode == 0, err
            assert "wire-secret" not in out + err
            assert not pathlib.Path(path).exists()
        print(f"WIRE_POLICY {policy}: PASS\n{out.strip()}")
print("INDEPENDENT_PROCESS_WIRE_PASS (recording handler; no viewer rendering claimed)")
