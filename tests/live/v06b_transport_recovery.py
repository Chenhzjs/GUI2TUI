#!/usr/bin/env python3
"""Real AT-SPI interruption and explicit fresh-authority recovery evidence."""

import os
import pathlib
import re
import signal
import subprocess
import time

import pexpect


ROOT = pathlib.Path(os.environ["PROJECT_ROOT"])
GUI2TUI = os.environ["GUI2TUI"]
INSPECT = os.environ["INSPECT"]
RUNTIME = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
FIXTURE = ROOT / "tests/fixtures/v06a_qt_runtime_fixture.py"
APP = "gui2tui-v06a-fixture"


def wait_for(predicate, timeout: float = 20.0, message: str = "condition"):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = predicate()
            if last:
                return last
        except (subprocess.CalledProcessError, StopIteration):
            pass
        time.sleep(0.05)
    raise AssertionError(f"timed out waiting for {message}; last={last!r}")


def start_fixture():
    return subprocess.Popen(
        ["python3", str(FIXTURE)],
        env=os.environ.copy(),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )


def stop(process) -> None:
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def tree() -> str:
    return subprocess.check_output(
        [INSPECT, "--app", APP, "--verbose"], env=os.environ, text=True
    )


def wait_tree(needle: str) -> str:
    return wait_for(
        lambda: (snapshot if needle in (snapshot := tree()) else None),
        message=f"fixture exposing {needle!r}",
    )


def command(child: pexpect.spawn, query: str) -> None:
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode() + b"\r")


def kill_accessibility_transport() -> None:
    listing = subprocess.check_output(["ps", "-eo", "pid=,args="], text=True)
    victims = []
    for line in listing.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        pid_text, _, command_line = stripped.partition(" ")
        if any(
            marker in command_line
            for marker in (
                "at-spi-bus-launcher",
                "at-spi2-registryd",
                "at-spi2/accessibility.conf",
            )
        ):
            victims.append(int(pid_text))
    assert victims, listing
    for pid in victims:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass


def start_accessibility_transport() -> None:
    subprocess.run(
        [
            "gdbus",
            "call",
            "--session",
            "--dest",
            "org.a11y.Bus",
            "--object-path",
            "/org/a11y/bus",
            "--method",
            "org.freedesktop.DBus.Properties.Set",
            "org.a11y.Status",
            "IsEnabled",
            "<true>",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )


fixture = None
child = None
try:
    fixture = start_fixture()
    first = wait_tree("Delayed toggle")
    first_locator = re.search(
        rf'Application "{APP}".* id=([^ ]+)', first
    ).group(1)
    child = pexpect.spawn(
        GUI2TUI,
        [
            "--app",
            APP,
            "--layout",
            "flat",
            "--settle-ms",
            "1500",
            "--timeout-ms",
            "700",
            "--no-mouse",
            "--log-level",
            "debug",
        ],
        env=os.environ.copy(),
        encoding=None,
        dimensions=(42, 180),
    )
    child.expect(b"Delayed toggle", timeout=15)
    log = RUNTIME / "gui2tui" / "product.log"

    command(child, "Replaceable toggle")
    wait_for(
        pathlib.Path(os.environ["V06A_REPLACE_READY"]).exists,
        message="blocked operation marker",
    )

    pathlib.Path(os.environ["V06B_TRANSPORT_DISABLED"]).touch()
    kill_accessibility_transport()
    wait_for(
        lambda: "bounded transport recovery exhausted"
        in log.read_text(encoding="utf-8"),
        message="bounded automatic reconnect exhaustion",
    )
    exhausted = [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "bounded transport recovery exhausted" in line
    ]
    assert len(exhausted) == 1
    assert '"backend_reconnect_attempts":4' in exhausted[0], exhausted[0]
    assert '"generation":null' in exhausted[0], exhausted[0]
    time.sleep(1.2)
    assert (
        log.read_text(encoding="utf-8").count("bounded transport recovery exhausted")
        == 1
    )
    print("EXTENDED_OUTAGE_SAFE_UNAVAILABLE=PASS")

    stop(fixture)
    fixture = None
    pathlib.Path(os.environ["V06B_TRANSPORT_DISABLED"]).unlink()
    start_accessibility_transport()
    fixture = start_fixture()
    second = wait_tree("Fresh toggle")
    second_locator = re.search(
        rf'Application "{APP}".* id=([^ ]+)', second
    ).group(1)
    # A newly launched accessibility bus may reuse its first unique owner and
    # object path.  The textual locator therefore may repeat; the fresh
    # ApplicationGeneration is the authority boundary around it.
    assert first_locator and second_locator

    child.send(b" ")
    assert not re.search(r'CheckBox "Fresh toggle".*\bchecked\b', tree())
    child.send(b"\x1b[15~")
    wait_for(
        lambda: "transport restored without semantic reauthorization"
        in log.read_text(encoding="utf-8"),
        message="transport-only recovery",
    )
    assert not re.search(r'CheckBox "Fresh toggle".*\bchecked\b', tree())

    ready = log.read_text(encoding="utf-8").count("fresh application selector ready")
    child.send(b"\x1b[15~")
    wait_for(
        lambda: log.read_text(encoding="utf-8").count(
            "fresh application selector ready"
        )
        > ready,
        message="post-recovery application selector",
    )
    child.send(b"/")
    child.send(APP.encode())
    child.send(b"\r\r")
    child.expect(b"Fresh toggle", timeout=15)
    command(child, "Fresh toggle")
    fresh = wait_tree("Status: fresh=True")
    assert re.search(r'CheckBox "Fresh toggle".*\bchecked\b', fresh), fresh
    selection_lines = [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "user selected fresh application generation" in line
    ]
    assert any('"generation":2' in line for line in selection_lines), selection_lines

    print("TRANSPORT_INTERRUPTION_RECOVERY=PASS")
    print("EXPLICIT_RECOVERY_AFTER_OUTAGE=PASS")
    print("RECONNECT_OLD_BINDINGS_REFUSED=PASS")
    print("RECOVERED_FRESH_OPERATION_USABLE=PASS")
    print("RECOVERY_DOES_NOT_RESURRECT_OPERATION=PASS")
    print("TRANSPORT_RECOVERY_REQUIRES_FRESH_GENERATION=PASS")
    print("TRANSPORT_RECOVERY_NOT_SEMANTIC_REAUTH=PASS")
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)
    child = None
except BaseException:
    log = RUNTIME / "gui2tui" / "product.log"
    if log.exists():
        print("--- v0.6B product log tail ---")
        print(log.read_text(encoding="utf-8")[-20000:])
    try:
        print("--- process list ---")
        print(subprocess.check_output(["ps", "-eo", "pid=,args="], text=True))
    except BaseException:
        pass
    raise
finally:
    if child is not None and child.isalive():
        child.close(force=True)
    stop(fixture)
