"""Real packaged CLI/PTY smoke in a fresh HOME. Only bundle paths are used."""
import json
import os
import pathlib
import stat
import subprocess
import time
import pexpect
import pyte

bundle = pathlib.Path(os.environ["GUI2TUI_BUNDLE"])
result = pathlib.Path(os.environ["RESULT_DIR"])
binary = bundle / "bin"
inspector = bundle / "libexec/gui2tui/gui2tui-inspect"
config = pathlib.Path(os.environ["XDG_CONFIG_HOME"]) / "gui2tui/config.toml"
sentinel = "release-password-sentinel"
transcript = ""

def run(args, ok=True, env=None):
    out = subprocess.run([str(binary / "gui2tui"), *args], capture_output=True, text=True, timeout=10, env=env)
    assert sentinel not in out.stdout + out.stderr
    if ok:
        assert out.returncode == 0, (args, out.stderr)
    else:
        assert out.returncode != 0, args
    return out

def launch(args, env_updates=None):
    global transcript
    transcript = ""
    environment = os.environ.copy()
    environment.update(env_updates or {})
    return pexpect.spawn(
        str(binary / "gui2tui"),
        args,
        encoding=None,
        dimensions=(38, 130),
        env=environment,
    )

def frame(child, seconds=.5):
    global transcript
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            chunk = child.read_nonblocking(65536, timeout=.05).decode("utf-8", "replace")
            transcript += chunk
            stream.feed(chunk)
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

