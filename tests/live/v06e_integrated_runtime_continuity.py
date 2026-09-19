#!/usr/bin/env python3
"""Integrated v0.6 runtime continuity evidence in one primary TUI process."""

from __future__ import annotations

import io
import json
import os
import pathlib
import re
import signal
import stat
import subprocess
import tempfile
import termios
import time

import pexpect


ROOT = pathlib.Path(os.environ["PROJECT_ROOT"])
GUI2TUI = os.environ["GUI2TUI"]
INSPECT = os.environ["INSPECT"]
RUNTIME = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
PRODUCT_LOG = RUNTIME / "gui2tui" / "product.log"
GTK_APP = "gui2tui-live-fixture"
QT_APP = "gui2tui-v06c-resource"
MODALITY_APP = "gui2tui-v06d-lifecycle"
LATE_APP = "gui2tui-v06a-fixture"
GTK_FIXTURE = ROOT / "tests/fixtures/gtk4_live_fixture.py"
QT_FIXTURE = ROOT / "tests/fixtures/v06c_qt_resource_fixture.py"
MODALITY_FIXTURE = ROOT / "tests/fixtures/v06d_lifecycle_fixture.py"
LATE_FIXTURE = ROOT / "tests/fixtures/v06a_qt_runtime_fixture.py"
HANDLER = ROOT / "tests/fixtures/v06d_text_handler.py"
QT_STATE = RUNTIME / "v06e-qt-state.json"
HANDLER_READY = RUNTIME / "v06e-handler-ready"
HANDLER_RESUME = RUNTIME / "v06e-handler-resume"
HANDLER_PID = RUNTIME / "v06e-handler-pid"
SUSPEND_MUTATE = pathlib.Path(os.environ["V06E_SUSPEND_MUTATE"])


def wait_for(read, predicate=lambda value: bool(value), timeout=20.0, message="condition"):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = read()
            if predicate(last):
                return last
        except (
            FileNotFoundError,
            ProcessLookupError,
            subprocess.CalledProcessError,
            json.JSONDecodeError,
            StopIteration,
            AssertionError,
        ):
            pass
        time.sleep(0.05)
    raise AssertionError(f"timed out waiting for {message}; last={last!r}")


def product_lines(needle):
    if not PRODUCT_LOG.exists():
        return []
    return [
        line
        for line in PRODUCT_LOG.read_text(encoding="utf-8").splitlines()
        if needle in line
    ]


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
        rf'(?:Button|CheckBox|ToggleButton) "{re.escape(label)}".* id=([^ ]+)',
        tree,
    )
    if not match:
        raise AssertionError(f"missing {label!r}")
    return match.group(1)


def invoke(application, label):
    def invoke_once():
        subprocess.run(
            [INSPECT, "--activate", node_for(application, label)],
            env=os.environ,
            text=True,
            capture_output=True,
            check=True,
            timeout=30,
        )
        return True

    wait_for(
        invoke_once,
        timeout=10,
        message=f"invoke {label!r} in {application}",
    )


def start_fixture(path, *arguments):
    return subprocess.Popen(
        ["python3", str(path), *map(str, arguments)],
        cwd=ROOT,
        env=os.environ.copy(),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )


def stop_fixture(process):
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def terminal_flags(child):
    local = termios.tcgetattr(child.child_fd)[3]
    return bool(local & termios.ICANON), bool(local & termios.ECHO)


def assert_raw(child):
    canonical, _echo = terminal_flags(child)
    assert not canonical


def assert_restored(child):
    canonical, echo = terminal_flags(child)
    assert canonical and echo, (canonical, echo)


def process_children(pid):
    path = pathlib.Path(f"/proc/{pid}/task/{pid}/children")
    if not path.exists():
        return []
    return [int(value) for value in path.read_text(encoding="ascii").split()]


def child_states(pid):
    states = []
    for child in process_children(pid):
        status = pathlib.Path(f"/proc/{child}/status")
        if status.exists():
            state = re.search(r"State:\s+(\S)", status.read_text(encoding="utf-8"))
            states.append((child, state.group(1) if state else "?"))
    return states


def artifacts(suffix="*"):
    return sorted(
        RUNTIME.glob(f"gui2tui/gui2tui-owned-*/operation-*/artifact-*.{suffix}")
    )


