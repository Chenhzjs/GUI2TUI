"""Kill and restart the real isolated AT-SPI launcher/bus while TUI is alive."""
import json
import os
import pathlib
import re
import signal
import subprocess
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
selector = "gui2tui-live-fixture"


def run(*command, check=True):
    return subprocess.run(command, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, check=check, timeout=20)


def enable_bus():
    address = run("gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                  "--object-path", "/org/a11y/bus", "--method", "org.a11y.Bus.GetAddress").stdout
    for prop in ("IsEnabled", "ScreenReaderEnabled"):
        run("gdbus", "call", "--session", "--dest", "org.a11y.Bus",
            "--object-path", "/org/a11y/bus", "--method",
            "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", prop, "<true>")
    owner = run("gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                "--object-path", "/org/freedesktop/DBus", "--method",
                "org.freedesktop.DBus.GetNameOwner", "org.a11y.Bus").stdout
    owner = re.search(r"'([^']+)'", owner).group(1)
    pid_text = run("gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                   "--object-path", "/org/freedesktop/DBus", "--method",
                   "org.freedesktop.DBus.GetConnectionUnixProcessID", owner).stdout
    launcher = int(re.search(r"uint32 (\d+)", pid_text).group(1))
    children = run("pgrep", "-P", str(launcher), check=False).stdout.split()
    return address.strip(), launcher, [int(value) for value in children]


fixture = None


def start_fixture():
    global fixture
    fixture = subprocess.Popen(["python3", "tests/fixtures/gtk4_live_fixture.py"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               start_new_session=True)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        listed = run(str(binary / "gui2tui-inspect"), "--list", check=False).stdout
        if selector in listed:
            return
        time.sleep(.1)
    raise AssertionError("fixture did not register on accessibility bus")


screen = pyte.Screen(140, 60)
stream = pyte.Stream(screen)


def pump(child, seconds=.3):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def status(child):
    child.send(b"\x1b[24~")
    frame = pump(child, .3)
    clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
    raw = clean[clean.index("{"):clean.rindex("}") + 1]
    value = json.loads(raw)
    child.send(b"\x1b")
    pump(child, .1)
    return value


address, launcher, bus_children = enable_bus()
start_fixture()
tree = run(str(binary / "gui2tui-inspect"), "--app", selector).stdout
button = next(line for line in tree.splitlines() if 'Button "Activate safely"' in line)
old_locator = re.search(r"atspi1_[A-Za-z0-9_-]+", button).group()
child = pexpect.spawn(str(binary / "gui2tui"), ["--app", selector], env=os.environ.copy(),
                     encoding=None, dimensions=(60, 140))
try:
    pump(child, 2)
    before = status(child)
    assert before["generation"] == 1
    loss_started = time.monotonic()
    root.joinpath("activation-blocked").touch()
    for pid in bus_children + [launcher]:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    time.sleep(2.5)  # longer than the complete 100/200/400/800 ms retry schedule
    unavailable = status(child)
    assert unavailable["state"] == "Degraded", unavailable
    assert unavailable["generation"] is None, unavailable
    assert unavailable["metrics"]["backend_losses"] == 1, unavailable
    attempts = unavailable["metrics"]["backend_reconnect_attempts"]
    assert attempts == 4, unavailable
    time.sleep(1.5)
    assert status(child)["metrics"]["backend_reconnect_attempts"] == attempts
    bus_absent = run("gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
        "--object-path", "/org/freedesktop/DBus", "--method",
        "org.freedesktop.DBus.NameHasOwner", "org.a11y.Bus").stdout
    assert "false" in bus_absent, "isolated activation override did not hold the daemon down"
    absent_seconds = time.monotonic() - loss_started

    os.killpg(fixture.pid, signal.SIGTERM)
    fixture.wait(timeout=5)
    root.joinpath("activation-blocked").unlink()
    new_address, new_launcher, new_children = enable_bus()
    assert new_address != address
    start_fixture()
    child.send(b"\x1b[15~")  # explicit bounded F5 reconnect/reselect
    pump(child, 3)
    after = status(child)
    assert after["state"] == "Running", after
    assert after["generation"] == 2, after
    assert after["focused_runtime"] != before["focused_runtime"], (before, after)
    assert after["metrics"]["backend_reconnects"] == 1, after

    stale = run(str(binary / "gui2tui-inspect"), "--actions", old_locator, check=False)
    assert stale.returncode != 0 and "panic" not in stale.stdout.lower(), stale.stdout
    new_tree = run(str(binary / "gui2tui-inspect"), "--app", selector).stdout
    new_button = next(line for line in new_tree.splitlines() if 'Button "Activate safely"' in line)
    new_locator = re.search(r"atspi1_[A-Za-z0-9_-]+", new_button).group()
    assert new_locator != old_locator
    run(str(binary / "gui2tui-inspect"), "--activate", new_locator)
    pump(child, 1)
    confirmed = run(str(binary / "gui2tui-inspect"), "--app", selector).stdout
    root.joinpath("confirmed-tree.txt").write_text(confirmed)
    assert "Status: activated" in confirmed
    assert 'CheckBox "Enable feature" [checked' in confirmed
    root.joinpath("daemon-recovery.json").write_text(json.dumps({
        "old_address": address, "new_address": new_address,
        "old_generation": before["generation"], "new_generation": after["generation"],
        "old_runtime": before["focused_runtime"], "new_runtime": after["focused_runtime"],
        "old_locator": old_locator, "new_locator": new_locator,
        "reconnect_attempts": after["metrics"]["backend_reconnect_attempts"],
        "reconnect_successes": after["metrics"]["backend_reconnects"],
        "daemon_absent_seconds": absent_seconds,
        "backoff_milliseconds": [100, 200, 400, 800],
    }, indent=2))
    print(f"REAL_ATSPI_DAEMON_LOSS=PASS absent_seconds={absent_seconds:.3f} bounded_attempts=4 no_busy_loop=true")
    print("REAL_ATSPI_DAEMON_RECONNECT=PASS fresh_generation=2 stale_locator_rejected=true")
finally:
    if child.isalive():
        child.send(b"q")
        try:
            child.expect(pexpect.EOF, timeout=5)
        except pexpect.TIMEOUT:
            child.close(force=True)
    if fixture is not None and fixture.poll() is None:
        os.killpg(fixture.pid, signal.SIGTERM)
        fixture.wait(timeout=5)
