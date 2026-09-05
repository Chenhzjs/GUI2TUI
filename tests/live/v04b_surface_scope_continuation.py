#!/usr/bin/env python3
"""Bounded live 0.4B surface/scope continuation checks under X11/AT-SPI."""

import os
import pathlib
import re
import subprocess
import time
from collections.abc import Callable

import pexpect


INSPECT = os.environ["INSPECT"]
GUI2TUI = os.environ["GUI2TUI"]
QT_APP = "gui2tui-qt-fixture"
GTK_APP = "gui2tui-live-fixture"
RUNTIME_DIR = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])


def inspect(application: str, *extra: str) -> str:
    return subprocess.check_output(
        [INSPECT, "--app", application, "--bootstrap", "walk", *extra],
        text=True,
        env=os.environ,
    )


def wait_for(read: Callable[[], str], predicate: Callable[[str], bool], timeout: float = 8) -> str:
    deadline = time.monotonic() + timeout
    last = ""
    while time.monotonic() < deadline:
        last = read()
        if predicate(last):
            return last
        time.sleep(0.05)
    raise AssertionError(f"authoritative condition was not reached\n{last}")


def node_id(tree: str, role: str, label: str) -> str:
    match = re.search(rf'{re.escape(role)} "{re.escape(label)}".* id=([^ ]+)', tree)
    if not match:
        raise AssertionError(f"missing {role} {label!r}\n{tree}")
    return match.group(1)


def unnamed_node_id(tree: str, role: str) -> str:
    match = re.search(rf'\b{re.escape(role)} \[.* id=([^ ]+)', tree)
    if not match:
        raise AssertionError(f"missing {role}\n{tree}")
    return match.group(1)


def find_action(application: str, label: str) -> str:
    tree = inspect(application, "--verbose")
    for role in ("Button", "MenuItem"):
        if re.search(rf'{role} "{re.escape(label)}"', tree):
            return node_id(tree, role, label)
    raise AssertionError(f"missing action target {label!r}\n{tree}")


def invoke(application: str, label: str, action: str = "Press") -> None:
    subprocess.run(
        [INSPECT, "--action-name", find_action(application, label), action],
        check=True,
        env=os.environ,
        stdout=subprocess.DEVNULL,
    )


def invoke_async(application: str, label: str, action: str = "Press") -> subprocess.Popen[str]:
    return subprocess.Popen(
        [INSPECT, "--action-name", find_action(application, label), action],
        env=os.environ,
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )


def tui(application: str) -> pexpect.spawn:
    child = pexpect.spawn(
        GUI2TUI,
        [
            "--app",
            application,
            "--layout",
            "flat",
            "--settle-ms",
            "700",
            "--no-mouse",
            "--log-level",
            "debug",
        ],
        env=os.environ.copy(),
        encoding=None,
        dimensions=(44, 180),
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


def terminal_output(child: pexpect.spawn) -> str:
    chunks: list[bytes] = []
    while True:
        try:
            chunks.append(child.read_nonblocking(size=65536, timeout=0))
        except pexpect.TIMEOUT:
            break
    rendered = b"".join(chunks).decode("utf-8", errors="replace")
    return re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", rendered)


def command(
    child: pexpect.spawn, query: str, condition: str, outcome: str, timeout: int = 10
) -> None:
    before = len(reports())
    open_palette(child, query)
    child.send(b"\r")
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        current = reports()
        if len(current) > before and any(
            f'condition="{condition}"' in line and f"outcome={outcome}" in line
            for line in current[before:]
        ):
            child.expect(b"confirmed" if outcome == "Confirmed" else b"deadline", timeout=5)
            return
        time.sleep(0.05)
    raise AssertionError(
        f"transition report missing: {query=} {condition=} {outcome=}\n"
        f"{reports()}\nTERMINAL:\n{terminal_output(child)}"
    )


def open_palette(child: pexpect.spawn, query: str) -> None:
    while True:
        try:
            child.read_nonblocking(size=4096, timeout=0)
        except pexpect.TIMEOUT:
            break
    child.send(b":")
    child.expect(b"Command palette", timeout=5)
    child.send(query.encode())
    # Input synchronization only; every success/refusal below is established
    # by fresh AT-SPI state or an explicit GUI2TUI result, never by this pause.
    time.sleep(0.2)


def expect_unavailable_status(child: pexpect.spawn) -> None:
    for word in (b"available", b"current", b"semantic", b"surface"):
        child.expect(word, timeout=8)


def finish(child: pexpect.spawn) -> None:
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)


def active_scope(application: str) -> tuple[str, str]:
    scopes = inspect(application, "--dump-scopes")
    active = next(line for line in scopes.splitlines() if "[ACTIVE]" in line)
    return active, scopes


def command_dump(application: str, query: str, all_scopes: bool = False) -> str:
    args = ["--dump-commands", "--command-query", query]
    if all_scopes:
        args.append("--all-scopes")
    return inspect(application, *args)


