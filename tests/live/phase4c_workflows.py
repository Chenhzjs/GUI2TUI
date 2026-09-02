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


def structural_noise_audit(scene, spatial_plan):
    """Classify rows from the flat scene using only presentation diagnostics."""
    presentation_by_node = {}
    rank = {"DiagnosticOnly": 0, "Empty": 0, "Structural": 1}
    for match in re.finditer(r"presentation=(\w+).*?nodes=\[([^]]*)\]", spatial_plan):
        kind = match[1]
        kind_rank = rank.get(kind, 2)
        for raw in re.findall(r"RuntimeNodeId\((\d+)\)", match[2]):
            node = int(raw)
            current = presentation_by_node.get(node)
            if current is None or kind_rank > current[0]:
                presentation_by_node[node] = (kind_rank, kind)
    counts = {"user_relevant": 0, "supporting": 0, "structural_only": 0,
              "diagnostic_only": 0, "duplicate_command_summary": 0}
    for line in scene.splitlines():
        if not line.lstrip().startswith("Element "):
            continue
        if "kind=CommandHeader" in line:
            counts["duplicate_command_summary"] += 1
            continue
        if "kind=Status" in line:
            counts["supporting"] += 1
            continue
        if "kind=Unsupported" not in line:
            counts["user_relevant"] += 1
            continue
        sources = [int(raw) for raw in re.findall(r"\d+", line.rsplit("sources=", 1)[-1])]
        kinds = {presentation_by_node[node][1] for node in sources
                 if node in presentation_by_node}
        if "Structural" in kinds:
            counts["structural_only"] += 1
        elif kinds - {"DiagnosticOnly", "Empty"}:
            counts["supporting"] += 1
        else:
            counts["diagnostic_only"] += 1
    counts["flat_rows"] = sum(counts.values())
    return counts


class Terminal:
    def __init__(self, selector):
        self.columns = int(os.environ.get("TERMINAL_COLUMNS", "160"))
        self.rows = int(os.environ.get("TERMINAL_ROWS", "50"))
        self.screen = pyte.Screen(self.columns, self.rows)
        self.stream = pyte.Stream(self.screen)
        self.transcript = ""
        self.loaded_status = None
        self.loaded_ms = None
        self.fallback_statuses = []
        self.started = time.monotonic()
        # Product diagnostics are optional for visual validation; individual
        # lifecycle tests may opt in when they need the private log.
        args = ["--app", selector, "--log-level",
                os.environ.get("GUI2TUI_TEST_LOG_LEVEL", "off")]
        layout = os.environ.get("GUI2TUI_LAYOUT")
        if layout:
            args.extend(["--layout", layout])
        self.child = pexpect.spawn(str(BIN / "gui2tui"), args,
                                   encoding=None, dimensions=(self.rows, self.columns), env=os.environ.copy())
        # AT-SPI events may replace the transient Loaded status before the PTY
        # is drained; readiness is the fully rendered interactive scene.
        try:
            ready = "? Help | F6 Region" if layout == "spatial" else "? Help | Tab Focus"
            self.wait(ready, timeout=40)
        except Exception:
            self.child.close(force=True)
            raise
        self.first_frame_ms = round((time.monotonic() - self.started) * 1000, 2)

    def pump(self, seconds=.2):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                chunk = self.child.read_nonblocking(65536, timeout=.02).decode("utf-8", "replace")
                self.transcript += chunk
                self.stream.feed(chunk)
            except pexpect.TIMEOUT:
                pass
        text = "\n".join(self.screen.display)
        if self.loaded_status is None:
            loaded = re.search(r"Loaded ([0-9,]+) semantic nodes via (.*?) in (\d+) ms", text)
            if loaded:
                self.loaded_status = loaded.groups()
                self.loaded_ms = round((time.monotonic() - self.started) * 1000, 2)
        fallback = next((line.strip() for line in text.splitlines() if "Full refresh fallback:" in line), None)
        if fallback and (not self.fallback_statuses or self.fallback_statuses[-1] != fallback):
            self.fallback_statuses.append(fallback)
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
        for region in range(8):
            for _ in range(32):
                text = self.pump(.025)
                if any((line.strip("│ ").startswith("> ") or f"[> {label}" in line)
                       and label in line for line in text.splitlines()):
                    return text
                self.child.send(b"\t")
            # Responsive spatial mode has region and local-control focus
            # levels. F6 is harmless in the legacy flat scene.
            self.child.send(b"\x1b[17~")
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
        self.child.setwinsize(self.rows, self.columns)
        self.screen.resize(self.rows, self.columns)
        return value


