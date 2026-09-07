#!/usr/bin/env python3
"""Integrated user-facing v0.4 continuation checks under X11/AT-SPI."""

import os
import pathlib
import re
import subprocess
import time
from collections.abc import Callable
from dataclasses import dataclass, field

import pexpect
import pyte


INSPECT = os.environ["INSPECT"]
GUI2TUI = os.environ["GUI2TUI"]
QT_APP = "gui2tui-qt-fixture"
GTK_APP = "gui2tui-live-fixture"
RUNTIME_DIR = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
FORBIDDEN_NORMAL_UI = (
    "BackendLocator",
    "RuntimeNodeId",
    "InteractionScope",
    "observation deadline",
    "authoritative transition",
    "semantic runtime",
    "current binding",
)
EVIDENCE: dict[str, str] = {}


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


def reports() -> list[str]:
    log = RUNTIME_DIR / "gui2tui" / "product.log"
    if not log.exists():
        return []
    return [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "semantic transition observation completed" in line
    ]


def command_dump(application: str, query: str) -> str:
    return inspect(application, "--dump-commands", "--command-query", query)


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
    for role in ("Button", "CheckBox", "ToggleButton", "MenuItem"):
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


def assert_task_language(frame: str) -> None:
    for internal in FORBIDDEN_NORMAL_UI:
        assert internal not in frame, f"normal UI leaked {internal!r}\n{frame}"


@dataclass
class Tui:
    application: str
    child: pexpect.spawn = field(init=False)
    screen: pyte.Screen = field(init=False)
    stream: pyte.Stream = field(init=False)

    def __post_init__(self) -> None:
        self.child = pexpect.spawn(
            GUI2TUI,
            [
                "--app",
                self.application,
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
            dimensions=(44, 200),
        )
        self.screen = pyte.Screen(200, 44)
        self.stream = pyte.Stream(self.screen)
        self.wait_screen(lambda frame: "Username" in frame, timeout=15)

    def pump(self, seconds: float = 0.15) -> str:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            try:
                chunk = self.child.read_nonblocking(size=65536, timeout=0.03)
                self.stream.feed(chunk.decode("utf-8", errors="replace"))
            except pexpect.TIMEOUT:
                pass
        return "\n".join(self.screen.display)

    def wait_screen(self, predicate: Callable[[str], bool], timeout: float = 8) -> str:
        deadline = time.monotonic() + timeout
        frame = self.pump(0.05)
        while time.monotonic() < deadline:
            if predicate(frame):
                return frame
            frame = self.pump(0.08)
        raise AssertionError(f"terminal condition was not reached\n{frame}")

    def open_palette(self, query: str) -> None:
        self.child.send(b":")
        self.wait_screen(lambda frame: "[search: current interaction scope; F2 toggle]" in frame)
        self.child.send(query.encode())
        self.wait_screen(
            lambda frame: f"> {query}  [search: current interaction scope; F2 toggle]" in frame
        )

    def command(
        self,
        query: str,
        condition: str,
        outcome: str,
        user_status: str | None = None,
        timeout: float = 12,
    ) -> str:
        before = len(reports())
        self.open_palette(query)
        self.child.send(b"\r")
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            frame = self.pump(0.08)
            current = reports()
            if len(current) > before and any(
                f'condition="{condition}"' in line and f"outcome={outcome}" in line
                for line in current[before:]
            ):
                if user_status is not None:
                    frame = self.wait_screen(lambda value: user_status in value)
                else:
                    frame = self.pump(0.2)
                assert_task_language(frame)
                return frame
        raise AssertionError(
            f"transition report missing: {query=} {condition=} {outcome=}\n"
            f"{reports()}\nTERMINAL:\n{frame}"
        )

    def finish(self) -> None:
        self.child.send(b"q")
        self.child.expect(pexpect.EOF, timeout=8)


# The same-scope menu uses the normal palette twice. Its changed scene, not a
# workflow screen, tells the user what is newly available.
assert "Activate Demo" not in command_dump(QT_APP, "Activate Demo")
qt = Tui(QT_APP)
menu_frame = qt.command("Tools", "exact-node-state", "Confirmed")
opened = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: bool(
        re.search(r"\bMenu \[.*\bshowing\b", tree)
        and re.search(r'MenuItem "Activate Demo".*\bshowing\b', tree)
    ),
)
menu_id = unnamed_node_id(opened, "Menu")
relations = inspect(QT_APP, "--relations", menu_id)
assert "PopupFor" not in relations, relations
scopes = inspect(QT_APP, "--dump-scopes")
assert "MenuPopup" not in scopes
assert any("Window" in line for line in scopes.splitlines() if "[ACTIVE]" in line)
assert "Activate Demo" in command_dump(QT_APP, "Activate Demo")
qt.open_palette("Activate Demo")
menu_frame = qt.wait_screen(lambda frame: "Activate Demo" in frame)
qt.child.send(b"\x1b")
qt.wait_screen(lambda frame: "[search: current interaction scope; F2 toggle]" not in frame)
closed_frame = qt.command(
    "Activate Demo",
    "exact-surface-unavailable",
    "Confirmed",
)
assert "Activate Demo" not in command_dump(QT_APP, "Activate Demo")
assert not re.search(r"\bMenu \[.*\bshowing\b", inspect(QT_APP, "--verbose"))
menu_count = int(
    re.search(r"Status: menu activated (\d+)", inspect(QT_APP)).group(1)
)
EVIDENCE["menu-revealed"] = menu_frame
EVIDENCE["menu-return"] = closed_frame
print("CONTINUATION_UX_MENU=PASS")


