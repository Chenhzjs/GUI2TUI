"""Real AT-SPI generations and attached/detached PTY, not a simulated cache.

Bounded accelerated churn. GUI mutations use advertised AT-SPI actions, never
fixture-native extraction. Writes an explicit duration, not a claimed soak.
"""
import json
import os
import pathlib
import re
import signal
import subprocess
import termios
import time

import pexpect
import pyte

root = pathlib.Path(os.environ["RESULT_DIR"])
binary = pathlib.Path(os.environ["TARGET_DIR"]) / "debug"
toolkit = os.environ.get("TEST_TOOLKIT", "gtk")
fixture = f"tests/fixtures/{'qt6' if toolkit == 'qt' else 'gtk4'}_live_fixture.py"
selector = {"qt": "gui2tui-qt-fixture", "chrome": "Google Chrome", "firefox": "Firefox"}.get(toolkit, "gui2tui-live-fixture")
rounds = int(os.environ.get("RESTART_ROUNDS", "20"))
started = time.monotonic()
application = None
tui = None
records = []


def inspect(*args, check=True):
    return subprocess.run([str(binary / "gui2tui-inspect"), *args], text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, check=check)


def start_application():
    global application
    if toolkit == "chrome":
        profile = root / f"chrome-profile-{len(records)}"
        command = ["google-chrome", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage",
                   "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
                   "--force-renderer-accessibility=complete", f"--user-data-dir={profile}",
                   f"file://{pathlib.Path('tests/fixtures/browser_fixture.html').resolve()}"]
    elif toolkit == "firefox":
        profile = root / f"firefox-profile-{len(records)}"
        profile.mkdir()
        profile.joinpath("user.js").write_bytes(pathlib.Path("tests/fixtures/firefox-user.js").read_bytes())
        command = [os.environ.get("FIREFOX_BIN", "/opt/firefox-154.0.1/firefox"), "--no-remote", "--profile", str(profile),
                   f"file://{pathlib.Path('tests/fixtures/browser_fixture.html').resolve()}"]
    else:
        command = ["python3", fixture]
    application = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    end = time.monotonic() + 8
    while time.monotonic() < end:
        if selector in inspect("--list", check=False).stdout:
            time.sleep(.2)
            return
        time.sleep(.1)
    raise AssertionError("fixture not registered")


screen = pyte.Screen(140, 70)
stream = pyte.Stream(screen)