def resources(pid):
    status = pathlib.Path(f"/proc/{pid}/status").read_text(encoding="utf-8")
    children = child_states(pid)
    return {
        "fds": len(list(pathlib.Path(f"/proc/{pid}/fd").iterdir())),
        "threads": int(re.search(r"Threads:\s+(\d+)", status).group(1)),
        "rss_kib": int(re.search(r"VmRSS:\s+(\d+)", status).group(1)),
        "external_children": len(children),
        "zombies": sum(state == "Z" for _, state in children),
        "candidate_artifacts": len(artifacts("txt")),
        "modality_artifacts": len(artifacts("png")),
    }


def pump(child, seconds=0.15):
    output = bytearray()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            output.extend(child.read_nonblocking(65536, timeout=0.03))
        except pexpect.TIMEOUT:
            pass
    return output.decode("utf-8", "replace")


def runtime_status(child):
    child.setwinsize(110, 180)
    pump(child, 0.05)
    before = len(product_lines("runtime status requested"))
    child.send(b"\x1b[24~")
    line = wait_for(
        lambda: (pump(child, 0.05), product_lines("runtime status requested"))[1],
        lambda lines: len(lines) > before,
        timeout=10,
        message="runtime status diagnostics",
    )[-1]
    value = json.loads(line.split(" status=", 1)[1])
    child.send(b"\x1b")
    pump(child, 0.05)
    return value


def sample(child, point):
    return {"point": point, **runtime_status(child), **resources(child.pid)}


def open_palette(child, query):
    pump(child, 0.1)
    child.send(b":")
    # Ratatui may place cursor-position sequences between visible letters, so
    # terminal text is not a stable synchronization primitive. The palette
    # shortcut is synchronous. Feed the query as ordinary key events and let
    # each filter update settle before submitting it.
    pump(child, 0.2)
    for character in query.encode():
        child.send(bytes([character]))
        pump(child, 0.015)
    pump(child, 0.25)


def command(child, query, predicate, message, timeout=20, wait_idle=True):
    open_palette(child, query)
    child.send(b"\r")
    result = wait_for(
        lambda: (pump(child, 0.04), predicate())[1],
        timeout=timeout,
        message=message,
    )
    if wait_idle:
        status = runtime_status(child)
        assert status["active_operations"] == 0
    return result


def force_refresh(child, message):
    before = len(product_lines("full semantic refresh completed"))
    child.send(b"r")
    wait_for(
        lambda: len(product_lines("full semantic refresh completed")),
        lambda count: count > before,
        timeout=15,
        message=message,
    )
    pump(child, 0.2)


def rendered_text(capture, start=0):
    value = capture.getvalue()[start:].decode("utf-8", "replace")
    return re.sub(
        r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))", "", value
    )


def submit_frozen_refusal(child, capture, still_current, message):
    before = len(capture.getvalue())
    child.send(b"\r")
    normalized = wait_for(
        lambda: (
            pump(child, 0.05),
            "".join(rendered_text(capture, before).split()),
        )[1],
        lambda text: "nolongeravailable" in text and "current" in text,
        timeout=12,
        message=message,
    )
    assert "nolongeravailable" in normalized and "current" in normalized
    assert still_current()


def choose(child, application):
    selected = len(product_lines("user selected fresh application generation"))
    ready = len(product_lines("fresh application selector ready"))
    pump(child, 0.1)
    child.send(b"b")
    wait_for(
        lambda: (pump(child, 0.05), len(product_lines("fresh application selector ready")))[1],
        lambda count: count > ready,
        timeout=15,
        message=f"selector for {application}",
    )
    child.send(b"/")
    child.send(application.encode())
    child.send(b"\r\r")
    wait_for(
        lambda: (pump(child, 0.05), len(product_lines("user selected fresh application generation")))[1],
        lambda count: count > selected,
        timeout=20,
        message=f"fresh selection of {application}",
    )
    pump(child, 0.3)


def begin_external_edit(child):
    for _ in range(30):
        child.send(b"e")
        try:
            wait_for(HANDLER_READY.exists, timeout=0.35, message="handler ready")
            return
        except AssertionError:
            child.send(b"\t")
    raise AssertionError("could not focus the qualified external text target")


def prepare_handler(immediate=False):
    for path in (HANDLER_READY, HANDLER_RESUME, HANDLER_PID):
        path.unlink(missing_ok=True)
    if immediate:
        HANDLER_RESUME.touch()


