"""A generated, in-memory portable resource with no URI or filesystem source.

Explicit test producer only; never extracts content from toolkit private APIs.
"""
import hashlib
import json
import socket
import struct
import sys

payload = (b'<svg xmlns="http://www.w3.org/2000/svg" width="480" height="180">'
           b'<rect width="480" height="180" fill="#10243e"/>'
           b'<text x="24" y="95" font-family="sans-serif" font-size="26" fill="white">'
           b'Reference-free artifact</text></svg>')
descriptor = {"request": "Artifact", "descriptor": {
    "id": 99, "kind": "Image", "mime": "image/svg+xml", "size": len(payload),
    "hash": list(hashlib.sha256(payload).digest()),
    "display_name": "In-memory diagram", "lifetime": "Session"}}


def receive(stream):
    def exact(size):
        data = b""
        while len(data) < size:
            part = stream.recv(size - len(data))
            if not part:
                raise EOFError("broker closed response")
            data += part
        return data
    return json.loads(exact(struct.unpack(">I", exact(4))[0]))


with socket.socket(socket.AF_UNIX) as stream:
    stream.settimeout(10)
    stream.connect(sys.argv[1])
    data = json.dumps(descriptor).encode()
    stream.sendall(struct.pack(">I", len(data)) + data)
    assert receive(stream)["status"] == "Approved"
    stream.sendall(payload)
    stream.shutdown(socket.SHUT_WR)
    result = receive(stream)
    assert result["status"] == "Opened" and result["artifact_bytes"] == len(payload)
    print(f"NO_REFERENCE_AVAILABLE=True source=generated-memory bytes={len(payload)} result={result['status']}")