def pump(seconds=.3):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(tui.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def status():
    tui.send(b"\x1b[24~")  # F12, contents-free runtime status
    frame = pump()
    clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
    raw = clean[clean.index("{"):clean.rindex("}") + 1]
    value = json.loads(raw)
    tui.send(b"\x1b")
    pump(.1)
    return value


def memory():
    text = pathlib.Path(f"/proc/{tui.pid}/status").read_text()
    return {"rss_kib": int(re.search(r"VmRSS:\s+(\d+)", text).group(1)),
            "threads": int(re.search(r"Threads:\s+(\d+)", text).group(1)),
            "fds": len(list(pathlib.Path(f"/proc/{tui.pid}/fd").iterdir()))}


try:
    start_application()
    tui = pexpect.spawn("bash", ["-c", 'exec "$1" --app "$3" 2>"$2"', "phase4a",
        str(binary / "gui2tui"), str(root / "runtime.log"), selector], env={**os.environ, "RUST_LOG": "gui2tui=debug"},
        encoding=None, dimensions=(70, 140))
    initial_frame = pump(2)
    root.joinpath("initial.txt").write_text(initial_frame)
    first = status()
    assert first["generation"] == 1 and first["endpoint"] == "Unavailable", first
    root.joinpath("initial-status.json").write_text(json.dumps(first, indent=2))
    old_foci = set()
    for generation in range(1, rounds + 1):
        current = status()
        assert current["generation"] == generation, current
        assert current["session"] == first["session"]
        assert current["focused_runtime"] not in old_foci
        old_foci.add(current["focused_runtime"])
        tree = inspect("--app", selector).stdout
        button_line = next(line for line in tree.splitlines() if 'Button "Activate safely"' in line)
        node = re.search(r"atspi1_[A-Za-z0-9_-]+", button_line).group()
        if generation == 1:
            os.kill(tui.pid, signal.SIGUSR1)
            pump(.5)
            flags = termios.tcgetattr(tui.child_fd)[3]
            assert flags & termios.ECHO and flags & termios.ICANON, flags
            # Runtime remains alive and consumes events with NO renderer attached.
            if toolkit in ("chrome", "firefox"):
                inspect("--action", node, "--index", "0")
            else:
                inspect("--activate", node)
            pump(2)
            root.joinpath("detached-inspector.txt").write_text(inspect("--app", selector).stdout)
            os.kill(tui.pid, signal.SIGUSR2)
            frame = pump(1)
            root.joinpath("reattached.txt").write_text(frame)
            root.joinpath("reattached-status.json").write_text(json.dumps(status(), indent=2))
            assert "Status: activated" in frame, frame
            root.joinpath("reattached.txt").write_text(frame)
            resumed = status()
            assert resumed["generation"] == 1
            assert resumed["focused_runtime"] == current["focused_runtime"]
            assert resumed["metrics"]["terminal_resumes"] == 1
            assert resumed["full_snapshots"] == current["full_snapshots"], resumed
            root.joinpath("resumed-status.json").write_text(json.dumps(resumed, indent=2))
            for rows, cols in [(24, 80), (50, 160), (20, 60), (70, 140)]:
                tui.setwinsize(rows, cols)
                screen.resize(rows, cols)
                pump(.2)
            resized = status()
            assert resized["focused_runtime"] == resumed["focused_runtime"]
            print("DETACH_EVENT_REATTACH_RESIZE=PASS", flush=True)
            if toolkit == "gtk":
                tree = inspect("--app", selector).stdout
                storm_line = next(line for line in tree.splitlines() if 'Button "Run accessibility event storm"' in line)
                storm_node = re.search(r"atspi1_[A-Za-z0-9_-]+", storm_line).group()
                before_storm = status()
                inspect("--activate", storm_node)
                pump(8)
                storm_status = status()
                storm_tree = inspect("--app", selector).stdout
                assert "Storm complete: 2000" in storm_tree
                assert storm_status["event_queue_depth"] <= storm_status["event_queue_capacity"]
                assert storm_status["events"]["received"] > before_storm["events"]["received"]
                assert storm_status["events"]["resync_requests"] <= 2, storm_status
                root.joinpath("storm-status.json").write_text(json.dumps(storm_status, indent=2))
                print("REAL_EVENT_STORM=PASS", storm_status["events"], flush=True)
        records.append({"generation": generation, "status": current, **memory()})
        if generation == rounds:
            break
        os.killpg(application.pid, signal.SIGTERM)
        application.wait(timeout=5)
        end = time.monotonic() + 6
        while "Application is no longer available" not in pump(.2):
            assert time.monotonic() < end, "application gone not detected"
        gone = status()
        assert gone["generation"] is None and gone["active_operations"] == 0, gone
        # Enter on a stale scene cannot dispatch an operation.
        tui.send(b"\r")
        pump(.1)
        start_application()
        stale = inspect("--actions", node, check=False)
        assert stale.returncode != 0 and "panic" not in stale.stdout.lower(), stale.stdout
        assert status()["generation"] is None, "same-name application was silently rebound"
        tui.send(b"\x1b[15~")  # F5 explicitly opens a NEW generation
        pump(.8)
        print(f"RESTART generation={generation + 1} stale_rejected=true", flush=True)

    os.kill(tui.pid, signal.SIGTERM)
    tui.expect(pexpect.EOF, timeout=10)
    flags = termios.tcgetattr(tui.child_fd)[3]
    assert flags & termios.ECHO and flags & termios.ICANON, flags
    tui.close()
    assert tui.exitstatus == 0, (tui.exitstatus, tui.signalstatus)
    report = {"toolkit": toolkit, "generations": rounds, "elapsed_seconds": time.monotonic() - started,
              "mode": "accelerated churn, NOT a 30-minute soak", "records": records,
              "sigterm_terminal_restored": True}
    root.joinpath("lifecycle.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({k: v for k, v in report.items() if k != "records"}), flush=True)
finally:
    if tui is not None and tui.isalive():
        tui.send(b"\x03")
        tui.close(force=True)
    if application is not None and application.poll() is None:
        os.killpg(application.pid, signal.SIGTERM)
        application.wait(timeout=5)
