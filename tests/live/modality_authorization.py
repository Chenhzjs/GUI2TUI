"""Real controlling-terminal authorization; Linux pexpect fixture."""
import pathlib
import socket
import struct
import json
import sys
import tempfile
import pexpect

binary = str(pathlib.Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="g2t-auth-") as directory:
    socket_path = str(pathlib.Path(directory) / "s")
    child = pexpect.spawn(binary, ["serve", "--socket", socket_path,
        "--mime", "image/*", "--recording-handler", "--max-requests", "4"], encoding="utf-8")
    child.expect("broker ready")
    for index, decision in enumerate(["o", "d", "s", None]):
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(5)
            stream.connect(socket_path)
            value = {"request": "Reference", "kind": "Image", "resource": {
                "reference": {"NetworkUri": "https://example.invalid/a.png?token=private-token"},
                "mime": "image/png", "provenance": "HyperlinkUri", "display_name": "test"}}
            body = json.dumps(value).encode()
            stream.sendall(struct.pack(">I", len(body)) + body)
            if decision:
                child.expect("Deny \\(default\\):")
                assert "private-token" not in child.before
                child.sendline(decision)
            length = struct.unpack(">I", stream.recv(4))[0]
            data = b""
            while len(data) < length:
                data += stream.recv(length - len(data))
            result = json.loads(data)
            assert result["status"] == ("Failed" if decision == "d" else "Opened")
            assert result["artifact_bytes"] == 0
            print(f"LOCAL_TTY_AUTH request={index + 1} choice={decision or 'session grant reused'} result={result['status']} bytes=0")
    child.expect(pexpect.EOF, timeout=10)
    child.close()
    assert child.exitstatus == 0
print("INTERACTIVE_LOCAL_AUTHORIZATION_PASS")
