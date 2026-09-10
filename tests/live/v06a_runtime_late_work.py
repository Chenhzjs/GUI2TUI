#!/usr/bin/env python3
"""Bounded v0.6A operation/restart/locator/external-text live evidence."""

import os
import pathlib
import re
import signal
import stat
import subprocess
import tempfile
import time

import pexpect


ROOT = pathlib.Path(os.environ["PROJECT_ROOT"])
GUI2TUI = os.environ["GUI2TUI"]
INSPECT = os.environ["INSPECT"]
RUNTIME = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
QT_APP = "gui2tui-v06a-fixture"
GTK_APP = "gui2tui-live-fixture"
QT_FIXTURE = ROOT / "tests/fixtures/v06a_qt_runtime_fixture.py"
GTK_FIXTURE = ROOT / "tests/fixtures/gtk4_live_fixture.py"


def wait_for(predicate, timeout: float = 12.0, message: str = "condition"):
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


def tree(application: str, *extra: str) -> str:
    return subprocess.check_output(
        [INSPECT, "--app", application, *extra], env=os.environ, text=True
    )


def node_id(snapshot: str, role: str, label: str) -> str:
    match = re.search(rf'{role} "{re.escape(label)}".* id=([^ ]+)', snapshot)
    if not match:
        raise AssertionError(f"missing {role} {label!r}\n{snapshot}")
    return match.group(1)


def start_fixture(path: pathlib.Path) -> subprocess.Popen:
    return subprocess.Popen(
        ["python3", str(path)],
        env=os.environ.copy(),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )


def stop(process: subprocess.Popen | None) -> None:
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def tui(application: str, config_home: pathlib.Path | None = None) -> pexpect.spawn:
    env = os.environ.copy()
    if config_home is not None:
        env["XDG_CONFIG_HOME"] = str(config_home)
    arguments = [
        "--app",
        application,
        "--settle-ms",
        "1500",
        "--timeout-ms",
        "1000",
        "--no-mouse",
        "--log-level",
        "debug",
    ]
    if application == QT_APP:
        arguments.extend(["--layout", "flat"])
    child = pexpect.spawn(
        GUI2TUI,
        arguments,
        env=env,
        encoding=None,
        dimensions=(42, 180),
    )
    expected = b"Delayed toggle" if application == QT_APP else b"Edit externally"
    child.expect(expected, timeout=15)
    return child


def command(child: pexpect.spawn, query: str) -> None:
    while True:
        try:
            child.read_nonblocking(size=4096, timeout=0)
        except pexpect.TIMEOUT:
            break
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode() + b"\r")


def wait_marker(path: pathlib.Path) -> None:
    wait_for(path.exists, message=str(path))


def wait_tree(application: str, needle: str) -> str:
    def current():
        snapshot = tree(application, "--verbose")
        return snapshot if needle in snapshot else None

    return wait_for(current, message=f"{application} exposing {needle!r}")


def choose_fresh_application(
    child: pexpect.spawn, application: str, product_log: pathlib.Path, message: str
) -> None:
    ready = product_log.read_text(encoding="utf-8").count(
        "fresh application selector ready"
    )
    selected = product_log.read_text(encoding="utf-8").count(
        "user selected fresh application generation"
    )
    child.send(b"\x1b[15~")
    wait_for(
        lambda: product_log.read_text(encoding="utf-8").count(
            "fresh application selector ready"
        )
        > ready,
        timeout=15,
        message=f"{message} selector",
    )
    child.send(b"/")
    child.send(application.encode())
    child.send(b"\r\r")
    wait_for(
        lambda: product_log.read_text(encoding="utf-8").count(
            "user selected fresh application generation"
        )
        > selected,
        timeout=15,
        message=message,
    )


