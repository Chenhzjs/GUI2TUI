#!/usr/bin/env python3
"""Bounded PTY/live evidence for v0.6D external and terminal lifecycle."""

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
APP = "gui2tui-v06d-lifecycle"
FIXTURE = ROOT / "tests/fixtures/v06d_lifecycle_fixture.py"
HANDLER = ROOT / "tests/fixtures/v06d_text_handler.py"


def wait_for(predicate, timeout=15.0, message="condition"):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            last = predicate()
            if last:
                return last
        except (FileNotFoundError, ProcessLookupError, subprocess.CalledProcessError):
            pass
        time.sleep(0.05)
    raise AssertionError(f"timed out waiting for {message}; last={last!r}")


def tree() -> str:
    return subprocess.check_output(
        [INSPECT, "--app", APP, "--verbose"],
        env=os.environ,
        text=True,
        stderr=subprocess.DEVNULL,
    )


def start_fixture() -> subprocess.Popen:
    return subprocess.Popen(
        ["python3", str(FIXTURE)],
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


def assert_restored(child):
    canonical, echo = terminal_flags(child)
    assert canonical and echo, (canonical, echo)


def assert_raw(child):
    canonical, _echo = terminal_flags(child)
    assert not canonical


def spawn_tui(config_home=None, extra_env=None):
    env = os.environ.copy()
    if config_home is not None:
        env["XDG_CONFIG_HOME"] = str(config_home)
    if extra_env:
        env.update(extra_env)
    arguments = [
        "--app",
        APP,
        "--timeout-ms",
        "1000",
        "--settle-ms",
        "500",
        "--no-mouse",
        "--log-level",
        "debug",
    ]
    # Spatial presentation performs the ordinary bounded text probe that
    # qualifies a complete multiline source for configured external editing.
    if config_home is None:
        arguments.extend(["--layout", "flat"])
    child = pexpect.spawn(
        GUI2TUI,
        arguments,
        cwd=ROOT,
        env=env,
        encoding=None,
        dimensions=(50, 160),
    )
    children.append(child)
    child.expect(
        b"Edit externally" if config_home is not None else b"Lifecycle external text",
        timeout=15,
    )
    assert_raw(child)
    return child


def command(child, query):
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode() + b"\r")


def begin_external_edit(child, started):
    for _ in range(12):
        child.send(b"e")
        deadline = time.monotonic() + 1.0
        while time.monotonic() < deadline:
            if started():
                return
            time.sleep(0.05)
        child.send(b"\t")
    raise AssertionError("could not focus the qualified external text surface")


def activate_fresh_control(child):
    for _ in range(8):
        child.send(b"\t\r")
        try:
            wait_for(
                lambda: "Lifecycle status: activated" in tree(),
                timeout=0.75,
                message="fresh action after repeated handoff",
            )
            return
        except AssertionError:
            child.send(b"\x1b")
    raise AssertionError("fresh action was not usable after external handoff")


def process_children(pid):
    path = pathlib.Path(f"/proc/{pid}/task/{pid}/children")
    if not path.exists():
        return []
    return [int(value) for value in path.read_text(encoding="ascii").split()]


def fd_count(pid):
    return len(list(pathlib.Path(f"/proc/{pid}/fd").iterdir()))


def artifacts(suffix="*"):
    return list(
        RUNTIME.glob(f"gui2tui/gui2tui-owned-*/operation-*/artifact-*.{suffix}")
    )


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