def launch_case():
    work = OUT / "work"
    work.mkdir()
    if os.environ.get("SPATIAL_UX_FIXTURE") == "1":
        text = """GUI2TUI v0.2 Review

Spatial reconstruction

The current prototype identifies important semantic regions and organizes them
into a terminal-native layout.

Accessibility boundaries

GUI2TUI consumes semantics exposed through AT-SPI. Missing or unreliable
information is degraded safely rather than guessed.

Next steps

Responsive region composition will improve navigation and adapt the interface
to smaller terminals.
"""
    else:
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
    if CASE == "eog":
        image = work / "viewer.png"
        shutil.copy("/usr/share/pixmaps/debian-logo.png", image)
        return "eog", ["eog", "--new-instance", str(image)]
    if CASE == "gtk-demo":
        return "gtk4-demo", ["gtk4-demo"]
    if CASE in ("gtk", "qt"):
        return ("gui2tui-live-fixture" if CASE == "gtk" else "gui2tui-qt-fixture"), [
            "python3", str(fixture / ("gtk4_live_fixture.py" if CASE == "gtk" else "qt6_live_fixture.py"))]
    if CASE == "static-image":
        os.environ["VISUAL_ONLY"] = "1"
        return "python3", ["python3", str(fixture / "gtk4_modality_fixture.py")]
    if CASE in ("chrome", "chrome-large"):
        page = "browser_fixture.html" if CASE == "chrome" else "browser_large_fixture.html?count=700"
        location = ("about:blank" if os.environ.get("PRELAUNCH_CACHE_LISTENER") == "1"
                    else f"file://{fixture / page}")
        return "Google Chrome", ["google-chrome", "--disable-gpu", "--disable-dev-shm-usage",
            "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
            "--force-renderer-accessibility=complete", f"--user-data-dir={OUT / 'profile'}",
            location] + (["--no-sandbox"] if os.environ.get("ISOLATED_SANDBOX_COMPARISON") == "1" else [])
    if CASE == "firefox":
        profile = OUT / "profile"
        profile.mkdir()
        shutil.copy(fixture / "firefox-user.js", profile / "user.js")
        return "Firefox", ["/opt/firefox-154.0.1/firefox", "--no-remote", "--profile", str(profile),
                           f"file://{fixture / 'browser_fixture.html'}"]
    if CASE in ("writer", "writer-long", "writer-settings", "calc"):
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
    if MODE not in {"probe", "workflow", "settings-probe", "benchmark", "cache-baseline", "fresh-benchmark", "modality", "reproduce-multiline"}:
        raise ValueError("unknown test mode")
    selector, command = launch_case()
    REPORT["selector"] = selector
    app_log = (OUT / "application.log").open("w")
    app = subprocess.Popen(command, stdout=app_log, stderr=subprocess.STDOUT, start_new_session=True)
    terminal = None
    cache_listener = None
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
        if MODE in ("cache-baseline", "fresh-benchmark"):
            if CASE == "chrome-large" and os.environ.get("PRELAUNCH_CACHE_LISTENER") == "1":
                listener_log = (OUT / "cache-listener.txt").open("w")
                cache_listener = subprocess.Popen(
                    [str(BIN / "gui2tui-inspect"), "--app", selector, "--watch-events"],
                    stdout=listener_log, stderr=subprocess.STDOUT, start_new_session=True)
                time.sleep(1)
                page = ROOT / "tests/fixtures/browser_large_fixture.html?count=700"
                open_result = subprocess.run(
                    ["google-chrome", f"--user-data-dir={OUT / 'profile'}", f"file://{page}"],
                    text=True, capture_output=True, timeout=15)
                save("cache-listener-open.txt", open_result.stdout + open_result.stderr)
            if CASE in ("chrome", "chrome-large", "firefox", "electron"):
                # Application discovery precedes completion of the bulk AT-SPI
                # Cache.  Sample the cache-ready condition only after the same
                # stabilization interval used by the normal workflow path.
                time.sleep(8)
            cache_before = save("cache-before.txt", inspect("--app", selector, "--probe-cache").stdout)
            terminal = Terminal(selector)
            fresh_frame = save("fresh-frame.txt", terminal.pump())
            fresh_resources = resources(terminal.child.pid)
            save("fresh-transcript.txt", terminal.transcript)
            loaded = terminal.loaded_status
            fallback_loaded = None
            if terminal.fallback_statuses:
                fallback_loaded = re.search(
                    r"([0-9,]+) nodes via (recursive walk|AT-SPI Cache) in (\d+) ms",
                    terminal.fallback_statuses[-1])
            if loaded is None and fallback_loaded:
                loaded = fallback_loaded.groups()
            REPORT["fresh"] = {
                "first_frame_ms": terminal.first_frame_ms,
                "initial_frame_ms": terminal.loaded_ms,
                "nodes": int(loaded[0].replace(",", "")) if loaded else None,
                "strategy": loaded[1] if loaded else None,
                "bootstrap_ms": int(loaded[2]) if loaded else None,
                "post_bootstrap_fallbacks": terminal.fallback_statuses,
                **fresh_resources,
            }
            assert "GUI2TUI" in fresh_frame
            assert not ("0 blocks" in fresh_frame and "completeness: Complete" in fresh_frame), fresh_frame
            REPORT["fresh"]["cache_items_before"] = int(re.search(r"items: (\d+)", cache_before)[1])
            forced = inspect("--app", selector, "--bootstrap", "cache", ok=False)
            save("cache-rejection.txt", forced.stderr)
            REPORT["fresh"]["cache_fallback_reason"] = forced.stderr.strip()
            if MODE == "fresh-benchmark" and os.environ.get("GUI2TUI_LAYOUT") == "spatial":
                spatial_evidence = save(
                    "spatial-evidence.txt",
                    inspect("--app", selector, "--dump-spatial-evidence").stdout)
                spatial_plan_result = inspect("--app", selector, "--dump-layout-plan")
                spatial_plan = save(
                    "spatial-plan.txt",
                    spatial_plan_result.stdout + spatial_plan_result.stderr)
                evidence_header = re.search(
                    r"nodes=(\d+) candidates=(\d+) requests=(\d+) successes=(\d+) failures=(\d+) rejected=(\d+) elapsed_ms=([0-9.]+)",
                    spatial_evidence)
                timing_header = re.search(
                    r"surface_ms=([0-9.]+) topology_ms=([0-9.]+) "
                    r"composition_ms=([0-9.]+) layout_ms=([0-9.]+)", spatial_plan)
                if evidence_header:
                    REPORT["spatial_evidence"] = {
                        "nodes": int(evidence_header[1]),
                        "candidates": int(evidence_header[2]),
                        "requests": int(evidence_header[3]),
                        "successes": int(evidence_header[4]),
                        "failures": int(evidence_header[5]),
                        "rejected": int(evidence_header[6]),
                        "elapsed_ms": float(evidence_header[7]),
                    }
                if timing_header:
                    REPORT["spatial_overhead_ms"] = {
                        "surface_inference": float(timing_header[1]),
                        "topology_inference": float(timing_header[2]),
                        "composition_planning": float(timing_header[3]),
                        "layout_compilation": float(timing_header[4]),
                    }
            if MODE == "fresh-benchmark":
                REPORT["checks"]["fresh_semantics_honest"] = "PASS"
                REPORT["result"] = "PASS"
                return
            deadline = time.monotonic() + 15
            attempts = 0
            forced = None
            while time.monotonic() < deadline:
                attempts += 1
                forced = inspect("--app", selector, "--bootstrap", "cache", ok=False)
                forced_nodes = len([line for line in forced.stdout.splitlines()
                                    if line and "… [" not in line])
                if forced.returncode == 0 and forced_nodes >= 5000:
                    break
                time.sleep(.25)
            save("cache-ready-stdout.txt", forced.stdout)
            save("cache-ready-stderr.txt", forced.stderr)
            save("cache-after.txt", inspect("--app", selector, "--probe-cache").stdout)
            REPORT["cache_ready_attempts"] = attempts
            REPORT["cache_ready"] = None
            if forced.returncode == 0 and forced_nodes >= 5000:
                fresh_terminal = terminal
                terminal = Terminal(selector)
                ready_frame = save("cache-ready-frame.txt", terminal.pump())
                ready_resources = resources(terminal.child.pid)
                save("cache-ready-transcript.txt", terminal.transcript)
                loaded = terminal.loaded_status
                REPORT["cache_ready"] = {
                    "first_frame_ms": terminal.first_frame_ms,
                    "initial_frame_ms": terminal.loaded_ms,
                    "nodes": int(loaded[0].replace(",", "")) if loaded else None,
                    "strategy": loaded[1] if loaded else None,
                    "bootstrap_ms": int(loaded[2]) if loaded else None,
                    **ready_resources,
                }
                assert "GUI2TUI" in ready_frame
                fresh_terminal.close()
            REPORT["checks"]["fresh_semantics_honest"] = "PASS"
            REPORT["checks"]["cache_ready_condition"] = (
                "PASS" if forced.returncode == 0 and forced_nodes >= 5000 else "UNAVAILABLE")
            REPORT["result"] = "PASS"
            return
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
        if os.environ.get("GUI2TUI_LAYOUT") == "spatial":
            spatial_evidence = save(
                "spatial-evidence.txt",
                inspect("--app", selector, "--dump-spatial-evidence").stdout)
            spatial_plan_result = inspect("--app", selector, "--dump-layout-plan")
            spatial_plan = save(
                "spatial-plan.txt",
                spatial_plan_result.stdout + spatial_plan_result.stderr)
            plan_header = re.search(
                r"composition=(\w+) regions=(\d+) leaves=(\d+) primary=(\d+) structural=(\d+)",
                spatial_plan)
            layout_timing_header = re.search(
                r"surface_ms=([0-9.]+) topology_ms=([0-9.]+) "
                r"composition_ms=([0-9.]+) layout_ms=([0-9.]+)", spatial_plan)
            evidence_header = re.search(
                r"nodes=(\d+) candidates=(\d+) requests=(\d+) successes=(\d+) failures=(\d+) rejected=(\d+) elapsed_ms=([0-9.]+)",
                spatial_evidence)
            if plan_header:
                REPORT["spatial_plan"] = {
                    "composition": plan_header[1], "regions": int(plan_header[2]),
                    "leaves": int(plan_header[3]), "primary": int(plan_header[4]),
                    "structural": int(plan_header[5])}
            if layout_timing_header:
                REPORT["layout_timings_ms"] = {
                    "surface_inference": float(layout_timing_header[1]),
                    "topology_inference": float(layout_timing_header[2]),
                    "composition_planning": float(layout_timing_header[3]),
                    "layout_compilation": float(layout_timing_header[4]),
                }
            reachability = re.search(r"actionable=(\d+) unplaced=(\d+)", spatial_plan)
            if reachability:
                REPORT["layout_reachability"] = {
                    "actionable": int(reachability[1]),
                    "unplaced": int(reachability[2])}
                assert int(reachability[2]) == 0, spatial_plan
                REPORT["checks"]["layout_reachability"] = "PASS"
            REPORT["structural_noise"] = structural_noise_audit(
                (OUT / "scene.txt").read_text(), spatial_plan)
            if evidence_header:
                REPORT["spatial_evidence"] = {
                    "nodes": int(evidence_header[1]), "candidates": int(evidence_header[2]),
                    "requests": int(evidence_header[3]), "successes": int(evidence_header[4]),
                    "failures": int(evidence_header[5]), "rejected": int(evidence_header[6]),
                    "elapsed_ms": float(evidence_header[7])}
            coverage = save(
                "presentation-coverage.txt",
                inspect("--app", selector, "--dump-presentation-coverage").stdout)
            missing = [int(node) for group in re.findall(r"^missing: \[(.*?)\]$", coverage, re.M)
                       for node in re.findall(r"SpatialRegionId\((\d+)\)", group)]
            assert not missing, coverage
            REPORT["presentation_coverage"] = {
                "terminal_classes": len(re.findall(r"^Terminal ", coverage, re.M)),
                "missing": len(missing),
            }
            REPORT["checks"]["presentation_coverage"] = "PASS"
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
        scene_frame = save("scene-frame.txt", terminal.pump())
        if os.environ.get("GUI2TUI_LAYOUT") == "spatial":
            assert "<Unsupported:" not in scene_frame, scene_frame
            assert "Primary Content" not in scene_frame, scene_frame
            assert "Supporting regions" not in scene_frame, scene_frame
            REPORT["checks"]["normal_scene_has_no_compiler_dump"] = "PASS"
        REPORT["first_frame_ms"] = terminal.first_frame_ms
        loaded = re.search(r"Loaded ([0-9,]+) semantic nodes via (.*?) in (\d+) ms", terminal.pump())
        if loaded:
            REPORT["tui_bootstrap"] = {"nodes": int(loaded[1].replace(",", "")), "strategy": loaded[2], "ms": int(loaded[3])}
        REPORT["resources_bootstrap"] = resources(terminal.child.pid)
        if MODE == "settings-probe":
            command_dialog(terminal, selector, "Options...", "Options - LibreOffice - User Data", close=False)
            tree = inspect("--app", selector, "--verbose").stdout
            target = locator_for(tree, "Cell", "General")
            save("settings-category-actions.txt", inspect("--actions", target).stdout)
            parent, child_index = selection_parent_and_index(tree, "Tree", "Cell", "General")
            save("settings-selection-strategy.txt",
                 f"parent={parent}\nchild=General\nchild_index={child_index}\n")
            inspect("--select-child", parent, "--child-index", str(child_index))
            terminal.pump(1)
            selected = save("settings-selected-tree.txt", inspect("--app", selector).stdout)
            assert 'Cell "General" [selected' in selected, selected

            checkbox_line = next(line for line in selected.splitlines()
                                 if 'CheckBox "Use data for document properties"' in line)
            initially_checked = "[checked" in checkbox_line
            terminal.focus("Use data for document properties")
            terminal.send("\r")
            terminal.pump(1)
            toggled = save("settings-toggled-tree.txt", inspect("--app", selector).stdout)
            toggled_line = next(line for line in toggled.splitlines()
                                if 'CheckBox "Use data for document properties"' in line)
            assert ("[checked" in toggled_line) != initially_checked, toggled_line
            terminal.send("\r")
            terminal.pump(1)
            restored = save("settings-restored-tree.txt", inspect("--app", selector).stdout)
            restored_line = next(line for line in restored.splitlines()
                                 if 'CheckBox "Use data for document properties"' in line)
            assert ("[checked" in restored_line) == initially_checked, restored_line

            cancel = locator_for(restored, "Button", "Cancel")
            inspect("--action-name", cancel, "Click")
            terminal.pump(1)
            closed = save("settings-closed-tree.txt", inspect("--app", selector).stdout)
            assert 'Dialog "Options - LibreOffice - User Data"' not in closed
            assert "Document:" in terminal.pump()
            REPORT["checks"]["settings_tree_navigation"] = "PASS"
            REPORT["checks"]["settings_checkbox_toggled_and_restored"] = "PASS"
            REPORT["checks"]["settings_dialog_context_restored"] = "PASS"
        elif MODE == "workflow":
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
        if cache_listener and cache_listener.poll() is None:
            os.killpg(cache_listener.pid, signal.SIGTERM)
            cache_listener.wait(timeout=5)
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