def write_config(base):
    directory = base / "gui2tui"
    directory.mkdir(mode=0o700)
    config = directory / "config.toml"
    config.write_text(
        "version=1\n[interaction.complex_text]\n"
        "program='python3'\n"
        f"args=[{str(HANDLER)!r},'{{file}}']\n",
        encoding="utf-8",
    )
    config.chmod(0o600)


def spawn_tui(application, config_home):
    env = os.environ.copy()
    env["XDG_CONFIG_HOME"] = str(config_home)
    env["V06D_HANDLER_READY"] = str(HANDLER_READY)
    env["V06D_HANDLER_RESUME"] = str(HANDLER_RESUME)
    env["V06D_HANDLER_PID"] = str(HANDLER_PID)
    capture = io.BytesIO()
    child = pexpect.spawn(
        GUI2TUI,
        [
            "--app",
            application,
            "--settle-ms",
            "700",
            "--timeout-ms",
            "1000",
            "--no-mouse",
            "--log-level",
            "debug",
        ],
        cwd=ROOT,
        env=env,
        encoding=None,
        dimensions=(110, 180),
    )
    child.logfile_read = capture
    return child, capture


def checked(tree, label):
    line = next(line for line in tree.splitlines() if f'"{label}"' in line)
    return "checked" in line