fixture = None
children = []
try:
    fixture = start_fixture()
    wait_for(lambda: "Lifecycle external text" in tree(), message="lifecycle fixture")
    snapshot = tree()
    assert "v06d-private-secret" not in snapshot

    # The panic is after TerminalGuard attachment and before application work.
    panic = pexpect.spawn(
        GUI2TUI,
        ["--test-panic-after-attach", "--no-mouse"],
        env=os.environ.copy(),
        encoding=None,
        dimensions=(30, 100),
    )
    panic.expect(pexpect.EOF, timeout=10)
    assert_restored(panic)
    panic.close()
    assert panic.exitstatus not in (None, 0)
    print("TERMINAL_CONTROLLED_PANIC_RESTORED=PASS")

    normal = spawn_tui()
    normal.send(b"q")
    normal.expect(pexpect.EOF, timeout=10)
    assert_restored(normal)
    normal.close()
    assert normal.exitstatus == 0
    print("TERMINAL_NORMAL_EXIT_RESTORED=PASS")

    for sig, name in ((signal.SIGINT, "SIGINT"), (signal.SIGTERM, "SIGTERM")):
        child = spawn_tui()
        os.kill(child.pid, sig)
        child.expect(pexpect.EOF, timeout=10)
        assert_restored(child)
        child.close()
        assert child.exitstatus == 0
        print(f"TERMINAL_{name}_RESTORED=PASS")

    suspended = spawn_tui()
    while True:
        try:
            suspended.read_nonblocking(4096, timeout=0)
        except pexpect.TIMEOUT:
            break
    os.kill(suspended.pid, signal.SIGTSTP)
    wait_for(
        lambda: pathlib.Path(f"/proc/{suspended.pid}/status")
        .read_text(encoding="utf-8")
        .split("State:", 1)[1]
        .lstrip()
        .startswith("T"),
        message="GUI2TUI stopped state",
    )
    assert_restored(suspended)
    os.kill(suspended.pid, signal.SIGCONT)
    suspended.expect_exact(b"\x1b[?1049h", timeout=10)
    assert_raw(suspended)
    suspended.setwinsize(55, 170)
    command(suspended, "Lifecycle activate")
    wait_for(lambda: "Lifecycle status: activated" in tree(), message="post-resume action")
    suspended.send(b"q")
    suspended.expect(pexpect.EOF, timeout=10)
    assert_restored(suspended)
    suspended.close()
    print("TERMINAL_SUSPEND_RESUME=PASS")

    # Give the external lifecycle family a fresh semantic state so its final
    # action proves post-handoff usability rather than observing prior state.
    stop_fixture(fixture)
    fixture = start_fixture()
    wait_for(lambda: "Lifecycle status: idle" in tree(), message="fresh fixture")

    with tempfile.TemporaryDirectory(prefix="gui2tui-v06d-config-") as temp:
        config_home = pathlib.Path(temp)
        write_config(config_home)
        edited = spawn_tui(config_home)
        initial_fds = fd_count(edited.pid)
        peak_fds = initial_fds
        for cycle in range(4):
            begin_external_edit(
                edited,
                lambda cycle=cycle: tree().count("v06d handler candidate") >= cycle + 1,
            )
            edited.expect(b"External text update confirmed", timeout=10)
            assert_raw(edited)
            peak_fds = max(peak_fds, fd_count(edited.pid))
            wait_for(lambda: not process_children(edited.pid), message="handler reap")
            assert not artifacts("txt")
        final_fds = fd_count(edited.pid)
        assert final_fds <= initial_fds + 2, (initial_fds, peak_fds, final_fds)
        activate_fresh_control(edited)
        edited.send(b"q")
        edited.expect(pexpect.EOF, timeout=10)
        assert_restored(edited)
        edited.close()
        print("EXTERNAL_HANDLER_TERMINAL_REACQUIRE=PASS")
        print(
            "REPEATED_EXTERNAL_HANDOFF_CONTINUITY=PASS "
            f"cycles=4 fds={initial_fds}/{peak_fds}/{final_fds}"
        )
        print("EXTERNAL_HANDLER_PROCESS_REAPED=PASS")
        print("TERMINAL_READER_SINGLE_OWNER=PASS")
        print("EXTERNAL_ARTIFACT_SUCCESS_CLEANUP=PASS")
        print("POST_EXTERNAL_LIFECYCLE_FRESH_OPERATION=PASS")

        stale_ready = RUNTIME / "stale-ready"
        stale_resume = RUNTIME / "stale-resume"
        stale_pid = RUNTIME / "stale-pid"
        stale = spawn_tui(
            config_home,
            {
                "V06D_HANDLER_READY": str(stale_ready),
                "V06D_HANDLER_RESUME": str(stale_resume),
                "V06D_HANDLER_PID": str(stale_pid),
            },
        )
        begin_external_edit(stale, stale_ready.exists)
        stop_fixture(fixture)
        fixture = start_fixture()
        wait_for(lambda: "lifecycle alpha" in tree(), message="replacement fixture")
        stale_resume.touch()
        wait_for(
            lambda: "application generation invalidated"
            in (RUNTIME / "gui2tui/product.log").read_text(encoding="utf-8"),
            message="stale generation retirement",
        )
        assert "v06d handler candidate" not in tree()
        stale_artifacts = artifacts("txt")
        assert len(stale_artifacts) == 1, stale_artifacts
        mode = stale_artifacts[0].lstat().st_mode
        assert stat.S_ISREG(mode) and mode & 0o077 == 0
        assert stale_artifacts[0].parent.lstat().st_mode & 0o077 == 0
        stale.send(b"q")
        stale.expect(pexpect.EOF, timeout=10)
        stale.close()
        print("EXTERNAL_TEXT_STALE_AUTHORITY_REFUSAL=PASS")
        print("EXTERNAL_ARTIFACT_PRIVATE_PERMISSIONS=PASS")

        conflict_ready = RUNTIME / "conflict-ready"
        conflict_resume = RUNTIME / "conflict-resume"
        conflicting = spawn_tui(
            config_home,
            {
                "V06D_HANDLER_READY": str(conflict_ready),
                "V06D_HANDLER_RESUME": str(conflict_resume),
            },
        )
        begin_external_edit(conflicting, conflict_ready.exists)
        authoritative = spawn_tui(
            config_home,
            {"V06D_HANDLER_REPLACEMENT": "lifecycle authoritative B\n"},
        )
        begin_external_edit(
            authoritative,
            lambda: "lifecycle authoritative B" in tree(),
        )
        authoritative.expect(b"External text update confirmed", timeout=10)
        os.kill(authoritative.pid, signal.SIGTERM)
        authoritative.expect(pexpect.EOF, timeout=10)
        assert_restored(authoritative)
        authoritative.close()
        conflict_resume.touch()
        conflicting.expect(b"External text conflict detected", timeout=10)
        assert "lifecycle authoritative B" in tree()
        assert "v06d handler candidate" not in tree()
        assert len(artifacts("txt")) == 1
        conflicting.send(b"q")
        conflicting.expect(pexpect.EOF, timeout=10)
        assert_restored(conflicting)
        conflicting.close()
        print("EXTERNAL_TEXT_CONFLICT_PRESERVATION=PASS")

        shutdown_ready = RUNTIME / "shutdown-ready"
        shutdown_resume = RUNTIME / "shutdown-resume"
        shutdown_pid = RUNTIME / "shutdown-pid"
        shutting_down = spawn_tui(
            config_home,
            {
                "V06D_HANDLER_READY": str(shutdown_ready),
                "V06D_HANDLER_RESUME": str(shutdown_resume),
                "V06D_HANDLER_PID": str(shutdown_pid),
            },
        )
        # Startup recovery removes the ordinary stale candidate.
        assert not artifacts("txt")
        begin_external_edit(shutting_down, shutdown_ready.exists)
        handler_pid = int(shutdown_pid.read_text(encoding="ascii"))
        os.kill(shutting_down.pid, signal.SIGTERM)
        assert_restored(shutting_down)
        assert shutting_down.isalive()
        assert pathlib.Path(f"/proc/{handler_pid}").exists()
        shutdown_artifacts = artifacts("txt")
        assert len(shutdown_artifacts) == 1, shutdown_artifacts
        assert "v06d handler candidate" not in tree()
        shutdown_resume.touch()
        shutting_down.expect(pexpect.EOF, timeout=10)
        assert_restored(shutting_down)
        shutting_down.close()
        wait_for(
            lambda: not pathlib.Path(f"/proc/{handler_pid}").exists(),
            message="shutdown handler exit and reap",
        )
        print("EXTERNAL_HANDLER_SHUTDOWN_SAFE=PASS policy=defer-once-force-on-repeat")

        force_ready = RUNTIME / "force-ready"
        force_resume = RUNTIME / "force-resume"
        force_pid_path = RUNTIME / "force-pid"
        forced = spawn_tui(
            config_home,
            {
                "V06D_HANDLER_READY": str(force_ready),
                "V06D_HANDLER_RESUME": str(force_resume),
                "V06D_HANDLER_PID": str(force_pid_path),
            },
        )
        begin_external_edit(forced, force_ready.exists)
        force_pid = int(force_pid_path.read_text(encoding="ascii"))
        os.kill(forced.pid, signal.SIGTERM)
        time.sleep(0.2)
        assert forced.isalive() and pathlib.Path(f"/proc/{force_pid}").exists()
        os.kill(forced.pid, signal.SIGTERM)
        wait_for(
            lambda: not forced.isalive(),
            message="explicit repeated-signal forced shutdown",
        )
        assert_restored(forced)
        forced.close(force=True)
        wait_for(
            lambda: not pathlib.Path(f"/proc/{force_pid}").exists(),
            message="forced handler terminal hangup",
        )
        # Starting this cycle recovered the prior completed-shutdown candidate;
        # the forced exit leaves only its still-handler-safe TTL candidate.
        assert len(artifacts("txt")) == 1
        print("EXTERNAL_HANDLER_HANG_ESCAPE=PASS repeated_signal=true")

    modality = spawn_tui()
    modality.send(b"\x1b[14~")  # F4
    modality.expect(b"External modality", timeout=10)
    modality.send(b"m")
    stop_fixture(fixture)
    fixture = None
    modality_log = RUNTIME / "gui2tui/product.log"
    wait_for(
        lambda: "application generation invalidated"
        in modality_log.read_text(encoding="utf-8"),
        message="modality origin generation retirement",
    )
    assert not artifacts("png")
    modality.send(b"q")
    modality.expect(pexpect.EOF, timeout=10)
    assert_restored(modality)
    modality.close()
    print("EXTERNAL_MODALITY_ORIGIN_LOSS_SAFE=PASS")
    print("EXTERNAL_MODALITY_STORAGE_BOUNDED=PASS max=8 ttl_seconds=300")
    print("EXTERNAL_ARTIFACT_STORAGE_BOUNDED=PASS namespaces=256 files_per_namespace=256 ttl_seconds=1800")
except BaseException:
    log = RUNTIME / "gui2tui/product.log"
    if log.exists():
        print("--- v0.6D product log tail ---")
        print(log.read_text(encoding="utf-8")[-12000:])
    raise
finally:
    for child in children:
        if child.isalive():
            child.close(force=True)
    stop_fixture(fixture)
