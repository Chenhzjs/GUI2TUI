#!/usr/bin/env python3
"""Development GUI task scenarios using only public AT-SPI semantics."""
import argparse
import json
import os
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


def actions(node):
    interface = node.queryAction()
    return [interface.getName(i) for i in range(interface.nActions)]


def invoke(node, action):
    interface = node.queryAction()
    indices = [i for i in range(interface.nActions) if interface.getName(i) == action]
    if len(indices) != 1:
        raise RuntimeError(f"action {action!r} is not unique: {actions(node)!r}")
    if not node.getState().contains(__import__('pyatspi').STATE_ENABLED):
        raise RuntimeError("target is disabled")
    if not interface.doAction(indices[0]):
        raise RuntimeError("Accessibility action was rejected")


def write_text(node, value):
    editable = node.queryEditableText()
    if not editable.setTextContents(value):
        raise RuntimeError("EditableText rejected setTextContents")


def run_firefox(app, url):
    address = find_unique(app, "combo box", "Search or enter address")
    write_text(address, url)
    invoke(address, "activate")
    return {"scenario": "firefox_address_navigate", "target_role": address.getRoleName(),
            "target_name": address.name, "action": "activate", "accepted": True, "url": url}


def run_mousepad(app, text):
    new_items = [n for n in children(app) if n.getRoleName() == "menu item" and n.name.strip() in {"New", "New File"}]
    if len(new_items) != 1:
        raise RuntimeError(f"expected one New menu item, got {len(new_items)}")
    invoke(new_items[0], "click")
    time.sleep(0.5)
    editors = [n for n in children(app) if n.getRoleName() in {"text", "text input", "document text"} and n.getState().contains(__import__('pyatspi').STATE_EDITABLE)]
    if len(editors) > 1:
        # Prefer the multiline document exposed by the new tab over a
        # transient single-line control such as a search field.
        multiline = [n for n in editors if n.getState().contains(__import__('pyatspi').STATE_MULTI_LINE)]
        if len(multiline) == 1:
            editors = multiline
    if len(editors) > 1:
        named = [n for n in editors if n.name]
        if len(named) == 1:
            editors = named
    if len(editors) > 1:
        focused = [n for n in editors if n.getState().contains(__import__('pyatspi').STATE_FOCUSED)]
        if len(focused) == 1:
            editors = focused
    if len(editors) != 1:
        raise RuntimeError(f"expected one editable document, got {[(n.getRoleName(), n.name, str(n.getState().getStates())) for n in editors]}")
    write_text(editors[0], text)
    return {"scenario": "mousepad_new_file_edit", "target_role": editors[0].getRoleName(),
            "action": "EditableText.setTextContents", "accepted": True, "text_length": len(text)}


def run_action(app, name, role, action, scenario):
    target = find_unique(app, role, name)
    invoke(target, action)
    return {"scenario": scenario,
            "target_role": role, "target_name": name, "action": action, "accepted": True}


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
    parser.add_argument("--scenario", choices=sorted(SCENARIOS[parser.parse_known_args()[0].application]), default=None)
    parser.add_argument("--url", default="https://example.com/")
    parser.add_argument("--text", default="GUI2TUI semantic editing\n")
    args = parser.parse_args()
    for path in ("/tmp/profile", "/tmp/runtime"):
        Path(path).mkdir(mode=0o700, exist_ok=True)
    Path("/tmp/data").mkdir(mode=0o700, exist_ok=True)
    Path("/tmp/data/sample.txt").write_text("Alpha paragraph.\nBeta paragraph.\n")
    pdf = b"%PDF-1.4\n1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj\n3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] >>endobj\nxref\n0 4\ntrailer<< /Root 1 0 R /Size 4 >>\n%%EOF\n"
    Path("/tmp/data/sample.pdf").write_bytes(pdf)
    os.environ.update(DISPLAY=":97", XDG_RUNTIME_DIR="/tmp/runtime", XDG_SESSION_TYPE="x11",
                      NO_AT_BRIDGE="0", GTK_A11Y="atspi", QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1",
                      MOZ_ACCESSIBILITY_ATSPI_ENABLED="1", LANG="C.UTF-8", LC_ALL="C.UTF-8")
    subprocess.Popen(["Xvfb", ":97", "-screen", "0", "1440x1000x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)
    subprocess.run(["dbus-update-activation-environment", "DISPLAY", "XDG_RUNTIME_DIR", "NO_AT_BRIDGE", "QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "GTK_A11Y"], check=True)
    for prop in ("IsEnabled", "ScreenReaderEnabled"):
        subprocess.run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                        "--object-path", "/org/a11y/bus", "--method",
                        "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", prop, "<true>"],
                       capture_output=True, check=True)
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
    if args.application == "firefox" and args.scenario in (None, "address_navigate"):
        result = run_firefox(app, args.url)
    elif args.application == "mousepad":
        result = run_mousepad(app, args.text) if args.scenario in (None, "new") else run_action(app, *SCENARIOS[args.application][args.scenario], args.scenario)
    else:
        result = run_action(app, *SCENARIOS[args.application][args.scenario or next(iter(SCENARIOS[args.application]))], args.scenario or next(iter(SCENARIOS[args.application])))
    result.update(application=args.application, process_returncode=proc.poll())
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
