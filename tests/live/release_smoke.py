"""Real packaged CLI/PTY smoke in a fresh HOME. Only bundle paths are used."""
import json
import os
import pathlib
import subprocess
import time
import pexpect
import pyte

bundle = pathlib.Path(os.environ["GUI2TUI_BUNDLE"])
result = pathlib.Path(os.environ["RESULT_DIR"])
binary = bundle / "bin"
config = pathlib.Path(os.environ["XDG_CONFIG_HOME"]) / "gui2tui/config.toml"
sentinel = "release-password-sentinel"

def run(args, ok=True, env=None):
    out = subprocess.run([str(binary / "gui2tui"), *args], capture_output=True, text=True, timeout=10, env=env)
    assert sentinel not in out.stdout + out.stderr
    if ok:
        assert out.returncode == 0, (args, out.stderr)
    else:
        assert out.returncode != 0, args
    return out

def launch(args):
    return pexpect.spawn(str(binary / "gui2tui"), args, encoding=None, dimensions=(38, 130), env=os.environ.copy())

def frame(child, seconds=.5):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    text = "\n".join(screen.display)
    assert sentinel not in text
    return text

def save(name, text):
    (result / name).write_text(text)

def wait_for(child, needle, timeout=10):
    deadline = time.monotonic() + timeout
    text = ""
    while time.monotonic() < deadline:
        text = frame(child, .15)
        if needle in text:
            return text
    raise AssertionError(f"timed out waiting for {needle!r}\n{text}")

screen = pyte.Screen(130, 38)
stream = pyte.Stream(screen)
assert subprocess.run([str(binary / "gui2tui"), "--version"], capture_output=True).returncode == 0
save("help.txt", run(["--help"]).stdout)
save("version.txt", run(["--version"]).stdout)
assert "doctor" in run(["--help"]).stdout
assert not config.exists()
run(["config", "check"])
run(["config", "show"])
assert not config.exists()
child = launch([])
initial = wait_for(child, "No accessible applications")
save("no-apps.txt", initial)
child.send(b"q")
child.expect(pexpect.EOF, timeout=5)
child.close()
assert child.exitstatus == 0