# Same-scope Qt menu: hidden -> current -> user-selected -> hidden.
assert "Activate Demo" not in command_dump(QT_APP, "Activate Demo")
menu = tui(QT_APP)
command(menu, "Tools", "exact-node-state", "Confirmed")
opened = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: bool(
        re.search(r'\bMenu \[.*\bshowing\b', tree)
        and re.search(r'MenuItem "Activate Demo".*\bshowing\b', tree)
    ),
)
menu_id = unnamed_node_id(opened, "Menu")
active, scopes = active_scope(QT_APP)
assert "Window" in active and "MenuPopup" not in scopes, scopes
relations = inspect(QT_APP, "--relations", menu_id)
assert "PopupFor" not in relations, relations
opened_commands = command_dump(QT_APP, "Activate Demo")
assert "Activate Demo" in opened_commands, (
    opened_commands
    + "\nALL SCOPES:\n"
    + command_dump(QT_APP, "Activate Demo", all_scopes=True)
    + "\nCURRENT TREE:\n"
    + inspect(QT_APP, "--verbose")
    + "\nCURRENT SCOPES:\n"
    + inspect(QT_APP, "--dump-scopes")
)
command(menu, "Activate Demo", "exact-surface-unavailable", "Confirmed")
closed = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Status: menu activated 1" in tree
    and not re.search(r'\bMenu \[.*\bshowing\b', tree),
)
assert "Activate Demo" in closed
assert "Activate Demo" not in command_dump(QT_APP, "Activate Demo")
finish(menu)
print("SAME_SCOPE_MENU_CONTINUATION=PASS")
print("TEMPORARY_VISIBILITY_ACTIONABILITY=PASS")
print("OWNERLESS_POPUP_NO_SCOPE_INFERENCE=PASS")


# A frozen menu palette entry loses authority when its surface disappears.
stale_menu = tui(QT_APP)
command(stale_menu, "Tools", "exact-node-state", "Confirmed")
open_palette(stale_menu, "Activate Demo")
invoke(QT_APP, "Activate Demo")
wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Status: menu activated 2" in tree
    and not re.search(r'\bMenu \[.*\bshowing\b', tree),
)
stale_menu.send(b"\r")
expect_unavailable_status(stale_menu)
assert "Status: menu activated 2" in inspect(QT_APP)
command(stale_menu, "Tools", "exact-node-state", "Confirmed")
command(stale_menu, "Activate Demo", "exact-surface-unavailable", "Confirmed")
assert "Status: menu activated 3" in inspect(QT_APP)
finish(stale_menu)
print("DISAPPEARED_SURFACE_STALE_BINDING_REFUSAL=PASS")


# Current Qt modal entry/exit and manual continuation.
modal = tui(QT_APP)
command(modal, "Open modal dialog", "new-active-modal", "Confirmed", timeout=12)
active, scopes = active_scope(QT_APP)
assert "ModalDialog" in active, scopes
assert "Activate safely" not in command_dump(QT_APP, "Activate safely")
command(modal, "Close", "scope-inactive", "Confirmed", timeout=12)
restored_scopes = wait_for(
    lambda: inspect(QT_APP, "--dump-scopes"),
    lambda text: any("Window" in line for line in text.splitlines() if "[ACTIVE]" in line),
)
active = next(line for line in restored_scopes.splitlines() if "[ACTIVE]" in line)
scopes = restored_scopes
assert "Window" in active and "ModalDialog" not in active, scopes
command(modal, "Enable feature", "exact-node-state", "Confirmed")
finish(modal)
print("MODAL_SCOPE_CONTINUATION_ENTER=PASS")
print("MODAL_SCOPE_CONTINUATION_EXIT=PASS")


# A palette entry captured in the Window cannot execute after modal authority enters.
background = tui(QT_APP)
open_palette(background, "Activate safely")
opening = invoke_async(QT_APP, "Open modal dialog")
wait_for(
    lambda: inspect(QT_APP, "--dump-scopes"),
    lambda text: any("ModalDialog" in line for line in text.splitlines() if "[ACTIVE]" in line),
    timeout=10,
)
background.send(b"\r")
expect_unavailable_status(background)
assert "Status: activated" not in inspect(QT_APP)
invoke(QT_APP, "Close")
opening.wait(timeout=8)
assert opening.returncode == 0, opening.stderr.read() if opening.stderr else ""
wait_for(
    lambda: inspect(QT_APP, "--dump-scopes"),
    lambda text: any("Window" in line for line in text.splitlines() if "[ACTIVE]" in line),
)
finish(background)
print("MODAL_BACKGROUND_AUTHORITY_REFUSAL=PASS")


# Independent GTK implementation uses the same modal authority rules.
gtk = tui(GTK_APP)
command(gtk, "Open modal dialog", "new-active-modal", "Confirmed", timeout=12)
active, scopes = active_scope(GTK_APP)
assert "ModalDialog" in active, scopes
command(gtk, "Close dialog", "scope-inactive", "Confirmed", timeout=12)
restored_scopes = wait_for(
    lambda: inspect(GTK_APP, "--dump-scopes"),
    lambda text: any("Window" in line for line in text.splitlines() if "[ACTIVE]" in line),
)
active = next(line for line in restored_scopes.splitlines() if "[ACTIVE]" in line)
scopes = restored_scopes
assert "Window" in active and "ModalDialog" not in active, scopes
finish(gtk)
print("CROSS_IMPLEMENTATION_SURFACE_CONTINUATION=PASS")
print("FILE_CHOOSER_SURFACE_LIFECYCLE=NOT_TESTED")