def selection_parent_and_index(tree, parent_role, child_role, child_label):
    """Resolve a named direct child through public tree structure, never a naked guessed index."""
    lines = tree.splitlines()
    for parent_at, parent_line in enumerate(lines):
        if not (re.search(rf"(?:^|[├└]── ){re.escape(parent_role)}(?: |$)", parent_line)
                and "interfaces=[" in parent_line and "Selection" in parent_line):
            continue
        marker = max(parent_line.find("├── "), parent_line.find("└── "))
        parent_depth = 0 if marker < 0 else marker
        child_depth = parent_depth + 4
        direct = []
        for line in lines[parent_at + 1:]:
            position = max(line.find("├── "), line.find("└── "))
            if position >= 0 and position <= parent_depth:
                break
            if position == child_depth:
                direct.append(line)
        target = next((index for index, line in enumerate(direct)
                       if f'{child_role} "{child_label}"' in line), None)
        if target is not None:
            parent = re.search(r"atspi1_[A-Za-z0-9_-]+", parent_line)[0]
            return parent, target
    raise LookupError(f"no Selection parent for {child_role} {child_label!r}")


def command_dialog(terminal, selector, query, title, close=True):
    terminal.send(":" + query)
    save("command-search.txt", terminal.pump(.3))
    terminal.send("\r")
    terminal.pump(1)
    tree = save("dialog-tree.txt", inspect("--app", selector).stdout)
    assert f'Dialog "{title}"' in tree or f'Window "{title}"' in tree, tree
    save("dialog-frame.txt", terminal.pump())
    if not close:
        return tree
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

        before_tabs = inspect("--app", selector).stdout
        new_line = next(line for line in before_tabs.splitlines()
                        if 'MenuItem "New      "' in line and "actions=[Click]" in line)
        new_tab = re.search(r"atspi1_[A-Za-z0-9_-]+", new_line)[0]
        inspect("--action-name", new_tab, "Click")
        after_new = save(
            "dynamic-tab-new-tree.txt", inspect("--app", selector, "--verbose").stdout)
        assert re.search(r'Tab "Untitled 1" \[[^]]*selected', after_new), after_new
        save("dynamic-tab-new-frame.txt", terminal.wait("Current · Untitled 1"))
        coverage = save(
            "dynamic-tab-new-coverage.txt",
            inspect("--app", selector, "--dump-presentation-coverage").stdout)
        assert "improperly collapsed: []" in coverage, coverage

        parent, child_index = selection_parent_and_index(
            after_new, "TabList", "Tab", "review.txt")
        inspect("--select-child", parent, "--child-index", str(child_index))
        after_switch = save("dynamic-tab-switched-tree.txt", inspect("--app", selector).stdout)
        assert re.search(r'Tab "review.txt" \[[^]]*selected', after_switch), after_switch
        old_context = next(line for line in after_switch.splitlines()
                           if 'Tab "Untitled 1"' in line)
        assert "selected" not in old_context, old_context
        save("dynamic-tab-switched-frame.txt", terminal.wait("Current · review.txt"))
        coverage = save(
            "dynamic-tab-switched-coverage.txt",
            inspect("--app", selector, "--dump-presentation-coverage").stdout)
        assert "improperly collapsed: []" in coverage, coverage
        terminal.child.send(b"\x1b[17~")
        save("dynamic-tab-f6-frame.txt", terminal.pump(.5))
        assert terminal.child.isalive()
        REPORT["checks"]["dynamic_current_context"] = "PASS"
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
        assert re.search(r"(?:Full|Exposed semantic) search: Cancelled", cancelled), cancelled
        if CASE == "writer-long":
            assert "Exposed semantic search: Cancelled" in cancelled, cancelled
            assert "Full search: Cancelled" not in cancelled, cancelled
        REPORT["checks"]["progressive_cancellation"] = "PASS"
        terminal.send(b"\x06")
        progressive = save("search-progressive.txt", terminal.pump(1))
        assert re.search(r"[1-9][0-9]* matches", progressive)
        if CASE == "writer-long":
            assert "Exposed semantic search:" in progressive, progressive
            assert "Full search complete" not in progressive, progressive
        terminal.send(b"\x1b")
        terminal.send(b"\x1b")
        REPORT["checks"]["reader_outline_search_frames"] = "PASS"
        if CASE.startswith("writer"):
            if CASE == "writer-long":
                before_content = save("realization-content-before.txt",
                                      inspect("--app", selector, "--dump-content").stdout)
                save("realization-outline-before.txt",
                     inspect("--app", selector, "--dump-outline").stdout)
                tree = inspect("--app", selector).stdout
                go_to_page = locator_for(tree, "MenuItem", "Go to Page...")
                save("go-to-page-actions.txt", inspect("--actions", go_to_page).stdout)
                invoked = inspect("--action-name", go_to_page, "Click", ok=False)
                save("go-to-page-invocation.txt", invoked.stdout + invoked.stderr)
                terminal.pump(1)
                page_tree = save("go-to-page-tree.txt", inspect("--app", selector).stdout)
                assert re.search(r'Dialog "[^"]*(?:Go to Page|Navigator)[^"]*"', page_tree), page_tree
                if 'TextInput' in page_tree:
                    terminal.focus("Text input:")
                    terminal.send("\r2\r")
                    terminal.pump(.5)
                    terminal.focus("OK")
                    terminal.send("\r")
                    terminal.pump(2)
                    navigation_result = "COMPLETED"
                else:
                    # LibreOffice 24.2 exposes the page selector as a Slider.
                    # GUI2TUI has no validated Slider mutation contract, so the
                    # honest semantic result is a safe, non-guessed cancel.
                    assert 'Slider value="1"' in page_tree, page_tree
                    cancel = locator_for(page_tree, "Button", "Cancel")
                    inspect("--action-name", cancel, "Click")
                    terminal.pump(1)
                    navigation_result = "SAFE DEGRADATION: page selector exposed only as Slider"
                after_content = save("realization-content-after.txt",
                                     inspect("--app", selector, "--dump-content").stdout)
                save("realization-outline-after.txt",
                     inspect("--app", selector, "--dump-outline").stdout)
                before_blocks = int(re.search(r"blocks=(\d+)", before_content)[1])
                after_blocks = int(re.search(r"blocks=(\d+)", after_content)[1])
                REPORT["writer_realization"] = {
                    "before_blocks": before_blocks,
                    "after_blocks": after_blocks,
                    "ordinary_navigation": "Go to Page...",
                    "navigation_result": navigation_result,
                }
                REPORT["checks"]["partial_search_wording"] = "PASS"
                REPORT["checks"]["ordinary_realization_attempt"] = "PASS"
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
    elif CASE == "eog":
        frame = terminal.pump()
        assert "Graphical content" in frame, frame
        assert "<Unsupported:" not in frame, frame
        command_dialog(terminal, selector, "About Image Viewer", "About Image Viewer")
        REPORT["checks"]["graphical_primary_normal_scene"] = "PASS"
        REPORT["checks"]["structural_noise_suppressed"] = "PASS"
    elif CASE == "gtk-demo":
        frame = terminal.pump()
        assert "GTK Demo" in frame, frame
        assert "<Unsupported:" not in frame, frame
        if "Document:" in frame:
            terminal.focus("Document:")
            terminal.send("\r")
            save("reader-frame.txt", terminal.wait("GTK Demo is a collection"))
            terminal.send(b"\x1b")
            REPORT["checks"]["semantic_detail_reader"] = "PASS"
        before = inspect("--app", selector).stdout
        if 'TextInput "Text input:"' not in before:
            search = locator_for(before, "ToggleButton", "Search")
            inspect("--action-name", search, "Click")
            terminal.pump(1)
        opened = save("search-opened-tree.txt", inspect("--app", selector).stdout)
        opened_search = next(line for line in opened.splitlines()
                             if 'ToggleButton "Search"' in line)
        assert "checked" in opened_search or "pressed" in opened_search, opened_search
        opened_scene = save(
            "search-opened-scene.txt", inspect("--app", selector, "--dump-scene").stdout)
        if "capability=EditText" in opened_scene:
            terminal.focus("Text input:")
            terminal.send("\rbutton\r")
            terminal.pump(1)
            after = save("search-toggle-tree.txt", inspect("--app", selector).stdout)
            assert after != before, "Search input produced no accessible state change"
            assert "TextInput" in after and 'value="button"' in after, after
            REPORT["checks"]["search_text_input"] = "PASS"
        else:
            assert 'TextInput "Search"' in opened, opened
            assert "capability=EditText" not in opened_scene, opened_scene
            REPORT["checks"]["search_text_input_safe_fallback"] = "PASS"
        search = locator_for(inspect("--app", selector).stdout, "ToggleButton", "Search")
        inspect("--action-name", search, "Click")
        hidden_frame = save("search-hidden-frame.txt", terminal.pump(1))
        hidden = save("search-hidden-tree.txt", inspect("--app", selector).stdout)
        hidden_search = next(line for line in hidden.splitlines()
                             if 'ToggleButton "Search"' in line)
        assert "checked" not in hidden_search and "pressed" not in hidden_search, hidden_search
        coverage = save(
            "search-hidden-coverage.txt",
            inspect("--app", selector, "--dump-presentation-coverage").stdout)
        assert "improperly collapsed: []" in coverage, coverage
        assert "GUI2TUI" in hidden_frame and terminal.child.isalive()
        REPORT["checks"]["dynamic_control_visibility"] = "PASS"
        REPORT["checks"]["normal_scene_grouping"] = "PASS"
    elif CASE == "designer":
        tree = inspect("--app", selector, "--verbose").stdout
        subtree = tree[tree.index('Dialog "New Form"'):]
        choice = locator_for(subtree, "ListItem", "QVGA portrait (240x320)")
        save("startup-choice-actions.txt", inspect("--actions", choice).stdout)
        inspect("--action-name", choice, "Toggle")
        terminal.pump(.5)
        selected = save("startup-choice-selected.txt", inspect("--app", selector).stdout)
        assert re.search(r'ListItem "QVGA portrait \(240x320\)" \[[^]]*selected', selected), selected

        checkbox = locator_for(selected[selected.index('Dialog "New Form"'):],
                               "CheckBox", "Show this Dialog on Startup")
        initially_checked = re.search(
            r'CheckBox "Show this Dialog on Startup" \[[^]]*checked', selected) is not None
        inspect("--action-name", checkbox, "Toggle")
        terminal.pump(.5)
        toggled = save("startup-checkbox-toggled.txt", inspect("--app", selector).stdout)
        assert (re.search(r'CheckBox "Show this Dialog on Startup" \[[^]]*checked', toggled)
                is not None) != initially_checked
        checkbox = locator_for(toggled[toggled.index('Dialog "New Form"'):],
                               "CheckBox", "Show this Dialog on Startup")
        inspect("--action-name", checkbox, "Toggle")
        terminal.pump(.5)

        create = locator_for(inspect("--app", selector).stdout, "Button", "Create")
        inspect("--action-name", create, "Press")
        terminal.pump(1)
        assert 'Dialog "New Form"' not in inspect("--app", selector).stdout
        terminal.send("r")
        save("form-created-frame.txt", terminal.pump(1))
        command_tree = inspect("--app", selector).stdout
        before_dialogs = set(re.findall(r'Dialog "([^"]+)"', command_tree))
        form_settings = locator_for(command_tree, "MenuItem", "Form Settings...")
        save("form-settings-actions.txt", inspect("--actions", form_settings).stdout)
        invoked = inspect("--action-name", form_settings, "Press", ok=False)
        save("form-settings-invocation.txt", invoked.stdout + invoked.stderr)
        terminal.pump(1)
        settings = save("form-settings-tree.txt", inspect("--app", selector).stdout)
        dialogs = [title for title in re.findall(r'Dialog "([^"]+)"', settings)
                   if title not in before_dialogs]
        assert dialogs, settings
        title = dialogs[-1]
        settings_subtree = settings[settings.rindex(f'"{title}"'):]
        close_role = "Cancel" if 'Button "Cancel"' in settings_subtree else "Close"
        close = locator_for(settings_subtree, "Button", close_role)
        close_actions = inspect("--actions", close).stdout
        close_action = "Press" if re.search(r"\bPress\b", close_actions) else "Click"
        inspect("--action-name", close, close_action)
        terminal.pump(1)
        assert title not in inspect("--app", selector).stdout
        REPORT["checks"]["real_choice_navigation"] = "PASS"
        REPORT["checks"]["real_form_control_toggle"] = "PASS"
        REPORT["checks"]["real_command_dialog_context"] = "PASS"
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