qt_process = None
qt_tui = None
gtk_process = None
gtk_tui = None
try:
    qt_process = start_fixture(QT_FIXTURE)
    first = wait_tree(QT_APP, "Delayed toggle")
    first_app_locator = node_id(first, "Application", QT_APP)
    qt_tui = tui(QT_APP)
    log = RUNTIME / "gui2tui" / "product.log"

    command(qt_tui, "Delayed toggle")
    wait_marker(pathlib.Path(os.environ["V06A_DELAY_READY"]))
    wait_for(
        lambda: log.exists()
        and "semantic transition event batch settled without confirmation"
        in log.read_text(encoding="utf-8"),
        message="operation observer quiescent bounded wait",
    )
    stop(qt_process)
    qt_process = start_fixture(QT_FIXTURE)
    second = wait_tree(QT_APP, "Status: initial")
    second_app_locator = node_id(second, "Application", QT_APP)
    assert first_app_locator != second_app_locator
    assert "Status: delayed returned" not in second
    assert not re.search(r'CheckBox "Fresh toggle".*\bchecked\b', second)

    wait_for(
        lambda: "application generation invalidated"
        in log.read_text(encoding="utf-8"),
        timeout=35,
        message="G1 generation invalidation",
    )
    invalidation = next(
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "application generation invalidated" in line
    )
    assert '"active_operations":0' in invalidation, invalidation
    choose_fresh_application(qt_tui, QT_APP, log, "fresh G2 binding")
    command(qt_tui, "Fresh toggle")
    fresh = wait_tree(QT_APP, "Status: fresh=True")
    assert re.search(r'CheckBox "Fresh toggle".*\bchecked\b', fresh), fresh

    print("LATE_OPERATION_GENERATION_ISOLATION=PASS")
    print("OPERATION_APP_EXIT_LATE_COMPLETION_ISOLATED=PASS")
    print("FRESH_GENERATION_AFTER_LATE_WORK_USABLE=PASS")
    print("GENERATION_INVALIDATION_RETIRES_ACTIVE_TICKETS=PASS")

    before_replace = tree(QT_APP, "--verbose")
    old_target = node_id(before_replace, "CheckBox", "Replaceable toggle")
    command(qt_tui, "Replaceable toggle")
    wait_marker(pathlib.Path(os.environ["V06A_REPLACE_READY"]))
    def current_replacement():
        snapshot = tree(QT_APP, "--verbose")
        return (
            snapshot
            if node_id(snapshot, "CheckBox", "Replaceable toggle") != old_target
            else None
        )

    replaced = wait_for(current_replacement, message="replacement locator")
    new_target = node_id(replaced, "CheckBox", "Replaceable toggle")
    assert new_target != old_target
    assert not re.search(r'CheckBox "Replaceable toggle".*\bchecked\b', replaced)
    pathlib.Path(os.environ["V06A_REPLACE_RESUME"]).touch()
    wait_for(
        lambda: "semantic transition observation completed outcome=Stale"
        in log.read_text(encoding="utf-8"),
        message="old exact-locator operation retirement",
    )
    refreshes = log.read_text(encoding="utf-8").count(
        "full semantic refresh completed"
    )
    qt_tui.send(b"r")  # fresh semantic snapshot creates the new exact binding
    wait_for(
        lambda: log.read_text(encoding="utf-8").count(
            "full semantic refresh completed"
        )
        > refreshes,
        message="fresh replacement binding",
    )
    command(qt_tui, "Replaceable toggle")
    current = wait_tree(QT_APP, "Status: replacement=True")
    assert re.search(r'CheckBox "Replaceable toggle".*\bchecked\b', current)
    print("LATE_TARGET_REPLACEMENT_LOCATOR_ISOLATION=PASS")

    qt_tui.send(b"q")
    qt_tui.expect(pexpect.EOF, timeout=8)
    qt_tui = None
    stop(qt_process)
    qt_process = None

    gtk_process = start_fixture(GTK_FIXTURE)
    wait_tree(GTK_APP, "External text probe")
    with tempfile.TemporaryDirectory(prefix="gui2tui-v06a-config-") as temp:
        config_home = pathlib.Path(temp)
        config_dir = config_home / "gui2tui"
        config_dir.mkdir(mode=0o700)
        handler = ROOT / "tests/fixtures/v03c_text_handler.py"
        config = config_dir / "config.toml"
        config.write_text(
            "version=1\n[interaction.complex_text]\n"
            "program='python3'\n"
            f"args=[{str(handler)!r},'{{file}}']\n",
            encoding="utf-8",
        )
        config.chmod(0o600)
        gtk_tui = tui(GTK_APP, config_home)
        gtk_tui.send(b"e")
        wait_marker(pathlib.Path(os.environ["GUI2TUI_VALIDATION_HANDLER_READY"]))
        stop(gtk_process)
        gtk_process = start_fixture(GTK_FIXTURE)
        replacement = wait_tree(GTK_APP, "alpha line")
        assert "handler candidate C" not in replacement
        pathlib.Path(os.environ["GUI2TUI_VALIDATION_HANDLER_RESUME"]).touch()
        product_log = RUNTIME / "gui2tui" / "product.log"
        wait_for(
            lambda: "application generation invalidated"
            in product_log.read_text(encoding="utf-8"),
            timeout=15,
            message="external-text G1 invalidation",
        )
        replacement = tree(GTK_APP, "--verbose")
        assert "alpha line" in replacement
        assert "handler candidate C" not in replacement
        artifacts = list(
            RUNTIME.glob("gui2tui/gui2tui-owned-*/operation-*/artifact-*.txt")
        )
        assert len(artifacts) == 1, artifacts
        artifact = artifacts[0]
        assert stat.S_ISREG(artifact.lstat().st_mode)
        assert artifact.lstat().st_mode & 0o077 == 0
        assert "handler candidate C" in artifact.read_text(encoding="utf-8")
        choose_fresh_application(
            gtk_tui,
            GTK_APP,
            product_log,
            "external-text fresh G2 binding",
        )
        gtk_tui.send(b"e")
        fresh_external = wait_tree(GTK_APP, "handler candidate C")
        assert "handler candidate C" in fresh_external
        print("EXTERNAL_TEXT_FINAL_TICKET_STALE_REFUSAL=PASS")
        gtk_tui.send(b"q")
        gtk_tui.expect(pexpect.EOF, timeout=8)
        gtk_tui = None
except BaseException:
    for application in (QT_APP, GTK_APP):
        try:
            print(f"--- {application} tree ---")
            print(tree(application, "--verbose"))
        except BaseException:
            pass
    product_log = RUNTIME / "gui2tui" / "product.log"
    if product_log.exists():
        print("--- v0.6A product log tail ---")
        print(product_log.read_text(encoding="utf-8")[-12000:])
    raise
finally:
    for child in (qt_tui, gtk_tui):
        if child is not None and child.isalive():
            child.close(force=True)
    stop(qt_process)
    stop(gtk_process)
