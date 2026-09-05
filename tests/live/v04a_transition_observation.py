#!/usr/bin/env python3
"""Bounded live 0.4A checks inside an existing X11/AT-SPI session."""

import os
import pathlib
import re
import subprocess
import time

import pexpect


INSPECT = os.environ["INSPECT"]
GUI2TUI = os.environ["GUI2TUI"]
APPLICATION = "gui2tui-qt-fixture"
RUNTIME_DIR = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])


def inspect(*extra: str) -> str:
    return subprocess.check_output(
        [INSPECT, "--app", APPLICATION, *extra], text=True, env=os.environ
    )


def node_id(tree: str, role: str, label: str) -> str:
    match = re.search(rf'{re.escape(role)} "{re.escape(label)}".* id=([^ ]+)', tree)
    if not match:
        raise AssertionError(f"missing {role} {label!r}\n{tree}")
    return match.group(1)


def invoke(label: str, action: str = "Press") -> None:
    tree = inspect("--verbose")
    target = next(
        (
            node_id(tree, role, label)
            for role in ("Button", "MenuItem")
            if re.search(rf'{role} "{re.escape(label)}"', tree)
        ),
        None,
    )
    if target is None:
        raise AssertionError(f"missing action target {label!r}")
    subprocess.run(
        [INSPECT, "--action-name", target, action],
        check=True,
        env=os.environ,
        stdout=subprocess.DEVNULL,
    )


def tui(application: str = APPLICATION, event_capacity: int | None = None) -> pexpect.spawn:
    arguments = [
        "--app",
        application,
        "--layout",
        "flat",
        "--settle-ms",
        "500",
        "--no-mouse",
        "--log-level",
        "debug",
    ]
    if event_capacity is not None:
        arguments.extend(["--event-buffer-capacity", str(event_capacity)])
    child = pexpect.spawn(
        GUI2TUI,
        arguments,
        env=os.environ.copy(),
        encoding=None,
        dimensions=(42, 180),
    )
    child.expect(b"Username", timeout=12)
    return child


def reports() -> list[str]:
    log = RUNTIME_DIR / "gui2tui" / "product.log"
    if not log.exists():
        return []
    return [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "semantic transition observation completed" in line
    ]


def command(
    child: pexpect.spawn, query: str, condition: str, outcome: str, timeout: int = 8
) -> None:
    before = len(reports())
    child.send(b":" + query.encode() + b"\r")
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        current = reports()
        if len(current) > before and any(
            f'condition="{condition}"' in line and f"outcome={outcome}" in line
            for line in current[before:]
        ):
            return
        time.sleep(0.05)
    raise AssertionError(f"transition report missing: {condition=} {outcome=} {reports()}")


def finish(child: pexpect.spawn) -> str:
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)
    log = RUNTIME_DIR / "gui2tui" / "product.log"
    return log.read_text(encoding="utf-8")


menu = tui()
command(menu, "Tools", "exact-node-state", "Confirmed")
opened = inspect("--verbose")
assert re.search(r'Menu .*\bshowing\b', opened), opened
assert 'MenuItem "Activate Demo"' in opened, opened
invoke("Activate Demo")
assert "Status: menu activated" in inspect(), inspect()
menu_log = finish(menu)
assert "condition=\"exact-node-state\"" in menu_log, menu_log
assert "outcome=Confirmed" in menu_log, menu_log
print("TRANSITION_MENU_STATE_CONFIRMATION=PASS")


modal = tui()
command(modal, "Open modal dialog", "new-active-modal", "Confirmed", timeout=10)
scopes = inspect("--dump-scopes")
assert "ModalDialog" in scopes and "[ACTIVE]" in scopes, scopes
command(modal, "Close", "scope-inactive", "Confirmed", timeout=10)
restored = inspect("--dump-scopes")
active_line = next(line for line in restored.splitlines() if "[ACTIVE]" in line)
assert "Window" in active_line and "ModalDialog" not in active_line, restored
modal_log = finish(modal)
assert "condition=\"new-active-modal\"" in modal_log, modal_log
assert "condition=\"scope-inactive\"" in modal_log, modal_log
print("TRANSITION_MODAL_ENTER_CONFIRMATION=PASS")
print("TRANSITION_MODAL_EXIT_CONFIRMATION=PASS")


