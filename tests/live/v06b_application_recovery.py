#!/usr/bin/env python3
"""Live app-switch, duplicate-name, and no-app recovery evidence for v0.6B."""

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
FIXTURE = ROOT / "tests/fixtures/v06b_qt_application_fixture.py"


def wait_for(predicate, timeout: float = 15.0, message: str = "condition"):
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


def start_fixture(name: str, title: str, control: str, state: pathlib.Path):
    return subprocess.Popen(
        [
            "python3",
            str(FIXTURE),
            "--name",
            name,
            "--title",
            title,
            "--control",
            control,
            "--state-file",
            str(state),
        ],
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


def app_list() -> str:
    result = subprocess.run(
        [INSPECT, "--list"],
        env=os.environ,
        text=True,
        capture_output=True,
        check=False,
    )
    return result.stdout


def indices_for(name: str) -> list[int]:
    result = []
    for line in app_list().splitlines():
        match = re.match(r"\s*(\d+)\s+(.*)$", line)
        if match and match.group(2) == name:
            result.append(int(match.group(1)))
    return result


def tree(name: str) -> str:
    return subprocess.check_output(
        [INSPECT, "--app", name, "--verbose"], env=os.environ, text=True
    )


def tree_by_index(index: int) -> str:
    return subprocess.check_output(
        [INSPECT, "--app-id", str(index), "--verbose"],
        env=os.environ,
        text=True,
    )


def root_locator(snapshot: str, name: str) -> str:
    match = re.search(rf'Application "{re.escape(name)}".* id=([^ ]+)', snapshot)
    if not match:
        raise AssertionError(f"missing application locator for {name!r}\n{snapshot}")
    return match.group(1)


def wait_app(name: str, control: str) -> str:
    def current():
        snapshot = tree(name)
        return snapshot if control in snapshot else None

    return wait_for(current, message=f"{name} exposing {control!r}")


def checked(path: pathlib.Path) -> bool:
    return path.exists() and path.read_text(encoding="utf-8").strip() == "checked=true"


def command(child: pexpect.spawn, query: str) -> None:
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode() + b"\r")


def choose(child: pexpect.spawn, query: str, expected: bytes, move: int = 0) -> None:
    child.send(b"b")
    child.expect(b"Select application", timeout=8)
    child.send(b"/")
    child.send(query.encode())
    child.send(b"\r")
    for _ in range(move):
        child.send(b"\x1b[B")
    child.send(b"\r")
    child.expect(expected, timeout=15)


def choose_from_unavailable(
    child: pexpect.spawn, query: str, expected: bytes, refresh: bool = False
) -> None:
    log = RUNTIME / "gui2tui" / "product.log"
    ready = (
        log.read_text(encoding="utf-8").count("fresh application selector ready")
        if log.exists()
        else 0
    )
    child.send(b"b")
    wait_for(
        lambda: log.exists()
        and log.read_text(encoding="utf-8").count(
            "fresh application selector ready"
        )
        > ready,
        message="fresh application selector ready",
    )
    if refresh:
        child.send(b"r")
    child.send(b"/")
    child.send(query.encode())
    child.send(b"\r\r")
    child.expect(expected, timeout=15)


