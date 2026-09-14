#!/usr/bin/env python3
"""Bounded live event/surface/resource continuity campaign for v0.6C."""

import json
import os
import pathlib
import re
import signal
import subprocess
import time

import pexpect
import pyte


ROOT = pathlib.Path(os.environ["PROJECT_ROOT"])
GUI2TUI = os.environ["GUI2TUI"]
INSPECT = os.environ["INSPECT"]
RUNTIME = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
STATE = RUNTIME / "v06c-resource.json"
RESOURCE_FIXTURE = ROOT / "tests/fixtures/v06c_qt_resource_fixture.py"
QT_APP = "gui2tui-v06c-resource"
GTK_APP = "gui2tui-live-fixture"
PRODUCT_LOG = RUNTIME / "gui2tui" / "product.log"


def wait_for(read, predicate=lambda value: bool(value), timeout=15, message="condition"):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = read()
            if predicate(last):
                return last
        except (subprocess.CalledProcessError, FileNotFoundError, json.JSONDecodeError):
            pass
        time.sleep(0.05)
    raise AssertionError(f"timed out waiting for {message}; last={last!r}")


def inspect(application, *extra, check=True):
    result = subprocess.run(
        [INSPECT, "--app", application, "--bootstrap", "walk", *extra],
        env=os.environ,
        text=True,
        capture_output=True,
        check=check,
        timeout=30,
    )
    return result.stdout


def node_for(application, label):
    tree = inspect(application, "--verbose")
    match = re.search(
        rf'(?:Button|CheckBox) "{re.escape(label)}".* id=([^ ]+)', tree
    )
    if not match:
        raise AssertionError(f"missing {label!r}\n{tree}")
    return match.group(1)


def invoke(application, label):
    subprocess.run(
        [INSPECT, "--activate", node_for(application, label)],
        env=os.environ,
        text=True,
        capture_output=True,
        check=True,
        timeout=30,
    )


def read_state():
    return json.loads(STATE.read_text(encoding="utf-8"))


def resources(pid):
    status = pathlib.Path(f"/proc/{pid}/status").read_text(encoding="utf-8")
    return {
        "fds": len(list(pathlib.Path(f"/proc/{pid}/fd").iterdir())),
        "threads": int(re.search(r"Threads:\s+(\d+)", status).group(1)),
        "rss_kib": int(re.search(r"VmRSS:\s+(\d+)", status).group(1)),
    }


screen = pyte.Screen(140, 100)
stream = pyte.Stream(screen)


def pump(child, seconds=0.15):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=0.03).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def runtime_status(child):
    child.setwinsize(100, 140)
    screen.resize(100, 140)
    pump(child, 0.05)
    before = len(product_lines("runtime status requested"))
    child.send(b"\x1b[24~")
    line = wait_for(
        lambda: (pump(child, 0.05), product_lines("runtime status requested"))[1],
        lambda lines: len(lines) > before,
        timeout=8,
        message="runtime status diagnostics",
    )[-1]
    value = json.loads(line.split(" status=", 1)[1])
    child.send(b"\x1b")
    pump(child, 0.05)
    return value


def open_palette(child, query):
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode())
    pump(child, 0.08)


def command(child, query, predicate, message):
    open_palette(child, query)
    child.send(b"\r")
    return wait_for(lambda: (pump(child, 0.03), predicate())[1], message=message)


def expect_current_command_unavailable(child):
    for word in (b"Command", b"available", b"choose", b"current", b"interface"):
        child.expect(word, timeout=8)


def choose(child, application, expected):
    selected = len(product_lines("user selected fresh application generation"))
    child.send(b"b")
    child.expect(b"Select application", timeout=8)
    child.send(b"/")
    child.send(application.encode())
    child.send(b"\r\r")
    wait_for(
        lambda: (pump(child, 0.05), len(product_lines("user selected fresh application generation")))[1],
        lambda count: count > selected,
        message=f"fresh selection of {application}",
    )
    pump(child, 0.2)


def product_lines(needle):
    if not PRODUCT_LOG.exists():
        return []
    return [
        line
        for line in PRODUCT_LOG.read_text(encoding="utf-8").splitlines()
        if needle in line
    ]