toggle = tui()
command(toggle, "Enable feature", "exact-node-state", "Confirmed")
toggle_tree = inspect("--verbose")
assert re.search(r'CheckBox "Enable feature".*\bchecked\b', toggle_tree), toggle_tree
toggle_log = finish(toggle)
assert "condition=\"exact-node-state\"" in toggle_log, toggle_log
assert "outcome=Confirmed" in toggle_log, toggle_log
no_event_reports = [
    line
    for line in toggle_log.splitlines()
    if "semantic transition observation completed" in line
    and "outcome=Confirmed" in line
    and "event_wakeups=0" in line
]
assert no_event_reports, toggle_log
print("TRANSITION_NO_EVENT_AUTHORITATIVE_CONFIRMATION=PASS")


unrelated = tui()
command(unrelated, "Activate safely", "new-active-modal", "Timeout", timeout=10)
unrelated_log = finish(unrelated)
timeout_reports = [
    line
    for line in unrelated_log.splitlines()
    if "semantic transition observation completed" in line
    and "condition=\"new-active-modal\"" in line
    and "outcome=Timeout" in line
]
assert timeout_reports, unrelated_log
assert any("event_wakeups=" in line and "event_wakeups=0" not in line for line in timeout_reports)
assert "Status: activated" in inspect(), inspect()
print("TRANSITION_UNRELATED_EVENT_REFUSAL=PASS")
print("TRANSITION_TIMEOUT_NO_FALSE_SUCCESS=PASS")


storm = tui("gui2tui-live-fixture", event_capacity=128)
command(storm, "Run accessibility event storm", "new-active-modal", "Timeout", timeout=12)
storm_tree = subprocess.check_output(
    [INSPECT, "--app", "gui2tui-live-fixture"], text=True, env=os.environ
)
assert "Storm complete: 2000" in storm_tree, storm_tree
storm_log = finish(storm)
storm_reports = [
    line
    for line in storm_log.splitlines()
    if "semantic transition observation completed" in line
    and 'condition="new-active-modal"' in line
    and "outcome=Timeout" in line
]
assert storm_reports, storm_log
storm_wakeups = [
    int(match.group(1))
    for line in storm_reports
    if (match := re.search(r"event_wakeups=(\d+)", line))
]
assert storm_wakeups and 0 < max(storm_wakeups) < 2000, storm_reports
print(f"TRANSITION_EVENT_BURST_WAKEUPS={max(storm_wakeups)}")
print("TRANSITION_EVENT_BURST_COALESCING=PASS")


replacement = tui()
replacement.send(b"\r")
replacement.expect(re.compile(br"Editing", re.S), timeout=5)
before = inspect("--verbose")
old_locator = node_id(before, "TextInput", "Username")
invoke("Replace username control")
deadline = time.monotonic() + 8
after = inspect("--verbose")
while time.monotonic() < deadline:
    candidate = node_id(after, "TextInput", "Username")
    if candidate != old_locator:
        break
    time.sleep(0.05)
    after = inspect("--verbose")
new_locator = node_id(after, "TextInput", "Username")
assert old_locator != new_locator, (old_locator, new_locator)
old_result = subprocess.run(
    [INSPECT, "--actions", old_locator],
    env=os.environ,
    text=True,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
assert old_result.returncode != 0, old_result.stdout
replacement.send(b"\x1b")
replacement.send(b"r")
replacement.send(b"\r-current\r")
replacement.expect(re.compile(br"Text.*update.*confirmed", re.S), timeout=10)
assert "replacement-qt-current" in inspect(), inspect()
finish(replacement)
print("TRANSITION_STALE_LOCATOR_AUTHORITY_REFUSAL=PASS")
print("PRESENTATION_ID_AUTHORITY_SEPARATION=PASS")