processes = []
children = []
main = None
main_capture = None
samples = []
try:
    gtk = start_fixture(GTK_FIXTURE)
    qt = start_fixture(QT_FIXTURE, "--state-file", QT_STATE)
    modality = start_fixture(MODALITY_FIXTURE)
    processes.extend([gtk, qt, modality])
    wait_for(
        lambda: inspect(GTK_APP, check=False),
        lambda tree: "Run accessibility event storm" in tree,
        message="GTK fixture",
    )
    wait_for(
        lambda: inspect(QT_APP, check=False),
        lambda tree: "Fresh operation" in tree,
        message="Qt fixture",
    )
    modality_tree = wait_for(
        lambda: inspect(MODALITY_APP, check=False),
        lambda tree: "Lifecycle diagram" in tree,
        message="GTK3 modality fixture",
    )
    assert "v06d-private-secret" not in modality_tree

    with tempfile.TemporaryDirectory(prefix="gui2tui-v06e-config-") as temp:
        config_home = pathlib.Path(temp)
        write_config(config_home)
        prepare_handler(immediate=True)
        main, main_capture = spawn_tui(GTK_APP, config_home)
        children.append(main)
        main_pid = main.pid
        samples.append(sample(main, "initial-g1-gtk"))
        assert_raw(main)
        session = samples[0]["session"]
        assert samples[0]["generation"] == 1
        assert samples[0]["event_producer_owned"]
        assert samples[0]["cache_nodes"] == samples[0]["cache_locators"]

        # Stage 1: an ordinary Action with authoritative GTK readback.
        command(
            main,
            "Activate safely",
            lambda: "Status: activated" in inspect(GTK_APP),
            "initial authoritative action",
        )
        print("STAGE1_INITIAL_SEMANTIC_TASK=PASS generation=1")

        # Stage 2: a frozen background command loses authority inside a modal.
        open_palette(main, "Activate safely")
        invoke(GTK_APP, "Open modal dialog")
        wait_for(
            lambda: inspect(GTK_APP, "--dump-scopes"),
            lambda scopes: "ModalDialog" in scopes and "[ACTIVE]" in scopes,
            message="active GTK modal scope",
        )
        submit_frozen_refusal(
            main,
            main_capture,
            lambda: "ModalDialog" in inspect(GTK_APP, "--dump-scopes"),
            "background command refusal in modal",
        )
        command(
            main,
            "Close dialog",
            lambda: "ModalDialog" not in inspect(GTK_APP, "--dump-scopes"),
            "modal close",
        )
        force_refresh(main, "post-modal fresh scene")
        modal_invalidations = len(product_lines("application generation invalidated"))
        stop_fixture(gtk)
        wait_for(
            lambda: len(product_lines("application generation invalidated")),
            lambda count: count > modal_invalidations,
            timeout=20,
            message="post-modal application retirement",
        )
        gtk = start_fixture(GTK_FIXTURE)
        processes[0] = gtk
        wait_for(
            lambda: inspect(GTK_APP, check=False),
            lambda tree: "Open GTK secondary surface" in tree,
            message="fresh GTK fixture after modal",
        )
        choose(main, GTK_APP)
        assert runtime_status(main)["generation"] == 2
        print("STAGE2_DYNAMIC_SCOPE=PASS")

        # Stage 3 / cross C: overflow while replacing one of two same-looking
        # windows. The old frozen command must not bind to the replacement.
        invoke(GTK_APP, "Open GTK secondary surface")
        wait_for(
            lambda: inspect(GTK_APP, "--dump-scopes"),
            lambda scopes: scopes.count("Window") >= 2,
            message="GTK secondary surface",
        )
        force_refresh(main, "secondary surface fresh scene")
        old_secondary = node_for(GTK_APP, "GTK secondary operation")
        open_palette(main, "Close GTK secondary surface")
        overflow_before = len(
            product_lines(
                "event overflow converged through fresh semantic resynchronization"
            )
        )
        invoke(GTK_APP, "Run accessibility event storm")
        invoke(GTK_APP, "Close GTK secondary surface")
        invoke(GTK_APP, "Open GTK secondary surface")
        wait_for(
            lambda: len(
                product_lines(
                    "event overflow converged through fresh semantic resynchronization"
                )
            ),
            lambda count: count > overflow_before,
            timeout=35,
            message="overflow resync during window replacement",
        )
        new_secondary = wait_for(
            lambda: node_for(GTK_APP, "GTK secondary operation"),
            lambda locator: locator != old_secondary,
            message="fresh secondary locator",
        )
        assert new_secondary != old_secondary
        submit_frozen_refusal(
            main,
            main_capture,
            lambda: node_for(GTK_APP, "GTK secondary operation") == new_secondary,
            "old secondary command refusal",
        )
        command(
            main,
            "Close GTK secondary surface",
            lambda: inspect(GTK_APP, "--dump-scopes").count("Window") == 1,
            "fresh replacement secondary close",
        )
        force_refresh(main, "post-secondary main scene")
        command(
            main,
            "Activate safely",
            lambda: "Status: activated" in inspect(GTK_APP),
            "other window operation after overflow",
        )
        overflow_status = sample(main, "post-overflow-g2-gtk")
        samples.append(overflow_status)
        assert overflow_status["events"]["resync_requests"] >= 1
        assert overflow_status["cache_nodes"] == overflow_status["cache_locators"]
        assert overflow_status["active_operations"] == 0
        print("STAGE3_EVENT_CACHE_PRESSURE=PASS")
        print("OVERFLOW_MULTI_WINDOW_CONVERGENCE=PASS")

        # Stage 4: explicit GTK -> Qt selection creates G3 and retires G2.
        retirements = len(product_lines("application generation invalidated"))
        choose(main, QT_APP)
        wait_for(
            lambda: len(product_lines("application generation invalidated")),
            lambda count: count > retirements,
            message="GTK generation retirement",
        )
        g3 = sample(main, "g3-qt")
        samples.append(g3)
        assert g3["generation"] == 3 and g3["session"] == session
        assert g3["event_producer_owned"] and g3["event_producer_active"]
        assert g3["recent_commands"] == 0 and g3["scope_focus_history"] == 0
        qt_before = json.loads(QT_STATE.read_text(encoding="utf-8"))["fresh_toggles"]
        command(
            main,
            "Fresh operation",
            lambda: json.loads(QT_STATE.read_text(encoding="utf-8"))[
                "fresh_toggles"
            ]
            > qt_before,
            "fresh Qt operation",
        )
        received = runtime_status(main)["events"]["received"]
        invoke(GTK_APP, "Activate safely")
        pump(main, 0.5)
        assert runtime_status(main)["events"]["received"] == received
        print("STAGE4_APPLICATION_SWITCH=PASS generation=3")

        # Stage 5: switch to the qualified GTK3 text surface in G4 for successful and conflicting
        # external edits. Terminal ownership returns to this same process.
        choose(main, MODALITY_APP)
        g4 = runtime_status(main)
        assert g4["generation"] == 4 and g4["session"] == session
        before_candidates = len(artifacts("txt"))
        prepare_handler(immediate=True)
        begin_external_edit(main)
        wait_for(lambda: not process_children(main_pid), message="successful handler reap")
        wait_for(
            lambda: inspect(MODALITY_APP),
            lambda tree: "v06d handler candidate" in tree,
            message="successful external authoritative readback",
        )
        assert_raw(main)
        wait_for(
            lambda: len(artifacts("txt")),
            lambda count: count == before_candidates,
            message="successful candidate cleanup",
        )
        assert "v06d handler candidate" in inspect(MODALITY_APP)
        success_status = runtime_status(main)
        assert success_status["generation"] == 4
        assert success_status["active_operations"] == 0

        prepare_handler()
        begin_external_edit(main)
        invoke(MODALITY_APP, "Change lifecycle text independently")
        HANDLER_RESUME.touch()
        wait_for(lambda: not process_children(main_pid), message="conflict handler reap")
        assert "lifecycle authoritative B" in inspect(MODALITY_APP)
        wait_for(
            lambda: len(artifacts("txt")),
            lambda count: count == before_candidates + 1,
            message="conflict candidate preservation",
        )
        for artifact in artifacts("txt"):
            assert stat.S_ISREG(artifact.lstat().st_mode)
            assert artifact.lstat().st_mode & 0o077 == 0
            assert artifact.parent.lstat().st_mode & 0o077 == 0
        samples.append(sample(main, "post-external-g4-gtk3"))
        print("STAGE5_EXTERNAL_SUCCESS_CONFLICT=PASS generation=4")

        # Stage 6: the handler outlives G4, but its ticket/candidate cannot
        # write the replacement GTK3 application. Recovery is explicit G5.
        prepare_handler()
        begin_external_edit(main)
        stale_handler_pid = int(HANDLER_PID.read_text(encoding="ascii"))
        stale_invalidations = len(product_lines("application generation invalidated"))
        stop_fixture(modality)
        modality = start_fixture(MODALITY_FIXTURE)
        processes[2] = modality
        wait_for(
            lambda: len(product_lines("application generation invalidated")),
            lambda count: count > stale_invalidations,
            timeout=20,
            message="external target generation invalidation",
        )
        wait_for(
            lambda: inspect(MODALITY_APP, check=False),
            lambda tree: "lifecycle alpha" in tree,
            message="replacement GTK3 application",
        )
        HANDLER_RESUME.touch()
        wait_for(
            lambda: not pathlib.Path(f"/proc/{stale_handler_pid}").exists(),
            message="stale handler reap",
        )
        assert "v06d handler candidate" not in inspect(MODALITY_APP)
        wait_for(
            lambda: len(artifacts("txt")),
            lambda count: count == before_candidates + 2,
            message="stale candidate preservation",
        )
        choose(main, MODALITY_APP)
        g5 = runtime_status(main)
        assert g5["generation"] == 5 and g5["session"] == session
        command(
            main,
            "Lifecycle activate",
            lambda: "Lifecycle status: activated" in inspect(MODALITY_APP),
            "fresh operation after app replacement",
        )
        samples.append(sample(main, "recovered-g5-gtk3"))
        print("STAGE6_APP_LOSS_RECOVERY=PASS generation=5")
        print("EXTERNAL_STALE_CANDIDATE_REFUSAL=PASS")

        # Existing external modality participates in the same current G5. Open
        # its reference view, then retire that origin on the next switch. The
        # 0.6E mixed path does not repeat 0.6D's capture/materialization family.
        assert runtime_status(main)["generation"] == 5
        modality_view_start = len(main_capture.getvalue())
        main.send(b"\x1b[14~")
        wait_for(
            lambda: (
                pump(main, 0.05),
                "".join(rendered_text(main_capture, modality_view_start).split()),
            )[1],
            lambda text: "Externalmodality" in text
            and "Lifecyclediagram" in text,
            message="external modality reference view",
        )
        assert not artifacts("png")
        main.send(b"\x1b")

        # Cross A: a pending G6 transition cannot publish into the same-looking
        # replacement. Explicit selection creates usable G7 authority.
        late = start_fixture(LATE_FIXTURE)
        processes.append(late)
        wait_for(
            lambda: inspect(LATE_APP, check=False),
            lambda tree: "Delayed toggle" in tree,
            message="late-operation fixture",
        )
        choose(main, LATE_APP)
        assert not artifacts("png")
        assert runtime_status(main)["generation"] == 6
        pathlib.Path(os.environ["V06A_DELAY_READY"]).unlink(missing_ok=True)
        unsettled = len(
            product_lines("semantic transition event batch settled without confirmation")
        )
        command(
            main,
            "Delayed toggle",
            lambda: pathlib.Path(os.environ["V06A_DELAY_READY"]).exists(),
            "pending delayed operation",
            wait_idle=False,
        )
        wait_for(
            lambda: len(
                product_lines(
                    "semantic transition event batch settled without confirmation"
                )
            ),
            lambda count: count > unsettled,
            timeout=12,
            message="owned late transition",
        )
        late_invalidations = len(product_lines("application generation invalidated"))
        stop_fixture(late)
        late = start_fixture(LATE_FIXTURE)
        processes[-1] = late
        wait_for(
            lambda: inspect(LATE_APP, check=False),
            lambda tree: "Status: initial" in tree,
            message="same-looking late replacement",
        )
        wait_for(
            lambda: len(product_lines("application generation invalidated")),
            lambda count: count > late_invalidations,
            timeout=35,
            message="late operation generation retirement",
        )
        replacement_tree = inspect(LATE_APP, "--verbose")
        assert "Status: delayed returned" not in replacement_tree
        assert not checked(replacement_tree, "Fresh toggle")
        choose(main, LATE_APP)
        g8 = runtime_status(main)
        assert g8["generation"] == 7 and g8["active_operations"] == 0
        command(
            main,
            "Fresh toggle",
            lambda: checked(inspect(LATE_APP, "--verbose"), "Fresh toggle"),
            "fresh operation after late replacement",
        )
        print("LATE_OPERATION_REPLACEMENT_REFUSAL=PASS generations=6/7")

        # Stage 7: detach/suspend G7, change the GUI through public AT-SPI,
        # resume, fresh-read, and prove one current input reader by one action.
        before_suspend = runtime_status(main)
        os.kill(main_pid, signal.SIGTSTP)
        wait_for(
            lambda: pathlib.Path(f"/proc/{main_pid}/status")
            .read_text(encoding="utf-8")
            .split("State:", 1)[1]
            .lstrip()
            .startswith("T"),
            message="primary TUI stopped state",
        )
        assert_restored(main)
        SUSPEND_MUTATE.touch()
        wait_for(
            lambda: not checked(inspect(LATE_APP, "--verbose"), "Fresh toggle"),
            message="GUI change while TUI suspended",
        )
        os.kill(main_pid, signal.SIGCONT)
        main.expect_exact(b"\x1b[?1049h", timeout=12)
        assert_raw(main)
        after_suspend = wait_for(
            lambda: runtime_status(main),
            lambda status: status["full_snapshots"]
            > before_suspend["full_snapshots"],
            timeout=45,
            message="fresh semantic snapshot after resume",
        )
        assert after_suspend["generation"] == before_suspend["generation"] == 7
        assert after_suspend["terminal"] == "Attached"
        assert after_suspend["cache_nodes"] == after_suspend["cache_locators"]
        command(
            main,
            "Fresh toggle",
            lambda: checked(inspect(LATE_APP, "--verbose"), "Fresh toggle"),
            "fresh post-resume operation",
        )
        main.setwinsize(105, 170)
        pump(main, 0.3)
        samples.append(sample(main, "final-g7-after-resume"))
        print("STAGE7_SUSPEND_RESUME=PASS generation=7")

        # Stage 8: normal owned shutdown restores the PTY and retires G7.
        shutdowns = len(product_lines("application view shutdown completed"))
        main.send(b"q")
        main.expect(pexpect.EOF, timeout=12)
        assert_restored(main)
        main.close()
        assert main.exitstatus == 0
        wait_for(
            lambda: len(product_lines("application view shutdown completed")),
            lambda count: count > shutdowns,
            message="primary runtime shutdown",
        )
        terminal_bytes = main_capture.getvalue()
        assert b"\x1b[?1049h" in terminal_bytes and b"\x1b[?1049l" in terminal_bytes
        main = None
        print("STAGE8_NORMAL_EXIT=PASS")
        print(f"INTEGRATED_PRIMARY_PID={main_pid}")

        # Cross B is intentionally a separate TUI process: shutdown ends the
        # primary run. It exercises the existing defer-once policy without
        # killing the user's handler and with a same-name replacement present.
        stop_fixture(modality)
        modality = start_fixture(MODALITY_FIXTURE)
        processes[2] = modality
        wait_for(
            lambda: inspect(MODALITY_APP, check=False),
            lambda tree: "Lifecycle external text" in tree,
            message="shutdown cross fixture",
        )
        prepare_handler()
        shutdown_child, shutdown_capture = spawn_tui(MODALITY_APP, config_home)
        children.append(shutdown_child)
        shutdown_status = runtime_status(shutdown_child)
        assert shutdown_status["generation"] == 1
        begin_external_edit(shutdown_child)
        shutdown_handler = int(HANDLER_PID.read_text(encoding="ascii"))
        before_shutdown_candidates = len(artifacts("txt"))
        assert before_shutdown_candidates == 1
        os.kill(shutdown_child.pid, signal.SIGTERM)
        assert_restored(shutdown_child)
        assert shutdown_child.isalive()
        assert pathlib.Path(f"/proc/{shutdown_handler}").exists()
        stop_fixture(modality)
        modality = start_fixture(MODALITY_FIXTURE)
        processes[2] = modality
        wait_for(
            lambda: inspect(MODALITY_APP, check=False),
            lambda tree: "lifecycle alpha" in tree,
            message="shutdown replacement target",
        )
        HANDLER_RESUME.touch()
        shutdown_child.expect(pexpect.EOF, timeout=15)
        assert_restored(shutdown_child)
        shutdown_child.close()
        assert shutdown_child.exitstatus == 0
        wait_for(
            lambda: not pathlib.Path(f"/proc/{shutdown_handler}").exists(),
            message="shutdown handler reaped",
        )
        assert "v06d handler candidate" not in inspect(MODALITY_APP)
        wait_for(
            lambda: len(artifacts("txt")),
            lambda count: count == before_shutdown_candidates,
            message="shutdown candidate preservation",
        )
        assert not any(state == "Z" for _, state in child_states(shutdown_child.pid))
        shutdown_bytes = shutdown_capture.getvalue()
        assert b"\x1b[?1049h" in shutdown_bytes and b"\x1b[?1049l" in shutdown_bytes
        print("EXTERNAL_HANDLER_SHUTDOWN_INTEGRATION=PASS policy=defer-once")

    compact_samples = []
    for item in samples:
        compact_samples.append(
            {
                key: item[key]
                for key in (
                    "point",
                    "session",
                    "generation",
                    "state",
                    "terminal",
                    "cache_nodes",
                    "cache_locators",
                    "scene_bindings",
                    "interaction_scopes",
                    "scope_focus_history",
                    "recent_commands",
                    "event_queue_depth",
                    "event_queue_capacity",
                    "active_operations",
                    "temporary_artifacts",
                    "fds",
                    "threads",
                    "rss_kib",
                    "external_children",
                    "zombies",
                    "candidate_artifacts",
                    "modality_artifacts",
                )
            }
        )
    assert all(item["session"] == compact_samples[0]["session"] for item in compact_samples)
    assert all(item["zombies"] == 0 for item in compact_samples)
    assert all(item["cache_nodes"] == item["cache_locators"] for item in compact_samples)
    assert compact_samples[-1]["active_operations"] == 0
    assert compact_samples[-1]["event_queue_depth"] == 0
    print("INTEGRATED_CONTINUOUS_RUNTIME=PASS")
    print("MIXED_LIFECYCLE_AUTHORITY_ISOLATION=PASS")
    print("MIXED_SURFACE_EVENT_CONVERGENCE=PASS")
    print("MIXED_EXTERNAL_TERMINAL_CONTINUITY=PASS")
    print("APP_RECOVERY_FRESH_AUTHORITY=PASS")
    print("POST_RECOVERY_SEMANTIC_USABILITY=PASS")
    print("TERMINAL_RESTORATION_AND_SINGLE_READER=PASS")
    print("LOGICAL_RESOURCE_GROWTH=BOUNDED")
    print("NO_STALE_AUTHORITY_MIGRATION=PASS")
    print("NO_NEW_RUNTIME_FRAMEWORK=PASS")
    print("RESOURCE_SAMPLES=" + json.dumps(compact_samples, sort_keys=True))
except BaseException:
    if main_capture is not None:
        rendered = rendered_text(main_capture)
        print("--- v0.6E primary terminal tail ---")
        print(rendered[-3000:])
    if PRODUCT_LOG.exists():
        print("--- v0.6E product log tail ---")
        print(PRODUCT_LOG.read_text(encoding="utf-8")[-8000:])
    raise
finally:
    for child in children:
        if child.isalive():
            child.close(force=True)
    for process in reversed(processes):
        stop_fixture(process)
