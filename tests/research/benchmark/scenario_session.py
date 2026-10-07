#!/usr/bin/env python3
"""Development GUI task scenarios using only public AT-SPI semantics."""
import argparse
import hashlib
import json
import os
import re
import ctypes
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
import subprocess
import time
from pathlib import Path

COMMANDS = {
    "firefox": ["firefox-esr", "--no-remote", "--profile", "/tmp/profile", "about:blank"],
    "mousepad": ["mousepad", "--disable-server"],
    "featherpad": ["featherpad", "/tmp/data/sample.txt"],
    "okular": ["okular", "/tmp/data/sample.pdf"],
}


def children(root):
    queue = [root]
    while queue:
        node = queue.pop(0)
        yield node
        try:
            queue.extend(node[i] for i in range(node.childCount) if node[i] is not None)
        except Exception:
            continue


def find_unique(app, role, name=None):
    matches = [n for n in children(app) if n.getRoleName() == role and (name is None or n.name == name)]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {role} {name!r}, got {len(matches)}")
    return matches[0]


class Terminal:
    """All writes go to the real GUI2TUI PTY, never to the GUI provider."""
    def __init__(self, binary, app, evidence):
        import pexpect
        import pyte
        self.pexpect = pexpect
        self.screen = pyte.Screen(320, 50)
        self.stream = pyte.Stream(self.screen)
        self.evidence = evidence
        self.transcript = ""
        self.child = pexpect.spawn(binary, ["--log-level", "debug", "--session", "desktop", "--layout", os.environ.get("GUI2TUI_TEST_LAYOUT", "spatial"), "--app", app],
                                  encoding="utf-8", dimensions=(50, 320), timeout=1)
        self.wait("? Help", 40)

    def pump(self, seconds=.15):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                chunk = self.child.read_nonblocking(65536, timeout=.03)
                self.transcript += chunk
                self.stream.feed(chunk)
            except self.pexpect.TIMEOUT:
                pass
        return "\n".join(self.screen.display)

    def wait(self, text, timeout=8):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if text in self.pump():
                return
        raise RuntimeError(f"GUI2TUI did not display {text!r}")

    def send(self, keys):
        self.child.send(keys)
        return self.pump()

    def command(self, name):
        self.send(":" + name.strip())
        self.wait("Command palette")
        self.send("\x1bOQ")  # F2: explicitly include all application commands
        for _ in range(6):
            frame = self.pump()
            lines = frame.splitlines()
            header = next(i for i, line in enumerate(lines) if "┌ Command palette " in line)
            left = lines[header].index("┌ Command palette ")
            right = lines[header].index("┐", left)
            if "0 commands" in frame:
                raise RuntimeError(f"GUI2TUI command unavailable: {name}")
            self.evidence.append({"palette_frame": frame})
            selected = []
            for line in lines[header + 2:]:
                part = line[left + 1:right].strip()
                if "└" in line[left:left + 1]:
                    break
                if part.startswith("> "):
                    selected.append(part[2:])
                elif selected and not selected[-1].endswith(" › " + name.strip()):
                    selected[-1] += " " + part
            if any(label.split(" › ")[-1].strip() == name.strip() for label in selected):
                self.evidence.append({"command": name.strip(), "frame": frame})
                self.send("\r")
                return
            self.send("\x1b[B")
        raise RuntimeError(f"no exact selected GUI2TUI command {name!r}")

    def focus(self, label):
        for _ in range(8):
            for _ in range(16):
                frame = self.pump(.03)
                if any(any(part.strip().startswith("> ") and label in part for part in line.split("│"))
                       for line in frame.splitlines()):
                    return
                self.send("\t")
            self.send("\x1b[17~")
        raise RuntimeError(f"GUI2TUI focus target unavailable: {label}")

    def close(self, output):
        output.mkdir(parents=True, exist_ok=True)
        (output / "terminal.ansi").write_text(self.transcript)
        (output / "terminal.txt").write_text("\n".join(self.screen.display))
        self.child.close(force=True)
        for log in Path("/tmp/runtime").rglob("product.log"):
            (output / "product.log").write_bytes(log.read_bytes())