processes = []
child = None
try:
    state_a = RUNTIME / "v06b-a.state"
    state_b = RUNTIME / "v06b-b.state"
    state_d1 = RUNTIME / "v06b-d1.state"
    state_d2 = RUNTIME / "v06b-d2.state"
    state_fresh = RUNTIME / "v06b-fresh.state"
    app_a = "gui2tui-v06b-app-a"
    app_b = "gui2tui-v06b-app-b"
    duplicate = "gui2tui-v06b-duplicate"
    fresh_name = "gui2tui-v06b-fresh"

    proc_a = start_fixture(app_a, "Application A", "A control", state_a)
    proc_b = start_fixture(app_b, "Application B", "B control", state_b)
    processes.extend([proc_a, proc_b])
    first_a = wait_app(app_a, "A control")
    wait_app(app_b, "B control")
    old_a_locator = root_locator(first_a, app_a)

    child = pexpect.spawn(
        GUI2TUI,
        [
            "--app",
            app_a,
            "--layout",
            "flat",
            "--settle-ms",
            "1000",
            "--timeout-ms",
            "800",
            "--no-mouse",
            "--log-level",
            "debug",
        ],
        env=os.environ.copy(),
        encoding=None,
        dimensions=(42, 180),
    )
    child.expect(b"A control", timeout=15)

    choose(child, app_b, b"B control")
    assert not checked(state_a)
    command(child, "B control")
    wait_for(lambda: checked(state_b), message="fresh B operation")
    assert not checked(state_a)
    log = RUNTIME / "gui2tui" / "product.log"
    wait_for(
        lambda: log.exists()
        and "user selected fresh application generation" in log.read_text(encoding="utf-8"),
        message="A to B generation replacement",
    )

    choose(child, app_a, b"A control")
    second_a = wait_app(app_a, "A control")
    assert root_locator(second_a, app_a) == old_a_locator
    command(child, "A control")
    wait_for(lambda: checked(state_a), message="fresh A operation after switch back")
    assert checked(state_b)
    generation_lines = [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "user selected fresh application generation" in line
    ]
    assert any('"generation":2' in line for line in generation_lines), generation_lines
    assert any('"generation":3' in line for line in generation_lines), generation_lines
    print("APP_SWITCH_OLD_BINDING_REFUSAL=PASS")
    print("APP_SWITCH_FRESH_BINDING_RECOVERY=PASS")

    proc_d1 = start_fixture(duplicate, "Duplicate Window", "Shared control", state_d1)
    proc_d2 = start_fixture(duplicate, "Duplicate Window", "Shared control", state_d2)
    processes.extend([proc_d1, proc_d2])
    duplicate_indices = wait_for(
        lambda: (found if len(found := indices_for(duplicate)) == 2 else None),
        message="two duplicate-name applications",
    )
    duplicate_locators = {
        root_locator(tree_by_index(index), duplicate) for index in duplicate_indices
    }
    assert len(duplicate_locators) == 2

    choose(child, duplicate, b"Shared control")
    command(child, "Shared control")
    wait_for(
        lambda: checked(state_d1) ^ checked(state_d2),
        message="one exact duplicate selected",
    )
    if checked(state_d1):
        selected_proc, surviving_proc = proc_d1, proc_d2
        selected_state, surviving_state = state_d1, state_d2
    else:
        selected_proc, surviving_proc = proc_d2, proc_d1
        selected_state, surviving_state = state_d2, state_d1
    assert checked(selected_state) and not checked(surviving_state)
    invalidations = log.read_text(encoding="utf-8").count(
        "application generation invalidated"
    )
    stop(selected_proc)
    wait_for(
        lambda: log.read_text(encoding="utf-8").count(
            "application generation invalidated"
        )
        > invalidations,
        message="selected duplicate generation invalidation",
    )
    child.expect(b"Application is no longer available", timeout=12)
    assert not checked(surviving_state)

    choose_from_unavailable(child, duplicate, b"Shared control")
    command(child, "Shared control")
    wait_for(lambda: checked(surviving_state), message="explicit duplicate recovery")
    assert surviving_proc.poll() is None
    print("DUPLICATE_NAME_RECOVERY_NO_GUESS=PASS")

    stop(surviving_proc)
    stop(proc_a)
    stop(proc_b)
    child.expect(b"Application is no longer available", timeout=12)
    child.send(b" ")
    assert child.isalive()
    assert indices_for(duplicate) == []
    assert indices_for(app_a) == []
    assert indices_for(app_b) == []
    ready = log.read_text(encoding="utf-8").count("fresh application selector ready")
    child.send(b"b")
    wait_for(
        lambda: log.read_text(encoding="utf-8").count(
            "fresh application selector ready"
        )
        > ready,
        message="empty application selector ready",
    )
    assert any(
        "fresh application selector ready" in line and "applications=0" in line
        for line in log.read_text(encoding="utf-8").splitlines()
    )

    proc_fresh = start_fixture(
        fresh_name, "Fresh Application", "Fresh control", state_fresh
    )
    processes.append(proc_fresh)
    wait_app(fresh_name, "Fresh control")
    child.send(b"r")
    child.send(b"/")
    child.send(fresh_name.encode())
    child.send(b"\r\r")
    child.expect(b"Fresh control", timeout=15)
    command(child, "Fresh control")
    wait_for(lambda: checked(state_fresh), message="no-app fresh selection operation")
    print("NO_APP_STATE_SAFE=PASS")
    print("NO_APP_FRESH_SELECTION_RECOVERY=PASS")
    print("RECOVERED_FRESH_OPERATION_USABLE=PASS")

    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)
    child = None
except BaseException:
    log = RUNTIME / "gui2tui" / "product.log"
    if log.exists():
        print("--- v0.6B product log tail ---")
        print(log.read_text(encoding="utf-8")[-16000:])
    print("--- applications ---")
    try:
        print(app_list())
    except BaseException:
        pass
    raise
finally:
    if child is not None and child.isalive():
        child.close(force=True)
    for process in reversed(processes):
        stop(process)