# Modal entry/exit uses the same normal scene. Dialog title/content and absent
# background commands communicate current authority without scope terminology.
modal_frame = qt.command(
    "Open modal dialog",
    "new-active-modal",
    "Confirmed",
)
modal_frame = qt.wait_screen(
    lambda frame: "Qt Fixture Dialog" in frame and "Close" in frame
)
assert "Qt Fixture Dialog" in modal_frame and "Close" in modal_frame
assert "Activate safely" not in command_dump(QT_APP, "Activate safely")
active = next(
    line for line in inspect(QT_APP, "--dump-scopes").splitlines() if "[ACTIVE]" in line
)
assert "ModalDialog" in active
return_frame = qt.command("Close", "scope-inactive", "Confirmed")
return_frame = qt.wait_screen(
    lambda frame: "Open modal dialog" in frame and "Activate safely" in frame
)
assert "Open modal dialog" in return_frame and "Activate safely" in return_frame
active = next(
    line for line in inspect(QT_APP, "--dump-scopes").splitlines() if "[ACTIVE]" in line
)
assert "Window" in active
EVIDENCE["qt-modal"] = modal_frame
EVIDENCE["qt-return"] = return_frame
print("CONTINUATION_UX_MODAL=PASS")


# A real unconfirmed transition reports only what is known, while the current
# authoritative application state and subsequent command remain usable.
timeout_frame = qt.command(
    "Activate safely",
    "new-active-modal",
    "Timeout",
    'Action for "Activate safely" was not confirmed; current interface is shown',
)
assert "Status: activated" in inspect(QT_APP)
qt.command("Enable feature", "exact-node-state", "Confirmed")
EVIDENCE["unconfirmed"] = timeout_frame
print("CONTINUATION_UX_UNCONFIRMED=PASS")


# A frozen menu entry is rejected after the menu closes independently. The
# wording is task-oriented, the second activation never occurs, and a fresh
# current command remains usable.
qt.command("Tools", "exact-node-state", "Confirmed")
qt.open_palette("Activate Demo")
invoke(QT_APP, "Activate Demo")
wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: f"Status: menu activated {menu_count + 1}" in tree
    and not re.search(r"\bMenu \[.*\bshowing\b", tree),
)
qt.child.send(b"\r")
stale_frame = qt.wait_screen(
    lambda frame: "Command is no longer available; choose from the current interface" in frame
)
assert_task_language(stale_frame)
assert f"Status: menu activated {menu_count + 1}" in inspect(QT_APP)
qt.command("Tools", "exact-node-state", "Confirmed")
qt.command(
    "Activate Demo",
    "exact-surface-unavailable",
    "Confirmed",
)
assert f"Status: menu activated {menu_count + 2}" in inspect(QT_APP)
EVIDENCE["stale-refusal"] = stale_frame
qt.finish()
print("CONTINUATION_UX_STALE_REFUSAL=PASS")


# GTK creates/destroys its dialog while Qt's lifecycle differs below the UI;
# the normal user model remains title/content/current action/return. The full
# runner already exercises that user path in the 0.4B probe before other apps
# can change X11 activation; the standalone UX pass additionally captures the
# rendered GTK frame.
if os.environ.get("V04D_SKIP_GTK_UX") != "1":
    gtk = Tui(GTK_APP)
    gtk_modal = gtk.command(
        "Open modal dialog",
        "new-active-modal",
        "Confirmed",
    )
    gtk_modal = gtk.wait_screen(
        lambda frame: "Dialog content" in frame and "Close dialog" in frame
    )
    assert "Dialog content" in gtk_modal and "Close dialog" in gtk_modal
    assert "Activate safely" not in command_dump(GTK_APP, "Activate safely")
    gtk_return = gtk.command("Close dialog", "scope-inactive", "Confirmed")
    gtk_return = gtk.wait_screen(
        lambda frame: "Open modal dialog" in frame and "Activate safely" in frame
    )
    assert "Open modal dialog" in gtk_return and "Activate safely" in gtk_return
    gtk.finish()
    EVIDENCE["gtk-modal"] = gtk_modal
print("CONTINUATION_UX_MODAL_CROSS_IMPLEMENTATION=PASS")


# Keep a small privacy-safe text-frame artifact for manual review. This is not
# a golden test and contains only controlled fixture content.
artifact = pathlib.Path("/tmp/gui2tui-v04d-frames.txt")
artifact.write_text(
    "\n".join(f"===== {name} =====\n{frame.rstrip()}" for name, frame in EVIDENCE.items()),
    encoding="utf-8",
)
print("NO_WORKFLOW_UI=PASS")