fixture = subprocess.Popen(["python3", str(bundle / "smoke/release_smoke_gtk.py")], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
broker = None
try:
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        apps = subprocess.run([str(binary / "gui2tui"), "inspect", "--list"], capture_output=True, text=True, timeout=5)
        if "gui2tui-release-demo" in apps.stdout:
            break
        time.sleep(.15)
    else:
        raise AssertionError("GTK fixture did not register with AT-SPI")
    healthy = run(["doctor", "--json", "--report", str(result / "report.json")])
    report = json.loads(healthy.stdout)
    assert not any(c["level"] == "FAIL" for c in report["checks"]), report
    assert any(c["name"] == "same-host-endpoint" and c["level"] == "WARN" for c in report["checks"])
    save("doctor-healthy.txt", run(["doctor", "--verbose"]).stdout)
    save("doctor-healthy.json", healthy.stdout)
    env = os.environ.copy()
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)
    headless = json.loads(run(["doctor", "--json"], env=env).stdout)
    assert not any(c["level"] == "FAIL" for c in headless["checks"])
    save("doctor-headless.json", json.dumps(headless, indent=2))
    env["DBUS_SESSION_BUS_ADDRESS"] = "unix:path=/nonexistent/gui2tui-bus"
    save("doctor-no-bus.json", run(["doctor", "--json"], ok=False, env=env).stdout)

    child = launch(["--log-level", "debug"])
    selector = wait_for(child, "gui2tui-release-demo")
    save("selector.txt", selector)
    child.send(b"/release\rr")  # filter accepted, refresh preserves it
    frame(child)
    child.send(b"\r")
    scene = wait_for(child, "Activate safely")
    assert "alice" in scene
    save("scene.txt", scene)
    # v0.2 spatial presentation compacts button labels (`[Activate safely]`)
    # while the compatibility layout retains padded labels.
    button_y = next(i for i, line in enumerate(scene.splitlines()) if "Activate safely" in line)
    button_x = scene.splitlines()[button_y].index("Activate safely")
    child.send(b"?")
    help_frame = frame(child)
    assert "GUI2TUI Help" in help_frame and "Tab / Shift-Tab" in help_frame
    save("scene-help.txt", help_frame)
    child.send(f"\x1b[<0;{button_x + 1};{button_y + 1}M\x1b[<0;{button_x + 1};{button_y + 1}m".encode())
    frame(child)
    unchanged = subprocess.check_output([str(binary / "gui2tui"), "inspect", "--app", "gui2tui-release-demo"], text=True, stderr=subprocess.DEVNULL, timeout=10)
    assert "Status: idle" in unchanged, "help overlay let a click reach the GUI"
    child.send(b"\x1b")
    for _ in range(20):
        current = frame(child, .08)
        if any(line.strip("│ ").startswith("> ") and "Activate safely" in line for line in current.splitlines()):
            break
        child.send(b"\t")
    else:
        raise AssertionError("button not focusable")
    child.send(b"\r")
    after = wait_for(child, "Status: activated")
    assert "[x] Enable feature" in after, after
    tree = subprocess.check_output([str(binary / "gui2tui"), "inspect", "--app", "gui2tui-release-demo"], text=True, timeout=10)
    assert 'CheckBox "Enable feature" [checked' in tree and 'Status: activated' in tree
    assert sentinel not in tree
    save("after-action.txt", after)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    assert not config.exists(), "zero-config startup wrote configuration"
    log = pathlib.Path(os.environ["XDG_RUNTIME_DIR"]) / "gui2tui/product.log"
    assert log.exists() and "session stopped" in log.read_text()
    assert sentinel not in log.read_text() and "alice" not in log.read_text()

    fixture.terminate()
    fixture.wait(timeout=5)
    fixture = subprocess.Popen(["python3", str(bundle / "smoke/release_smoke_gtk.py")], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        apps = subprocess.run([str(binary / "gui2tui"), "inspect", "--list"], capture_output=True, text=True, timeout=5)
        if "gui2tui-release-demo" in apps.stdout:
            break
        time.sleep(.15)
    else:
        raise AssertionError("restarted GTK fixture did not register with AT-SPI")
    child = launch(["--app", "gui2tui-release-demo", "--no-mouse"])
    no_mouse_frame = wait_for(child, "Activate safely")
    child.send(f"\x1b[<0;{button_x + 1};{button_y + 1}M\x1b[<0;{button_x + 1};{button_y + 1}m".encode())
    frame(child)
    unchanged = subprocess.check_output([str(binary / "gui2tui"), "inspect", "--app", "gui2tui-release-demo"], text=True, stderr=subprocess.DEVNULL, timeout=10)
    assert "Status: idle" in unchanged
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0

    run(["config", "init"])
    run(["config", "check"])
    run(["config", "init"], ok=False)
    config.chmod(0o400)
    run(["config", "check"])
    config.chmod(0o600)
    valid = config.read_text()
    config.write_text("[terminal]\nmouse='config-secret'")
    error = run(["config", "check"], ok=False)
    assert "line 2" in error.stderr and "config-secret" not in error.stderr
    config.write_text(valid)

    socket = result / "runtime/viewer.sock"
    broker = subprocess.Popen([str(binary / "gui2tui"), "endpoint", "serve", "--socket", str(socket), "--mime", "image/png", "--recording-handler", "--authorization", "once"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(.5)
    connected = json.loads(run(["--modality-socket", str(socket), "doctor", "--json"]).stdout)
    assert any(c["name"] == "same-host-endpoint" and c["level"] == "PASS" for c in connected["checks"])
    save("doctor-broker.json", json.dumps(connected, indent=2))
    print("PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true")
finally:
    if broker is not None:
        broker.terminate()
        broker.wait(timeout=5)
    fixture.terminate()
    fixture.wait(timeout=5)
    if child.isalive():
        child.close(force=True)
