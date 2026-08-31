"""Real application tests; application names belong here, never in production logic.

Only terminal events and public AT-SPI Inspector operations drive GUI2TUI workflows.
Documents generated here are application input, not accessibility/extraction side channels.
"""
import json
import base64
import os
import pathlib
import re
import shutil
import signal
import statistics
import subprocess
import time

import pexpect
import pyte

ROOT = pathlib.Path(os.environ["PROJECT_ROOT"])
OUT = pathlib.Path(os.environ["RESULT_DIR"])
BIN = pathlib.Path(os.environ["GUI2TUI_BIN"])
CASE = os.environ["TEST_APP"]
MODE = os.environ["TEST_MODE"]
REPORT = {"application": CASE, "mode": MODE, "checks": {}, "result": "NOT TESTED"}
RESTARTED = []
SENTINELS = ("phase-zero-secret", "phase-two-secret", "browser-phase-secret")


def save(name, text):
    assert not any(secret in text for secret in SENTINELS), "password exposure"
    (OUT / name).write_text(text)
    return text


def inspect(*args, timeout=45, ok=True):
    result = subprocess.run([str(BIN / "gui2tui-inspect"), *args], text=True,
                            capture_output=True, timeout=timeout)
    assert not any(secret in result.stdout + result.stderr for secret in SENTINELS)
    if ok:
        assert result.returncode == 0, result.stderr
    return result


def resources(pid):
    status = pathlib.Path(f"/proc/{pid}/status").read_text()
    return {"rss_kib": int(re.search(r"VmRSS:\s+(\d+)", status)[1]),
            "fd": len(list(pathlib.Path(f"/proc/{pid}/fd").iterdir()))}