def observe(app):
    """Test-side read-only oracle. No Action/EditableText/Component mutation."""
    rows = []
    for n in children(app):
        n.clearCache()
        role = n.getRoleName()
        row = {"role": role, "name": n.name, "states": [str(x) for x in n.getState().getStates()], "path": n.path, "interfaces": list(n.get_interfaces()), "child_count": n.childCount}
        row["ancestors"] = []
        parent = n.parent
        for _ in range(12):
            if parent is None:
                break
            row["ancestors"].append({"role": parent.getRoleName(), "name": parent.name})
            parent = parent.parent
        try:
            action = n.queryAction()
            row["actions"] = [action.getName(i) for i in range(action.nActions)]
        except NotImplementedError:
            row["actions"] = []
        if role == "frame":
            try:
                import pyatspi
                rect = n.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                row["bounds"] = [rect.x, rect.y, rect.width, rect.height]
            except NotImplementedError:
                pass
        if "password" not in role:
            try:
                t = n.queryText()
                row["text"] = t.getText(0, min(t.characterCount, 4096))
                if any("MULTI_LINE" in st for st in row["states"]) and t.characterCount:
                    row["default_attributes"] = t.getDefaultAttributes()
                    row["first_attributes"] = t.getAttributes(0)
            except NotImplementedError:
                pass
        rows.append(row)
    return rows


