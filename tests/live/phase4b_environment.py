"""Isolated negative diagnostics: real D-Bus without any activation services.
No desktop daemon is stopped. Run with an absolute built/packaged gui2tui path.
"""
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import threading
import time
import pexpect
import pyte

binary = pathlib.Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="gui2tui-p4b-env-") as directory:
    root = pathlib.Path(directory)
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    conf = root / "bus.conf"
    conf.write_text(f'''<busconfig><type>session</type><listen>unix:tmpdir={directory}</listen>
    <policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>''')
    bus = subprocess.Popen(["dbus-daemon", "--nofork", "--print-address=1", "--config-file=" + str(conf)], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    address = bus.stdout.readline().strip()
    env = {**os.environ, "HOME": directory, "XDG_CONFIG_HOME": str(root / "config"), "XDG_RUNTIME_DIR": str(runtime), "DBUS_SESSION_BUS_ADDRESS": address, "TERM": "xterm-256color"}
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)

    def doctor(extra=(), environment=None):
        started = time.monotonic()
        result = subprocess.run([str(binary), *extra, "doctor", "--json"], env=environment or env, capture_output=True, text=True, timeout=7)
        return json.loads(result.stdout), time.monotonic() - started

    child = None
    try:
        report, elapsed = doctor()
        checks = {c["name"]: c for c in report["checks"]}
        assert checks["session-bus"]["level"] == "PASS", checks
        assert checks["accessibility-bus"]["level"] == "FAIL", checks
        assert checks["same-host-endpoint"]["level"] == "WARN", checks
        print(f"DOCTOR_REAL_SESSION_NO_ATSPI=PASS elapsed={elapsed:.3f}s")
        child = pexpect.spawn(str(binary), ["--timeout-ms", "200"], env=env, encoding=None, dimensions=(32, 110))
        screen = pyte.Screen(110, 32)
        stream = pyte.Stream(screen)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try:
                stream.feed(child.read_nonblocking(65536, timeout=.1).decode())
            except pexpect.TIMEOUT:
                pass
        assert "Desktop accessibility service unavailable" in "\n".join(screen.display)
        child.send(b"q")
        child.expect(pexpect.EOF, timeout=3)
        child.close()
        assert child.exitstatus == 0
        print("DEGRADED_FIRST_RUN_SELECTOR=PASS quit_responsive=true")
        runtime.chmod(0o755)
        bad, _ = doctor()
        assert any(c["name"] == "runtime-directory" and c["level"] == "FAIL" for c in bad["checks"])
        runtime.chmod(0o700)
        fallback_env = env.copy()
        fallback_env.pop("XDG_RUNTIME_DIR")
        fallback_env["TMPDIR"] = directory
        fallback, _ = doctor(environment=fallback_env)
        assert any(c["name"] == "runtime-directory" and c["level"] == "PASS" for c in fallback["checks"])
        print("RUNTIME_UNSAFE_REJECTED_PRIVATE_FALLBACK=PASS")
        listener = socket.socket(socket.AF_UNIX)
        path = runtime / "slow.sock"
        listener.bind(str(path))
        listener.listen(1)
        closed = []
        def slow():
            connection, _ = listener.accept()
            with connection:
                def exact(count):
                    data = b""
                    while len(data) < count:
                        part = connection.recv(count - len(data))
                        assert part
                        data += part
                    return data
                exact(int.from_bytes(exact(4), "big"))
                time.sleep(2)
                closed.append(connection.recv(1) == b"")
        peer = threading.Thread(target=slow)
        peer.start()
        slow_report, elapsed = doctor(["--modality-socket", str(path)])
        assert elapsed < 2, elapsed
        assert any(c["name"] == "same-host-endpoint" and c["level"] == "WARN" for c in slow_report["checks"])
        peer.join()
        listener.close()
        assert closed == [True], "deadline did not close the diagnostic connection"
        print(f"DOCTOR_STALLED_ENDPOINT=PASS elapsed={elapsed:.3f}s")
    finally:
        if child is not None and child.isalive():
            child.close(force=True)
        bus.terminate()
        bus.wait(timeout=3)