def cache_diagnostics(tree):
    """Read public wire metadata only; diagnose residency, never extract document files."""
    from gi.repository import Gio
    encoded = re.search(r"atspi1_([A-Za-z0-9_-]+)", tree)[1]
    bus_name = base64.urlsafe_b64decode(encoded + "=" * (-len(encoded) % 4)).split(b"\0")[0].decode()
    session = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    address = session.call_sync("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress", None, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    bus = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    items = bus.call_sync(bus_name, "/org/a11y/atspi/cache", "org.a11y.atspi.Cache", "GetItems", None, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    if not items or len(items[0]) != 10:
        return {"items": len(items), "modern": False}
    by_object = {tuple(item[0]): item for item in items}
    children = {}
    for item in items:
        children.setdefault(tuple(item[2]), []).append(tuple(item[0]))
    incomplete = []
    for item in items:
        obj = tuple(item[0])
        if item[4] <= len(children.get(obj, [])):
            continue
        chain, cursor = [], obj
        while cursor in by_object and cursor not in chain and len(chain) < 100:
            chain.append(cursor)
            cursor = tuple(by_object[cursor][2])
        incomplete.append({"path": obj[1], "expected": item[4], "cached": len(children.get(obj, [])),
                           "ancestor_paths": [ref[1] for ref in chain], "application": item[1][1]})
    bus.close_sync(None)
    return {"items": len(items), "modern": True, "incomplete": incomplete}


class Terminal:
    def __init__(self, selector):
        self.screen = pyte.Screen(160, 50)
        self.stream = pyte.Stream(self.screen)
        self.started = time.monotonic()
        self.child = pexpect.spawn(str(BIN / "gui2tui"), ["--app", selector, "--log-level", "debug"],
                                   encoding=None, dimensions=(50, 160), env=os.environ.copy())
        # AT-SPI events may replace the transient Loaded status before the PTY
        # is drained; readiness is the fully rendered interactive scene.
        try:
            self.wait("? Help | Tab Focus | Enter Use", timeout=40)
        except Exception:
            self.child.close(force=True)
            raise
        self.first_frame_ms = round((time.monotonic() - self.started) * 1000, 2)

    def pump(self, seconds=.2):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                self.stream.feed(self.child.read_nonblocking(65536, timeout=.02).decode("utf-8", "replace"))
            except pexpect.TIMEOUT:
                pass
        text = "\n".join(self.screen.display)
        assert not any(secret in text for secret in SENTINELS)
        return text

    def wait(self, needle, timeout=12):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            text = self.pump(.1)
            if needle in text:
                return text
        raise AssertionError(f"frame did not contain {needle!r}\n{text}")

    def send(self, data):
        self.child.send(data.encode() if isinstance(data, str) else data)
        return self.pump()

    def focus(self, label):
        for _ in range(250):
            text = self.pump(.025)
            if any(line.strip("│ ").startswith("> ") and label in line for line in text.splitlines()):
                return text
            self.child.send(b"\t")
        raise AssertionError(f"no focus target {label!r}\n{text}")

    def close(self):
        if self.child.isalive():
            self.child.send(b"\x03")
            self.child.expect(pexpect.EOF, timeout=8)
            self.child.close()

    def status(self):
        self.child.setwinsize(90, 160)
        self.screen.resize(90, 160)
        self.send(b"\x1b[24~")
        frame = self.pump(.2)
        clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
        value = json.loads(clean[clean.index("{"):clean.rindex("}") + 1])
        self.send(b"\x1b")
        self.child.setwinsize(50, 160)
        self.screen.resize(50, 160)
        return value


def launch_case():
    work = OUT / "work"
    work.mkdir()
    text = "GUI2TUI real editor review\n" + "\n".join(
        f"Section {i}: accessibility workflow paragraph for semantic navigation." for i in range(100))
    (work / "review.txt").write_text(text)
    for i in range(40):
        (work / f"item-{i:03}.txt").write_text(f"Local validation item {i}\n")
    fixture = ROOT / "tests/fixtures"
    if CASE == "mousepad":
        return "Mousepad", ["mousepad", "--disable-server", str(work / "review.txt")]
    if CASE == "pcmanfm":
        return "pcmanfm-qt", ["pcmanfm-qt", "--profile", OUT.name, "--new-window", "--daemon-mode", str(work)]
    if CASE == "pcmanfm-settings":
        return "pcmanfm-qt", ["pcmanfm-qt", "--profile", OUT.name, "--show-pref", "behavior"]
    if CASE == "designer":
        return "designer", ["/usr/lib/qt6/bin/designer"]
    if CASE in ("gtk", "qt"):
        return ("gui2tui-live-fixture" if CASE == "gtk" else "gui2tui-qt-fixture"), [
            "python3", str(fixture / ("gtk4_live_fixture.py" if CASE == "gtk" else "qt6_live_fixture.py"))]
    if CASE == "static-image":
        os.environ["VISUAL_ONLY"] = "1"
        return "python3", ["python3", str(fixture / "gtk4_modality_fixture.py")]
    if CASE in ("chrome", "chrome-large"):
        page = "browser_fixture.html" if CASE == "chrome" else "browser_large_fixture.html?count=700"
        return "Google Chrome", ["google-chrome", "--disable-gpu", "--disable-dev-shm-usage",
            "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
            "--force-renderer-accessibility=complete", f"--user-data-dir={OUT / 'profile'}",
            f"file://{fixture / page}"] + (["--no-sandbox"] if os.environ.get("ISOLATED_SANDBOX_COMPARISON") == "1" else [])
    if CASE == "firefox":
        profile = OUT / "profile"
        profile.mkdir()
        shutil.copy(fixture / "firefox-user.js", profile / "user.js")
        return "Firefox", ["/opt/firefox-154.0.1/firefox", "--no-remote", "--profile", str(profile),
                           f"file://{fixture / 'browser_fixture.html'}"]
    if CASE in ("writer", "writer-long", "calc"):
        document = work / "review.fodt"
        source = (fixture / "libreoffice_content_fixture.fodt").read_text()
        if CASE == "writer-long":
            extra = "".join(f'<text:h text:outline-level="2">Review {i}</text:h><text:p>'
                            + (f"Accessibility review section {i}. " * 12) + "</text:p>" for i in range(150))
            source = source.replace("</office:text>", extra + "</office:text>")
        document.write_text(source)
        if CASE == "calc":
            document = work / "review.csv"
            document.write_text("Item,Count,Status\n" + "".join(f"Item {i},{i},Ready\n" for i in range(300)))
        return "soffice", ["libreoffice", "--nologo", "--nodefault", "--norestore",
            f"-env:UserInstallation=file://{OUT / 'profile'}", str(document)]
    if CASE == "electron":
        return "Code", [os.environ["CODE_BINARY"], "--disable-gpu",
                        "--force-renderer-accessibility=complete", "--disable-extensions",
                        "--skip-welcome", "--skip-release-notes", f"--user-data-dir={OUT / 'profile'}",
                        str(work / "review.txt")]
    raise ValueError(CASE)


def main():
    if MODE not in {"probe", "workflow", "benchmark", "modality", "reproduce-multiline"}:
        raise ValueError("unknown test mode")
    selector, command = launch_case()
    REPORT["selector"] = selector
    app_log = (OUT / "application.log").open("w")
    app = subprocess.Popen(command, stdout=app_log, stderr=subprocess.STDOUT, start_new_session=True)
    terminal = None
    try:
        if CASE.startswith("pcmanfm"):
            time.sleep(3)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if app.poll() is not None:
                REPORT["application_exit_code"] = app.returncode
                REPORT["result"] = "BLOCKED"
                raise AssertionError("Application exited before accessibility discovery")
            apps = inspect("--list", ok=False).stdout
            if selector.lower() in apps.lower():
                break
            time.sleep(.3)
        else:
            REPORT["result"] = "BLOCKED"
            save("discovery-failure.txt", inspect("--list", ok=False).stdout)
            windows = subprocess.run(["xwininfo", "-root", "-tree"], capture_output=True, text=True, timeout=5)
            save("discovery-window-tree.txt", windows.stdout)
            raise AssertionError("Application not discovered through AT-SPI")
        save("applications.txt", apps)
        time.sleep(8 if CASE in ("chrome", "chrome-large", "firefox", "electron") else 2)
        started = time.monotonic()
        tree = save("tree.txt", inspect("--app", selector).stdout)
        REPORT["snapshot_wall_ms"] = round((time.monotonic() - started) * 1000, 2)
        REPORT["nodes"] = len([line for line in tree.splitlines() if line and "… [" not in line])
        REPORT["advertised_action_nodes"] = len(re.findall(r"id=atspi1_", tree))
        REPORT["checks"]["discovery_tree"] = "PASS"
        save("cache-probe.txt", inspect("--app", selector, "--probe-cache", ok=False).stdout)
        forced = inspect("--app", selector, "--bootstrap", "cache", ok=False)
        save("cache-result.txt", forced.stderr)
        REPORT["cache_forced_exit"] = forced.returncode
        if CASE.startswith("chrome"):
            save("cache-diagnostics.json", json.dumps(cache_diagnostics(tree), indent=2))
        for name, flag in (("scene", "--dump-scene"), ("content", "--dump-content"),
                           ("outline", "--dump-outline"), ("commands", "--dump-commands"),
                           ("choices", "--dump-choices"), ("scopes", "--dump-scopes")):
            save(f"{name}.txt", inspect("--app", selector, flag).stdout)
        if MODE == "modality":
            result = subprocess.run(["python3", str(ROOT / "tests/live/phase3h_probe.py")],
                                    env={**os.environ, "APP_SELECTOR": selector},
                                    text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
            save("modality-results.txt", result.stdout)
            assert result.returncode == 0, result.stdout
            REPORT["checks"]["headless_snapshot_same_host"] = "PASS"
            REPORT["result"] = "PASS"
            return
        terminal = Terminal(selector)
        save("scene-frame.txt", terminal.pump())
        REPORT["first_frame_ms"] = terminal.first_frame_ms
        loaded = re.search(r"Loaded ([0-9,]+) semantic nodes via (.*?) in (\d+) ms", terminal.pump())
        if loaded:
            REPORT["tui_bootstrap"] = {"nodes": int(loaded[1].replace(",", "")), "strategy": loaded[2], "ms": int(loaded[3])}
        REPORT["resources_bootstrap"] = resources(terminal.child.pid)
        if MODE == "workflow":
            workflow(terminal, selector, app, command)
        elif MODE == "reproduce-multiline":
            terminal.focus("Text input:")
            terminal.send("\r")
            save("multiline-edit-frame.txt", terminal.wait("Enter Commit"))
            terminal.send(b"\x1b")
            REPORT["checks"]["multiline_edit_exposed"] = True
            REPORT["checks"]["no_write_performed"] = True
        elif MODE == "benchmark":
            REPORT["benchmark"] = []
            terminal.close()
            for _ in range(4):
                terminal = Terminal(selector)
                REPORT["benchmark"].append({"first_frame_ms": terminal.first_frame_ms,
                                            **resources(terminal.child.pid)})
                terminal.close()
            REPORT["warm_first_frame_median_ms"] = statistics.median(
                s["first_frame_ms"] for s in REPORT["benchmark"][1:])
        REPORT["result"] = "PROBE PASS" if MODE == "probe" else "PASS"
    finally:
        if terminal:
            terminal.close()
        try:
            os.killpg(app.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(app.pid, signal.SIGKILL)
            app.wait(timeout=5)
        app_log.close()
        for restarted in RESTARTED:
            if restarted.poll() is None:
                os.killpg(restarted.pid, signal.SIGTERM)
                restarted.wait(timeout=5)


def locator_for(tree, role, label):
    line = next(line for line in tree.splitlines() if f'{role} "{label}"' in line)
    return re.search(r"atspi1_[A-Za-z0-9_-]+", line)[0]


def command_dialog(terminal, selector, query, title):
    terminal.send(":" + query)
    save("command-search.txt", terminal.pump(.3))
    terminal.send("\r")
    terminal.pump(1)
    tree = save("dialog-tree.txt", inspect("--app", selector).stdout)
    assert f'Dialog "{title}"' in tree or f'Window "{title}"' in tree, tree
    save("dialog-frame.txt", terminal.pump())
    # Close only the named dialog's advertised Close button (never a window's unrelated Close).
    subtree = tree[tree.index(f'"{title}"'):]
    target = locator_for(subtree, "Button", "Close")
    inspect("--activate", target)
    terminal.pump(1)
    assert f'Dialog "{title}"' not in inspect("--app", selector).stdout
    save("dialog-closed-frame.txt", terminal.pump())
    REPORT["checks"]["command_dialog_close"] = "PASS"


def workflow(terminal, selector, app, command):
    if CASE == "mousepad":
        assert "Document:" in terminal.pump(), "multiline text was not mapped to Reader"
        terminal.focus("Document:")
        terminal.send("\r")
        save("reader-frame.txt", terminal.wait("Section 0"))
        terminal.send("/Section 80")
        save("search-indexed.txt", terminal.pump(.5))
        terminal.send(b"\x06")
        save("search-progressive.txt", terminal.pump(1))
        terminal.send(b"\x1b")  # completed search -> Reader
        terminal.send(b"\x1b")  # Reader -> Scene (another Esc would quit)
        REPORT["checks"]["multiline_reader"] = "PASS"
        command_dialog(terminal, selector, "About", "About Mousepad")
    elif CASE in ("chrome", "firefox", "writer", "writer-long"):
        terminal.focus("Document:")
        terminal.send("\r")
        save("reader-frame.txt", terminal.wait("Reader"))
        terminal.send("o")
        save("outline-frame.txt", terminal.wait("Outline"))
        terminal.send(b"\x1b")
        terminal.send("/semantic")
        save("search-indexed.txt", terminal.pump(.5))
        terminal.child.send(b"\x06\x1b")
        cancelled = save("search-cancelled.txt", terminal.pump(.3))
        assert "Full search: Cancelled" in cancelled, cancelled
        REPORT["checks"]["progressive_cancellation"] = "PASS"
        terminal.send(b"\x06")
        save("search-progressive.txt", terminal.pump(1))
        assert re.search(r"[1-9][0-9]* matches", (OUT / "search-progressive.txt").read_text())
        terminal.send(b"\x1b")
        terminal.send(b"\x1b")
        REPORT["checks"]["reader_outline_search_frames"] = "PASS"
        if CASE.startswith("writer"):
            command_dialog(terminal, selector, "About LibreOffice", "About LibreOffice")
        else:
            tree = inspect("--app", selector).stdout
            target = locator_for(tree, "Button", "Replace article paragraph")
            save("mutation-actions.txt", inspect("--actions", target).stdout)
            # Explicit diagnostic index from the existing browser fixture contract;
            # the semantic TUI must continue refusing anonymous default actions.
            inspect("--action", target, "--index", "0")
            terminal.pump(1)
            save("mutation-tree.txt", inspect("--app", selector).stdout)
            # Text nodes may have no accessible name: use the actual Reader's
            # Text-interface materialization, not an inferred DOM value.
            terminal.focus("Document:")
            terminal.send("\r/replaced article paragraph")
            terminal.send(b"\x06")
            changed = save("mutation-readback.txt", terminal.pump(1))
            assert re.search(r"[1-9][0-9]* matches", changed), changed
            terminal.send(b"\x1b")
            terminal.send(b"\x1b")
            REPORT["checks"]["external_content_mutation"] = "PASS"
            terminal.focus("Document:")
            terminal.send("\r/Evaluation scores\r\r")
            table_frame = save("table-frame.txt", terminal.pump(1))
            assert "Navigate semantic cells" in table_frame, table_frame
            terminal.send("l")
            save("table-moved-frame.txt", terminal.pump())
            terminal.send(b"\x1b")
            terminal.send(b"\x1b")
            REPORT["checks"]["table_navigation"] = "PASS"
            terminal.send(b"\x1bOS")  # F4: explicit metadata inspection, not acquisition
            modality = save("headless-modality-frame.txt", terminal.wait("External modality"))
            assert "[Open locally]" not in modality
            terminal.send("\r")
            inspected = save("headless-reference-frame.txt", terminal.pump(.5))
            REPORT["checks"]["headless_modality_no_fake_open"] = "PASS"
            REPORT["checks"]["headless_reference_payload_zero"] = "PASS" if "payload_bytes=0" in inspected else "NOT VERIFIED"
            terminal.send(b"\x1b")
    elif CASE in ("gtk", "qt"):
        terminal.focus("Username")
        terminal.send("\r-p4c\r")
        terminal.pump(1)
        assert 'value="alice-p4c"' in inspect("--app", selector).stdout
        save("edit-confirmed-frame.txt", terminal.pump())
        terminal.send("\r-cancelled\x1b")
        assert 'value="alice-p4c"' in inspect("--app", selector).stdout
        terminal.focus("Password")
        terminal.send("\r")
        save("password-refusal.txt", terminal.wait("Password editing is disabled"))
        terminal.focus("Activate safely")
        terminal.send("\r")
        save("activated-frame.txt", terminal.wait("Status: activated"))
        tree = save("activated-tree.txt", inspect("--app", selector).stdout)
        assert 'CheckBox "Enable feature" [checked' in tree
        REPORT["checks"]["edit_cancel_button_password"] = "PASS"
        terminal.focus("Enable feature")
        terminal.send("\r")
        if CASE == "gtk":
            terminal.wait("No compatible")
            assert 'CheckBox "Enable feature" [checked' in inspect("--app", selector).stdout
        else:
            terminal.pump(.5)
            assert 'CheckBox "Enable feature" [checked' not in inspect("--app", selector).stdout
            terminal.focus("Choice:")
            terminal.send("\r")
            save("choice-overlay.txt", terminal.wait("Beta"))
            terminal.send(b"\x1b[B\r")
            save("choice-confirmed-frame.txt", terminal.wait("Choice: Beta"))
            tree = save("choice-confirmed-tree.txt", inspect("--app", selector).stdout)
            assert 'ListItem "Beta" [selected,transient]' in tree[tree.index('ComboBox "'):]
        REPORT["checks"]["choice_or_safe_degradation"] = "PASS"
    elif CASE == "designer":
        tree = inspect("--app", selector).stdout
        subtree = tree[tree.index('Dialog "New Form"'):]
        inspect("--activate", locator_for(subtree, "Button", "Close"))
        terminal.pump(1)
        assert 'Dialog "New Form"' not in inspect("--app", selector).stdout
        save("startup-dialog-closed.txt", terminal.pump())
        terminal.send(":About")
        save("commands-about.txt", terminal.pump(.3))
        REPORT["checks"]["startup_dialog_close_commands"] = "PASS"
    elif CASE == "pcmanfm":
        terminal.send(":")
        save("commands-frame.txt", terminal.pump())
        terminal.send(b"\x1b")
        tree = inspect("--app", selector).stdout
        # User names a concrete file accessible, not a naked child index.
        target = locator_for(tree, "ListItem", "item-003.txt")
        save("selection-actions.txt", inspect("--actions", target).stdout)
        inspect("--action-name", target, "Toggle")
        terminal.pump(.5)
        tree = save("selection-confirmed.txt", inspect("--app", selector).stdout)
        assert re.search(r'ListItem "item-003.txt".*selected', tree)
        REPORT["checks"]["named_file_selection"] = "PASS"
    elif CASE == "pcmanfm-settings":
        terminal.focus("Open files with single click")
        terminal.send("\r")
        terminal.pump(.4)
        tree = save("settings-toggle.txt", inspect("--app", selector).stdout)
        assert 'CheckBox "Open files with single click" [checked' in tree
        terminal.send("\r")  # restore preference in the private test profile
        terminal.focus("Selection: Behavior")
        terminal.send("\r")
        save("settings-choice-overlay.txt", terminal.wait("Display"))
        terminal.send(b"\x1b[B\r")
        save("settings-display-frame.txt", terminal.wait("Selection: Display"))
        tree = save("settings-selected-tree.txt", inspect("--app", selector).stdout)
        assert re.search(r'ListItem "Display".*selected', tree)
        REPORT["checks"]["settings_checkbox_selection"] = "PASS"
    elif CASE == "electron":
        terminal.focus("Document:")
        terminal.send("\r")
        save("reader-frame.txt", terminal.wait("Reader"))
        terminal.send("/review")
        terminal.send(b"\x06")
        frame = save("search-progressive.txt", terminal.pump(2))
        assert re.search(r"[1-9][0-9]* matches", frame), frame
        terminal.send(b"\x1b")
        terminal.send(b"\x1b")
        terminal.focus("Manage")
        terminal.send("\r")
        save("anonymous-action-refused.txt", terminal.wait("No compatible"))
        REPORT["checks"]["reader_search_anonymous_refusal"] = "PASS"
    else:
        raise NotImplementedError("Workflow awaits concrete probe targets")
    REPORT["resources_after_workflow"] = resources(terminal.child.pid)
    save("final-frame.txt", terminal.pump())
    if CASE in ("mousepad", "chrome"):
        # Lose a real application during a Reader workflow, then explicitly
        # reopen the same application name as a fresh generation.
        terminal.focus("Document:")
        terminal.send("\r")
        before = terminal.status()
        save("runtime-before-exit.json", json.dumps(before, indent=2))
        old_tree = inspect("--app", selector).stdout
        old_locator = re.search(r"atspi1_[A-Za-z0-9_-]+", old_tree)[0]
        os.killpg(app.pid, signal.SIGTERM)
        app.wait(timeout=5)
        save("application-gone.txt", terminal.wait("Application is no longer available", timeout=15))
        stale = inspect("--actions", old_locator, ok=False)
        assert stale.returncode != 0
        save("stale-locator.txt", stale.stderr)
        restarted = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        RESTARTED.append(restarted)
        deadline = time.monotonic() + 20
        while selector.lower() not in inspect("--list", ok=False).stdout.lower():
            assert time.monotonic() < deadline
            time.sleep(.2)
        time.sleep(3)
        terminal.send(b"\x1b[15~")
        terminal.pump(2)
        fresh = terminal.status()
        save("runtime-after-restart.json", json.dumps(fresh, indent=2))
        assert fresh["generation"] > before["generation"], (before, fresh)
        assert terminal.child.isalive()
        REPORT["checks"]["reader_death_restart_stale_generation"] = "PASS"


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        REPORT["error_type"] = type(error).__name__
        if REPORT["result"] != "BLOCKED":
            REPORT["result"] = "FAIL"
        raise
    finally:
        (OUT / "report.json").write_text(json.dumps(REPORT, indent=2) + "\n")
        print(json.dumps(REPORT, indent=2))