SCENARIOS = {
    "firefox": {
        "address_navigate": ("Search or enter address", "combo box", "activate"),
        "new_tab": ("Open a new tab (Ctrl+T)", "push button", "press"),
        "reload": ("Reload", "push button", "press"),
        "list_tabs": ("List all tabs", "push button", "press"),
        "firefox_view": ("Firefox View", "toggle button", "press"),
    },
    "mousepad": {
        "find": ("Find", "push button", "click"),
        "new": ("New", "push button", "click"),
        "fullscreen": ("Fullscreen", "push button", "click"),
        "line_numbers": ("Line Numbers      ", "menu item", "click"),
        "word_wrap": ("Word Wrap      ", "menu item", "click"),
    },
    "featherpad": {
        "select_text": ("Select Text", "check box", "Toggle"),
        "new": ("New", "push button", "Press"),
        "reload": ("Reload", "push button", "Press"),
        "find": ("Find", "push button", "Press"),
        "side_pane": ("Side-Pane", "push button", "Press"),
    },
    "okular": {
        "sidebar": ("Show Sidebar", "check box", "Toggle"),
        "browse": ("Browse", "check box", "Toggle"),
        "highlighter": ("Highlighter", "check box", "Toggle"),
        "underline": ("Underline", "check box", "Toggle"),
        "zoom_in": ("Zoom In", "push button", "Press"),
    },
}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("application", choices=COMMANDS)
    parser.add_argument("--scenario")
    parser.add_argument("--url", default="data:text/html,<title>GUI2TUI destination</title><h1>GUI2TUI destination</h1>")
    parser.add_argument("--binary", required=True)
    parser.add_argument("--layout", choices=["flat", "spatial"], default="flat")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--text", default="GUI2TUI semantic editing\n")
    args = parser.parse_args()
    os.environ["GUI2TUI_TEST_LAYOUT"] = args.layout
    if args.scenario and args.scenario not in SCENARIOS[args.application]:
        parser.error("unknown scenario for application")
    for path in ("/tmp/profile", "/tmp/runtime"):
        Path(path).mkdir(mode=0o700, exist_ok=True)
    if args.application == "firefox":
        Path("/tmp/profile/user.js").write_text('user_pref("browser.startup.homepage_override.mstone", "ignore");\nuser_pref("datareporting.policy.firstRunURL", "");\nuser_pref("browser.shell.checkDefaultBrowser", false);\n')
    Path("/tmp/data").mkdir(mode=0o700, exist_ok=True)
    Path("/tmp/data/sample.txt").write_text("Alpha paragraph.\nBeta paragraph.\n")
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>",
               b"<< /Length 0 >>\nstream\n\nendstream"]
    pdf = b"%PDF-1.4\n"
    offsets = [0]
    for i, obj in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf += f"{i} 0 obj\n".encode() + obj + b"\nendobj\n"
    xref = len(pdf)
    pdf += b"xref\n0 5\n0000000000 65535 f \n"
    pdf += b"".join(f"{n:010} 00000 n \n".encode() for n in offsets[1:])
    pdf += f"trailer\n<< /Root 1 0 R /Size 5 >>\nstartxref\n{xref}\n%%EOF\n".encode()
    Path("/tmp/data/sample.pdf").write_bytes(pdf)
    os.environ.update(DISPLAY=":97", XDG_RUNTIME_DIR="/tmp/runtime", XDG_SESSION_TYPE="x11",
                      NO_AT_BRIDGE="0", GTK_A11Y="atspi", QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1",
                      TERM="xterm-256color", MOZ_ACCESSIBILITY_ATSPI_ENABLED="1", LANG="C.UTF-8", LC_ALL="C.UTF-8")
    subprocess.Popen(["Xvfb", ":97", "-screen", "0", "1440x1000x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    xlib = ctypes.CDLL("libX11.so.6")
    xlib.XOpenDisplay.restype = ctypes.c_void_p
    xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    for _ in range(50):
        display = xlib.XOpenDisplay(None)
        if display:
            xlib.XCloseDisplay(display)
            break
        time.sleep(.1)
    subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)
    subprocess.run(["dbus-update-activation-environment", "DISPLAY", "XDG_RUNTIME_DIR", "NO_AT_BRIDGE", "QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "GTK_A11Y"], check=True)
    for prop in ("IsEnabled", "ScreenReaderEnabled"):
        subprocess.run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                        "--object-path", "/org/a11y/bus", "--method",
                        "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", prop, "<true>"],
                       capture_output=True, check=True)
    if (args.application == "mousepad" and args.scenario in (None, "new")) or (args.application == "featherpad" and args.scenario == "reload"):
        config = Path("/tmp/config/gui2tui")
        config.mkdir(parents=True, exist_ok=True)
        editor = Path("/tmp/task-editor.py")
        editor.write_text("import pathlib,sys\npathlib.Path(sys.argv[1]).write_text(" + repr(args.text) + ")\n")
        (config / "config.toml").write_text('[interaction.complex_text]\nprogram="/usr/bin/python3"\nargs=["/tmp/task-editor.py", "{file}"]\n')
        os.environ["XDG_CONFIG_HOME"] = "/tmp/config"
    reload_requests = []
    if args.application == "firefox" and args.scenario == "reload":
        class Page(BaseHTTPRequestHandler):
            def do_GET(self):
                reload_requests.append(self.path)
                body = ("<title>Reload fixture</title><h1>Load " + str(len(reload_requests)) + "</h1>").encode()
                self.send_response(200)
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(body)
            def log_message(self, *args):
                pass
        server = HTTPServer(("127.0.0.1", 8765), Page)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        COMMANDS["firefox"][-1] = "http://127.0.0.1:8765/"
    import pyatspi
    proc = subprocess.Popen(COMMANDS[args.application], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    desktop = pyatspi.Registry.getDesktop(0)
    started = time.monotonic()
    app = None
    while time.monotonic() - started < 30:
        apps = [a for a in desktop if a is not None and a.childCount > 0]
        if len(apps) == 1:
            app = apps[0]
            break
        time.sleep(0.25)
    if app is None:
        raise RuntimeError("expected one accessible application")
    scenario = args.scenario or next(iter(SCENARIOS[args.application]))
    steps = []
    terminal = None
    result = {"application": args.application, "scenario": scenario,
              "driver": "gui2tui_pty", "layout": args.layout, "task_completed": False, "steps": steps,
              "binary_sha256": hashlib.sha256(Path(args.binary).read_bytes()).hexdigest()}
    try:
        time.sleep(.5)
        before = observe(app)
        result["before"] = before
        terminal = Terminal.__new__(Terminal)
        terminal.__init__(args.binary, app.name, steps)
        terminal.pump(3)
        request_count_before = len(reload_requests)
        name = SCENARIOS[args.application][scenario][0].strip()
        if scenario == "address_navigate":
            terminal.focus(name)
            terminal.send("\r")
            terminal.wait("[editing]")
            terminal.send(args.url)
            terminal.send("\x13")
            terminal.pump(2)
            terminal.send("\x1b\r")  # explicit GUI2TUI Raw Enter, not inferred Submit
        else:
            if args.application == "featherpad" and scenario == "reload":
                terminal.focus("Document:")
                terminal.send("e")
                terminal.pump(3)
                edited = observe(app)
                if not any(r.get("text") == args.text for r in edited):
                    raise RuntimeError("reload precondition: GUI2TUI text edit not confirmed")
            if scenario == "zoom_in":
                terminal.focus("Fit Width")
                terminal.send("\r")
                terminal.wait("[editing]")
                terminal.send("\x7f" * len("Fit Width"))
                terminal.send("100%")
                terminal.send("\x13")
                terminal.pump(3)
                before = observe(app)
                result["before"] = before
            if args.application == "mousepad" and scenario in ("line_numbers", "word_wrap", "fullscreen"):
                terminal.command("Document" if scenario == "word_wrap" else "View")
                terminal.pump(5)
            try:
                terminal.command(name)
            except RuntimeError as error:
                steps.append({"command_unavailable": str(error), "fallback": "find named control in GUI2TUI scene"})
                terminal.send("\x1b")
                terminal.focus(name)
                terminal.send("\r")
            if args.application == "mousepad" and scenario == "new":
                terminal.pump(.8)
                terminal.focus("Document:")
                terminal.send("e")
                terminal.pump(2)
        terminal.pump(8)
        if args.application == "featherpad" and scenario == "reload":
            pending = observe(app)
            if any(r["name"] == "Discard changes" and r["role"] == "push button" for r in pending):
                terminal.command("Discard changes")
                terminal.pump(4)
        after = observe(app)
        result.update(before=before, after=after, operation_dispatched=True)
        # A changed tree alone does not prove task completion.
        result["status"] = "dispatched_requires_task_assertion"
        if scenario == "address_navigate":
            result["task_completed"] = any(r["role"] == "document web" and r["name"] == "GUI2TUI destination" for r in after)
            result["assertion"] = "destination document title exposed by public Accessibility"
        elif args.application == "mousepad" and scenario == "new":
            result["task_completed"] = any(r.get("text") == args.text for r in after) and sum(r["role"] == "page tab" for r in after) > sum(r["role"] == "page tab" for r in before)
            result["assertion"] = "new tab and exact document text"
        elif scenario in ("line_numbers", "word_wrap", "select_text", "sidebar", "browse", "highlighter", "underline"):
            def checked(rows):
                return [any("CHECKED" in state for state in r["states"]) for r in rows if r["name"].strip() == name and r["role"] == SCENARIOS[args.application][scenario][1] and (scenario != "underline" or any(a["name"] == "Annotation Toolbar" for a in r["ancestors"]))]
            old, new = checked(before), checked(after)
            result["task_completed"] = len(old) == len(new) == 1 and old != new
            result["assertion"] = "exact named control checked state changed"
        elif scenario in ("new_tab", "new"):
            result["task_completed"] = sum(r["role"] == "page tab" for r in after) > sum(r["role"] == "page tab" for r in before)
            result["assertion"] = "new public page tab"
        elif args.application == "firefox" and scenario == "reload":
            result["task_completed"] = request_count_before > 0 and len(reload_requests) > request_count_before
            result["assertion"] = "local HTTP fixture received a fresh reload request"
            result["request_counts"] = [request_count_before, len(reload_requests)]
        elif args.application == "featherpad" and scenario == "reload":
            result["task_completed"] = any(r.get("text") == "Alpha paragraph.\nBeta paragraph.\n" for r in after) and not any(r.get("text") == args.text for r in after)
            result["assertion"] = "reload restores disk fixture after GUI2TUI unsaved edit"
        elif scenario == "fullscreen":
            old = [r.get("bounds") for r in before if r["role"] == "frame"]
            new = [r.get("bounds") for r in after if r["role"] == "frame"]
            result["task_completed"] = len(old) == len(new) == 1 and old != new and new[0] == [0, 0, 1440, 1000]
            result["assertion"] = "public window bounds changed to Xvfb screen bounds"
        elif scenario == "list_tabs":
            result["task_completed"] = any(r["role"] == "panel" and r["name"] == "List all tabs" and any("SHOWING" in s for s in r["states"]) for r in after)
            result["assertion"] = "public tab list panel is showing"
        elif scenario == "firefox_view":
            result["task_completed"] = any(r["role"] == "document web" and r["name"] == "Firefox View" for r in after)
            result["assertion"] = "Firefox View document exposed"
        elif scenario == "side_pane":
            def pane_items(rows):
                return [r for r in rows if r["role"] == "list item" and r["name"] == "sample.txt" and any(a["role"] == "split pane" for a in r["ancestors"])]
            result["task_completed"] = not pane_items(before) and len(pane_items(after)) == 1
            result["assertion"] = "document side pane exposes sample.txt list item"
        elif scenario == "zoom_in":
            def zoom(rows):
                return [float(r["name"].strip('%').replace(',', '')) for r in rows if r["role"] == "combo box" and re.fullmatch(r"[0-9,.]+%", r["name"])]
            old, new = zoom(before), zoom(after)
            result["task_completed"] = len(new) == 1 and len(old) == 1 and new[0] > old[0]
            result["assertion"] = "public zoom percentage increased"
        elif scenario == "find":
            result["task_completed"] = any(any("FOCUSED" in st for st in r["states"]) and any("EDITABLE" in st for st in r["states"]) and not any("MULTI_LINE" in st for st in r["states"]) for r in after)
            result["assertion"] = "search focuses a public editable single-line control"
        if "assertion" in result:
            result["status"] = "passed" if result["task_completed"] else "assertion_failed"
    except Exception as error:
        result.update(status="failed", error=str(error))
        result["after"] = observe(app)
    finally:
        if terminal and hasattr(terminal, "child"):
            terminal.close(args.output)
        args.output.mkdir(parents=True, exist_ok=True)
        log = args.output / "product.log"
        if log.exists():
            result["backend_action_deliveries"] = [
                {"locator": match[0], "action_index": int(match[1]), "accepted": match[2] == "true"}
                for match in re.findall(r"public action delivery returned locator=(\S+) action_index=(\d+) accepted=(true|false)", log.read_text())
            ]
        (args.output / "result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({k:v for k,v in result.items() if k not in ("before", "after", "steps")}))


if __name__ == "__main__":
    main()