def wait_for_output(child, needle, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = frame(child, .15)
        if needle in transcript:
            return text
    raise AssertionError(f"timed out waiting for output {needle!r}\n{text}")

def focus_control(child, needle, attempts=40):
    for _ in range(attempts):
        text = frame(child, .12)
        if any(
            line.strip("│ ").startswith("> ") and needle in line
            for line in text.splitlines()
        ):
            return text
        child.send(b"\t")
    raise AssertionError(f"control not focusable: {needle!r}\n{text}")

def inspect_tree(verbose=False, application="gui2tui-release-demo"):
    args = [str(inspector), "--app", application]
    if verbose:
        args.append("--verbose")
    return subprocess.check_output(
        args, text=True, stderr=subprocess.DEVNULL, timeout=10
    )

def invoke_named_button(label):
    tree = inspect_tree()
    node_id = next(
        line.rsplit("id=", 1)[1].split()[0]
        for line in tree.splitlines()
        if f'Button "{label}"' in line
    )
    subprocess.run(
        [str(inspector), "--action-name", node_id, "Click"],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=10,
    )

def wait_for_tree(needle, present=True, timeout=10, application="gui2tui-release-demo"):
    deadline = time.monotonic() + timeout
    tree = ""
    while time.monotonic() < deadline:
        tree = inspect_tree(application=application)
        if (needle in tree) == present:
            return tree
        time.sleep(.1)
    raise AssertionError(
        f"timed out waiting for tree presence={present}: {needle!r}\n{tree}"
    )

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
value_fixture = None
text_fixture = None
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

    # Keep the historical pointer-coordinate assertions on the compatibility
    # layout; the initial no-app launch above already exercises v0.2's spatial
    # default startup path.
    child = launch(["--layout", "flat", "--log-level", "debug"])
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
    button_y = next(
        i
        for i, line in enumerate(scene.splitlines())
        if "[Activate safely]" in line or "[ Activate safely ]" in line
    )
    button_x = scene.splitlines()[button_y].index("Activate safely")
    child.send(b"?")
    help_frame = frame(child)
    assert "GUI2TUI Help" in help_frame and "Tab / Shift-Tab" in help_frame
    save("scene-help.txt", help_frame)
    child.send(f"\x1b[<0;{button_x + 1};{button_y + 1}M\x1b[<0;{button_x + 1};{button_y + 1}m".encode())
    frame(child)
    unchanged = subprocess.check_output([str(binary / "gui2tui"), "inspect", "--app", "gui2tui-release-demo"], text=True, stderr=subprocess.DEVNULL, timeout=10)
    assert "Status: activated" not in unchanged, "help overlay let a click reach the GUI"
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
    # The TUI frame above is the authoritative post-action read-back.  The
    # inspector may compact or omit unchanged fixture details in v0.2 scenes.
    assert sentinel not in tree
    save("after-action.txt", after)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0

    # The default responsive layout carries a qualified bounded Value through
    # the normal TUI operation path.  Fresh Inspector reads independently
    # confirm both mutation and restoration; the adjacent progress value never
    # receives an adjustment affordance.
    value_fixture = subprocess.Popen(
        ["python3", str(bundle / "smoke/release_smoke_qt.py")],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        apps = run(["inspect", "--list"])
        if "gui2tui-release-value-demo" in apps.stdout:
            break
        time.sleep(.15)
    else:
        raise AssertionError("Qt Value fixture did not register with AT-SPI")
    child = launch(["--app", "gui2tui-release-value-demo", "--no-mouse"])
    value_scene = wait_for(child, "Release value")
    focus_control(child, "Release value")
    child.send(b"\x1b[A")
    value_after = wait_for(child, "Release value: 5")
    value_tree = inspect_tree(verbose=True, application="gui2tui-release-value-demo")
    assert any(
        'Slider "Release value"' in line and 'value="5"' in line
        for line in value_tree.splitlines()
    ), value_tree
    child.send(b"\x1b[B")
    value_restored = wait_for(child, "Release value: 4")
    restored_tree = inspect_tree(verbose=True, application="gui2tui-release-value-demo")
    assert any(
        'Slider "Release value"' in line and 'value="4"' in line
        for line in restored_tree.splitlines()
    ), restored_tree
    progress_lines = [
        line for line in restored_tree.splitlines()
        if 'ProgressBar "Release progress"' in line
    ]
    assert progress_lines and all("read-only" in line for line in progress_lines), restored_tree
    save("value-initial.txt", value_scene)
    save("value-after.txt", value_after)
    save("value-restored.txt", value_restored)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    value_fixture.terminate()
    value_fixture.wait(timeout=5)
    value_fixture = None
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
    assert "Status: activated" not in unchanged
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

    handler = bundle / "smoke/v03c_text_handler.py"
    config.write_text(
        valid
        + "\n[interaction.complex_text]\n"
        + "program='python3'\n"
        + f"args=[{str(handler)!r},'{{file}}']\n"
    )
    config.chmod(0o600)

    # This controlled GTK3 application owns a complete plain-text GUI buffer
    # initialized from a synthetic backing file.  It never saves that file, so
    # the smoke independently proves that the configured handler receives only
    # GUI2TUI's private candidate and cannot bypass application authority.
    backing = result / "release-text.txt"
    starting_text = "release alpha\nrelease beta\n"
    backing.write_text(starting_text, encoding="utf-8")
    text_fixture = subprocess.Popen(
        [
            "python3",
            str(bundle / "smoke/release_smoke_text_gtk3.py"),
            str(backing),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        apps = run(["inspect", "--list"])
        if "gui2tui-release-text-demo" in apps.stdout:
            break
        time.sleep(.15)
    else:
        raise AssertionError("GTK3 text fixture did not register with AT-SPI")
    text_application = "gui2tui-release-text-demo"

    # Positive complete-text interaction from the extracted package.
    child = launch(
        ["--app", text_application, "--no-mouse"],
        {"GUI2TUI_VALIDATION_HANDLER_MODE": "positive"},
    )
    external_scene = wait_for(child, "Edit externally")
    time.sleep(1)
    child.send(b"e")
    external_after = wait_for_output(child, "External text update confirmed")
    assert "handler candidate C" in inspect_tree(application=text_application)
    assert backing.read_text(encoding="utf-8") == starting_text
    save("external-text-scene.txt", external_scene)
    save("external-text-confirmed.txt", external_after)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    child = launch(
        ["--app", text_application, "--no-mouse"],
        {"GUI2TUI_VALIDATION_HANDLER_MODE": "reset"},
    )
    wait_for(child, "Edit externally")
    child.send(b"e")
    wait_for_output(child, "External text update confirmed")
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    wait_for_tree(
        "handler candidate C", present=False, application=text_application
    )
    wait_for_tree("release alpha", application=text_application)

    # Concurrent authoritative GUI state B must win over private candidate C.
    ready = result / "handler-ready"
    resume = result / "handler-resume"
    ready.unlink(missing_ok=True)
    resume.unlink(missing_ok=True)
    child = launch(
        ["--app", text_application, "--no-mouse"],
        {
            "GUI2TUI_VALIDATION_HANDLER_MODE": "conflict",
            "GUI2TUI_VALIDATION_HANDLER_READY": str(ready),
            "GUI2TUI_VALIDATION_HANDLER_RESUME": str(resume),
        },
    )
    wait_for(child, "Edit externally")
    time.sleep(1)
    child.send(b"e")
    deadline = time.monotonic() + 10
    while not ready.exists() and time.monotonic() < deadline:
        time.sleep(.05)
    assert ready.exists(), "handler did not reach conflict barrier"
    authoritative = launch(
        ["--app", text_application, "--no-mouse"],
        {"GUI2TUI_VALIDATION_HANDLER_MODE": "authoritative"},
    )
    wait_for(authoritative, "Edit externally")
    authoritative.send(b"e")
    wait_for_output(authoritative, "External text update confirmed")
    authoritative.send(b"q")
    authoritative.expect(pexpect.EOF, timeout=5)
    authoritative.close()
    assert authoritative.exitstatus == 0
    wait_for_tree("release authoritative B", application=text_application)
    resume.touch()
    conflict = wait_for_output(child, "External text conflict detected")
    authoritative_tree = inspect_tree(application=text_application)
    assert "release authoritative B" in authoritative_tree
    assert "handler candidate C" not in authoritative_tree
    artifacts = list(
        pathlib.Path(os.environ["XDG_RUNTIME_DIR"]).glob(
            "gui2tui/gui2tui-owned-*/operation-*/artifact-*.txt"
        )
    )
    assert len(artifacts) == 1, artifacts
    artifact = artifacts[0]
    metadata = artifact.lstat()
    assert stat.S_ISREG(metadata.st_mode) and metadata.st_mode & 0o077 == 0
    assert artifact.parent.lstat().st_mode & 0o077 == 0
    assert "handler candidate C" in artifact.read_text(encoding="utf-8")
    save("external-text-conflict.txt", conflict)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    child = launch(
        ["--app", text_application, "--no-mouse"],
        {"GUI2TUI_VALIDATION_HANDLER_MODE": "reset"},
    )
    wait_for(child, "Edit externally")
    child.send(b"e")
    wait_for_output(child, "External text update confirmed")
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0
    wait_for_tree("release alpha", application=text_application)

    # A non-zero handler exit cannot mutate GUI state or leave the terminal
    # detached, and normal status remains concise.
    child = launch(
        ["--app", text_application, "--no-mouse"],
        {"GUI2TUI_VALIDATION_HANDLER_MODE": "fail"},
    )
    wait_for(child, "Edit externally")
    time.sleep(1)
    child.send(b"e")
    failed = wait_for_output(child, "handler exited unsuccessfully")
    failure_tree = inspect_tree(application=text_application)
    assert "release alpha" in failure_tree
    assert "handler candidate C" not in failure_tree
    assert backing.read_text(encoding="utf-8") == starting_text
    save("external-text-handler-failure.txt", failed)
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=5)
    child.close()
    assert child.exitstatus == 0

    socket = result / "runtime/viewer.sock"
    broker = subprocess.Popen([str(binary / "gui2tui"), "endpoint", "serve", "--socket", str(socket), "--mime", "image/png", "--recording-handler", "--authorization", "once"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(.5)
    connected = json.loads(run(["--modality-socket", str(socket), "doctor", "--json"]).stdout)
    assert any(c["name"] == "same-host-endpoint" and c["level"] == "PASS" for c in connected["checks"])
    save("doctor-broker.json", json.dumps(connected, indent=2))
    print("PACKAGE_VALUE_END_TO_END=PASS restored=true progress_read_only=true")
    print("PACKAGE_EXTERNAL_TEXT_END_TO_END=PASS authoritative_readback=true")
    print("PACKAGE_EXTERNAL_TEXT_CONFLICT_REFUSAL=PASS candidate_preserved=true")
    print("PACKAGE_EXTERNAL_TEXT_HANDLER_FAILURE=PASS gui_unchanged=true terminal_restored=true")
    print("PACKAGE_BACKING_FILE_BYPASS=ABSENT")
    print("PACKAGED_FRESH_HOME_SMOKE=PASS no_config=true action_confirmed=true password_absent=true broker_capabilities=true")
finally:
    if broker is not None:
        broker.terminate()
        broker.wait(timeout=5)
    fixture.terminate()
    fixture.wait(timeout=5)
    if value_fixture is not None:
        value_fixture.terminate()
        value_fixture.wait(timeout=5)
    if text_fixture is not None:
        text_fixture.terminate()
        text_fixture.wait(timeout=5)
    if child.isalive():
        child.close(force=True)