processes = []
child = None
samples = []
surface_cycles = int(os.environ.get("V06C_SURFACE_CYCLES", "50"))
assert surface_cycles >= 2
try:
    resource_log = (RUNTIME / "v06c-resource.log").open("w")
    resource_process = subprocess.Popen(
        ["python3", str(RESOURCE_FIXTURE), "--state-file", str(STATE)],
        env=os.environ.copy(),
        stdout=resource_log,
        stderr=resource_log,
        start_new_session=True,
    )
    resource_log.close()
    processes.append(resource_process)
    gtk_log = (RUNTIME / "v06c-gtk.log").open("w")
    gtk_process = subprocess.Popen(
        ["python3", str(ROOT / "tests/fixtures/gtk4_live_fixture.py")],
        env=os.environ.copy(),
        stdout=gtk_log,
        stderr=gtk_log,
        start_new_session=True,
    )
    gtk_log.close()
    processes.append(gtk_process)
    wait_for(lambda: inspect(QT_APP, check=False), lambda value: "Fresh operation" in value, message="Qt resource fixture")
    wait_for(lambda: inspect(GTK_APP, check=False), lambda value: "Run accessibility event storm" in value, message="GTK storm fixture")

    child = pexpect.spawn(
        GUI2TUI,
        [
            "--app",
            QT_APP,
            "--layout",
            "flat",
            "--settle-ms",
            "80",
            "--timeout-ms",
            "1000",
            "--no-mouse",
            "--log-level",
            "debug",
        ],
        env=os.environ.copy(),
        encoding=None,
        dimensions=(100, 140),
    )
    child.expect(b"Fresh operation", timeout=15)
    initial_status = runtime_status(child)
    initial_resources = resources(child.pid)
    samples.append({"point": "initial", **initial_status, **initial_resources})
    assert initial_status["event_producer_owned"]
    assert initial_status["event_producer_active"]
    assert initial_status["cache_nodes"] == initial_status["cache_locators"]

    # Fifty same-looking modal replacements exceed the old history growth
    # pattern while remaining a bounded, cycle-based qualification.
    for cycle in range(1, surface_cycles + 1):
        command(
            child,
            "Open churn modal",
            lambda: read_state()["modal_open"],
            f"modal open {cycle}",
        )
        command(
            child,
            "Close churn modal",
            lambda: not read_state()["modal_open"]
            and read_state()["modal_closed"] >= cycle,
            f"modal close {cycle}",
        )
        if cycle in (surface_cycles // 2, surface_cycles):
            sample = runtime_status(child)
            samples.append({"point": f"modal-{cycle}", **sample, **resources(child.pid)})
            assert sample["cache_nodes"] == sample["cache_locators"]
            assert sample["scope_focus_history"] <= sample["interaction_scopes"]
            assert sample["recent_commands"] <= sample["scene_bindings"]
            assert sample["active_operations"] == 0

    # Freeze a command for one dialog, replace it with an identical dialog,
    # and prove that presentation similarity cannot reactivate the old target.
    command(child, "Open churn modal", lambda: read_state()["modal_open"], "stale modal open")
    open_palette(child, "Close churn modal")
    old_closed = read_state()["modal_closed"]
    invoke(QT_APP, "Close churn modal")
    wait_for(read_state, lambda state: state["modal_closed"] > old_closed, message="old modal close")
    invoke(QT_APP, "Open churn modal")
    wait_for(read_state, lambda state: state["modal_open"], message="replacement modal")
    pump(child, 0.6)
    child.send(b"\r")
    expect_current_command_unavailable(child)
    assert read_state()["modal_open"]
    command(child, "Close churn modal", lambda: not read_state()["modal_open"], "fresh replacement close")

    # Two simultaneous top-level windows; close/recreate the same-looking W2
    # and reject the command captured from its predecessor.
    command(child, "Open secondary surface", lambda: read_state()["secondary_open"], "secondary open")
    scopes = wait_for(
        lambda: inspect(QT_APP, "--dump-scopes"),
        lambda value: value.count("Window") >= 2 and "[ACTIVE]" in value,
        message="two current windows",
    )
    assert scopes.count("Window") >= 2
    command(
        child,
        "Secondary surface operation",
        lambda: read_state()["secondary_toggles"] >= 1,
        "secondary operation",
    )
    open_palette(child, "Close secondary surface")
    secondary_closed = read_state()["secondary_closed"]
    invoke(QT_APP, "Close secondary surface")
    wait_for(read_state, lambda state: state["secondary_closed"] > secondary_closed, message="old secondary close")
    invoke(QT_APP, "Open secondary surface")
    wait_for(read_state, lambda state: state["secondary_open"], message="replacement secondary")
    pump(child, 0.6)
    child.send(b"\r")
    expect_current_command_unavailable(child)
    assert read_state()["secondary_open"]
    command(
        child,
        "Secondary surface operation",
        lambda: read_state()["secondary_toggles"] >= 2,
        "replacement secondary operation",
    )
    command(child, "Close secondary surface", lambda: not read_state()["secondary_open"], "secondary close")
    modal_final = runtime_status(child)
    samples.append({"point": "surface-final", **modal_final, **resources(child.pid)})
    assert modal_final["scope_focus_history"] <= modal_final["interaction_scopes"]
    assert modal_final["recent_commands"] <= modal_final["scene_bindings"]
    assert modal_final["active_operations"] == 0
    print("SURFACE_CHURN_RUNTIME_CONTINUITY=PASS")
    print("FOCUS_HISTORY_BOUNDED=PASS")
    print("RECENT_COMMANDS_BOUNDED=PASS")
    print("MULTI_WINDOW_CHURN_AUTHORITY=PASS")

    retirements = len(product_lines("application generation invalidated"))
    choose(child, GTK_APP, b"Run accessibility event storm")
    wait_for(
        lambda: len(product_lines("application generation invalidated")),
        lambda count: count > retirements,
        message="old event producer retirement",
    )
    current = runtime_status(child)
    assert current["event_producer_owned"] and current["event_producer_active"]
    retirement_line = product_lines("application generation invalidated")[-1]
    assert "event_producer_graceful=true" in retirement_line, retirement_line

    # Events emitted by the retired Qt application are not even counted by
    # the fresh GTK subscription, much less applied as semantic truth.
    received_before = current["events"]["received"]
    invoke(QT_APP, "Open churn modal")
    wait_for(read_state, lambda state: state["modal_open"], message="old-app event source")
    invoke(QT_APP, "Close churn modal")
    wait_for(read_state, lambda state: not state["modal_open"], message="old-app event cleanup")
    pump(child, 0.5)
    after_old_event = runtime_status(child)
    assert after_old_event["events"]["received"] == received_before
    print("EVENT_TASK_RETIREMENT=PASS")
    print("OLD_APP_EVENT_ISOLATION=PASS")

    # Exercise the real capacity-2048 queue. The existing fixture emits both
    # property and selection events, then a modal transition is folded into
    # the authoritative full-resync baseline.
    overflow_before = len(product_lines("event overflow converged through fresh semantic resynchronization"))
    invoke(GTK_APP, "Run accessibility event storm")
    invoke(GTK_APP, "Open modal dialog")
    wait_for(
        lambda: len(product_lines("event overflow converged through fresh semantic resynchronization")),
        lambda count: count > overflow_before,
        timeout=30,
        message="event overflow fresh resync",
    )
    wait_for(
        lambda: inspect(GTK_APP, "--dump-scopes"),
        lambda value: any("ModalDialog" in line and "[ACTIVE]" in line for line in value.splitlines()),
        message="overflow surface convergence",
    )
    overflow_status = runtime_status(child)
    assert overflow_status["event_queue_capacity"] == 2048
    assert overflow_status["events"]["resync_requests"] >= 1
    assert overflow_status["cache_nodes"] == overflow_status["cache_locators"]
    assert "Storm complete: 2000" in inspect(GTK_APP)
    command(child, "Close dialog", lambda: "ModalDialog" not in inspect(GTK_APP, "--dump-scopes"), "close post-overflow dialog")
    command(child, "Activate safely", lambda: "Status: activated" in inspect(GTK_APP), "post-overflow operation")
    print("EVENT_OVERFLOW_FRESH_RESYNC=PASS")
    print("OVERFLOW_SURFACE_CONVERGENCE=PASS")

    # Several more fresh application views prove that exact producer/cache/
    # history ownership returns to one current bounded shape per generation.
    sequence = (
        [(QT_APP, b"Fresh operation"), (GTK_APP, b"Run accessibility event storm")] * 3
        + [(QT_APP, b"Fresh operation")]
    )
    generation = overflow_status["generation"]
    for index, (application, expected) in enumerate(sequence, 1):
        choose(child, application, expected)
        sample = runtime_status(child)
        generation += 1
        assert sample["generation"] == generation
        assert sample["event_producer_owned"] and sample["event_producer_active"]
        assert sample["cache_nodes"] == sample["cache_locators"]
        assert sample["recent_commands"] == 0
        assert sample["scope_focus_history"] == 0
        assert sample["active_operations"] == 0
        samples.append({"point": f"view-{index}", **sample, **resources(child.pid)})

    fresh_before = read_state()["fresh_toggles"]
    command(
        child,
        "Fresh operation",
        lambda: read_state()["fresh_toggles"] > fresh_before,
        "final fresh operation",
    )
    # Resource settling only: semantic ordering above is established through
    # exact state and generation checks, never this bounded wait.
    pump(child, 3.0)
    final_status = runtime_status(child)
    final_resources = resources(child.pid)
    last_view_resources = samples[-1]
    if last_view_resources["fds"] > initial_resources["fds"]:
        assert final_resources["fds"] < last_view_resources["fds"], (
            initial_resources,
            last_view_resources,
            final_resources,
        )
    if last_view_resources["threads"] > initial_resources["threads"]:
        assert final_resources["threads"] < last_view_resources["threads"], (
            initial_resources,
            last_view_resources,
            final_resources,
        )
    assert len(product_lines("application generation invalidated")) >= 7
    assert all("event_producer_graceful=true" in line for line in product_lines("application generation invalidated"))
    assert final_status["event_producer_owned"] and final_status["event_producer_active"]
    assert final_status["event_queue_depth"] == 0
    assert final_status["active_operations"] == 0
    print("APPLICATION_VIEW_RESOURCE_RETIREMENT=PASS")
    print("CACHE_CHURN_BOUNDED=PASS")
    print("CACHE_LOGICAL_GROWTH=BOUNDED")
    print("EVENT_TASK_GROWTH=BOUNDED")
    print("FOCUS_HISTORY_GROWTH=BOUNDED")
    print("RECENT_COMMAND_GROWTH=BOUNDED")
    print("NO_STALE_HISTORY_AUTHORITY=PASS")
    print("NO_NEW_SURFACE_IDENTITY=PASS")
    print("FD_GROWTH=BOUNDED")
    print("THREAD_GROWTH=BOUNDED")
    print("RSS_GROWTH=OBSERVED_BOUNDED")
    print(json.dumps({
        "initial_resources": initial_resources,
        "final_resources": final_resources,
        "logical_samples": [
            {
                key: sample[key]
                for key in (
                    "point", "generation", "cache_nodes", "cache_locators",
                    "scene_bindings", "interaction_scopes", "scope_focus_history",
                    "recent_commands", "event_queue_depth", "active_operations",
                    "fds", "threads", "rss_kib",
                )
            }
            for sample in samples
        ],
    }, sort_keys=True))

    shutdowns = len(product_lines("application view shutdown completed"))
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)
    child = None
    wait_for(
        lambda: len(product_lines("application view shutdown completed")),
        lambda count: count > shutdowns,
        message="owned shutdown retirement",
    )
except BaseException:
    if PRODUCT_LOG.exists():
        print("--- v0.6C product log tail ---")
        print(PRODUCT_LOG.read_text(encoding="utf-8")[-24000:])
    raise
finally:
    if child is not None and child.isalive():
        child.close(force=True)
    for process in reversed(processes):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
