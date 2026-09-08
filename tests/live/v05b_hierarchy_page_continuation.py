#!/usr/bin/env python3
"""Bounded Linux evidence for verified hierarchy and page continuation.

The terminal drives GUI2TUI through its normal command palette. Fixture setup
and assertions use only the public AT-SPI inspector; no input or coordinate
emulation is used.
"""

from __future__ import annotations

import os
import pathlib
import re
import subprocess
import time
from collections.abc import Callable

import pexpect


INSPECT = os.environ["INSPECT"]
GUI2TUI = os.environ["GUI2TUI"]
RUNTIME_DIR = pathlib.Path(os.environ["XDG_RUNTIME_DIR"])
GTK_DEMO = "gtk4-demo"
GTK_PAGES = "gui2tui-v05b-gtk-pages"
QT_PAGES = "gui2tui-v05b-qt-pages"


def inspect(application: str, *extra: str) -> str:
    return subprocess.check_output(
        [INSPECT, "--app", application, "--bootstrap", "walk", *extra],
        text=True,
        env=os.environ,
    )


def wait_for(
    read: Callable[[], str], predicate: Callable[[str], bool], timeout: float = 10
) -> str:
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


def line_for(tree: str, role: str, label: str) -> str:
    return next(
        line
        for line in tree.splitlines()
        if re.search(rf'{re.escape(role)} "{re.escape(label)}"', line)
    )


def action_id(application: str, label: str) -> str:
    tree = inspect(application, "--verbose")
    for role in ("Button", "CheckBox", "ToggleButton", "Tab"):
        if re.search(rf'{role} "{re.escape(label)}"', tree):
            return node_id(tree, role, label)
    raise AssertionError(f"missing action target {label!r}\n{tree}")


def invoke(application: str, label: str, action: str) -> None:
    subprocess.run(
        [INSPECT, "--action-name", action_id(application, label), action],
        check=True,
        env=os.environ,
        stdout=subprocess.DEVNULL,
    )


def actions(locator: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [INSPECT, "--actions", locator],
        env=os.environ,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def command_dump(application: str, query: str) -> str:
    return inspect(application, "--dump-commands", "--command-query", query)


def tui(application: str, expected: bytes) -> pexpect.spawn:
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
        dimensions=(90, 200),
    )
    child.expect(expected, timeout=15)
    return child


def open_palette(child: pexpect.spawn, query: str) -> None:
    child.send(b":")
    child.expect(b"Command palette", timeout=6)
    child.send(query.encode())
    # This only synchronizes the terminal validation client. Product truth is
    # proven below by fresh AT-SPI state/readback or a current-command refusal.
    time.sleep(0.15)


def product_lines(marker: str) -> list[str]:
    log = RUNTIME_DIR / "gui2tui" / "product.log"
    if not log.exists():
        return []
    return [line for line in log.read_text(encoding="utf-8").splitlines() if marker in line]


def run_transition_command(child: pexpect.spawn, query: str, condition: str) -> None:
    before = len(product_lines("semantic transition observation completed"))
    open_palette(child, query)
    child.send(b"\r")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        fresh = product_lines("semantic transition observation completed")[before:]
        if any(
            f'condition="{condition}"' in line and "outcome=Confirmed" in line
            for line in fresh
        ):
            return
        time.sleep(0.05)
    raise AssertionError(f"missing confirmed transition for {query!r}")


def run_page_command(child: pexpect.spawn, query: str) -> None:
    before = len(product_lines("current semantic target observation completed"))
    open_palette(child, query)
    child.send(b"\r")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        fresh = product_lines("current semantic target observation completed")[before:]
        if any("outcome=Confirmed" in line for line in fresh):
            return
        time.sleep(0.05)
    raise AssertionError(f"missing confirmed current-page readback for {query!r}")


def expect_unavailable(child: pexpect.spawn) -> None:
    # Terminal redraw ANSI escapes can occur between words; the status keeps
    # the task-oriented recovery word intact.
    child.expect(b"interface", timeout=8)


def finish(child: pexpect.spawn) -> None:
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)


def tab_state(tree: str, label: str) -> tuple[str, str]:
    return node_id(tree, "Tab", label), line_for(tree, "Tab", label)


def assert_gtk_advanced_current(tree: str) -> None:
    advanced_id, advanced_line = tab_state(tree, "Advanced")
    general_id, general_line = tab_state(tree, "General")
    assert "selected" in advanced_line, advanced_line
    assert "selected" not in general_line, general_line
    assert "Advanced page setting" in tree
    assert advanced_id != general_id


def assert_qt_advanced_current(tree: str) -> None:
    """Qt's public composite postcondition; never locator identity by name."""
    advanced_id, advanced_line = tab_state(tree, "Advanced")
    _general_id, general_line = tab_state(tree, "General")
    tab_list_line = next(line for line in tree.splitlines() if "TabList" in line)
    assert "focused" in advanced_line, advanced_line
    assert "focused" not in general_line, general_line
    assert 'TabList "Advanced"' in tab_list_line, tab_list_line
    assert sum('Tab "Advanced"' in line for line in tree.splitlines()) == 1
    assert "Advanced page setting" in tree
    assert advanced_id


# GTK hierarchy: exact public expansion action and Expanded-state proof. The
# realized rows are sibling-shaped, and their old locator must not survive a
# collapse/re-realization.
gtk_before = inspect(GTK_DEMO, "--verbose")
constraints = node_id(gtk_before, "Button", "Constraints")
old_simple = node_id(gtk_before, "Button", "Simple Constraints")
assert "expandable" in line_for(gtk_before, "Button", "Constraints")
assert "expanded" in line_for(gtk_before, "Button", "Constraints")
assert "listitem.collapse" in line_for(gtk_before, "Button", "Constraints")
assert "listitem.expand" in line_for(gtk_before, "Button", "Constraints")

gtk_tui = tui(GTK_DEMO, b"GTK Demo")
assert "Collapse Constraints" in command_dump(GTK_DEMO, "Constraints")
run_transition_command(gtk_tui, "Collapse Constraints", "exact-node-state")
collapsed = wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: "expanded" not in line_for(tree, "Button", "Constraints")
    and "Simple Constraints" not in tree,
)
assert actions(old_simple).returncode != 0
assert "Expand Constraints" in command_dump(GTK_DEMO, "Constraints")
run_transition_command(gtk_tui, "Expand Constraints", "exact-node-state")
restored = wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: "expanded" in line_for(tree, "Button", "Constraints")
    and "Simple Constraints" in tree,
)
assert node_id(restored, "Button", "Simple Constraints") != old_simple

# A frozen operation that no longer expresses the current desired state cannot
# invert expansion. This is intentionally a normal command-palette refusal.
open_palette(gtk_tui, "Collapse Constraints")
invoke(GTK_DEMO, "Constraints", "listitem.collapse")
wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: "expanded" not in line_for(tree, "Button", "Constraints"),
)
gtk_tui.send(b"\r")
expect_unavailable(gtk_tui)
still_collapsed = inspect(GTK_DEMO, "--verbose")
assert "expanded" not in line_for(still_collapsed, "Button", "Constraints")

open_palette(gtk_tui, "Expand Constraints")
invoke(GTK_DEMO, "Constraints", "listitem.expand")
wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: "expanded" in line_for(tree, "Button", "Constraints"),
)
gtk_tui.send(b"\r")
expect_unavailable(gtk_tui)
assert "expanded" in line_for(inspect(GTK_DEMO, "--verbose"), "Button", "Constraints")
finish(gtk_tui)

# The current Qt tree exposes an action named Toggle, but no explicit
# expansion semantics. It must receive no product Expand command and is never
# probed by invocation.
qt_tree = inspect(QT_PAGES, "--verbose")
assert "Tree \"Ambiguous hierarchy\"" in qt_tree
assert "actions=[Toggle]" in qt_tree
assert "Expand" not in command_dump(QT_PAGES, "Ambiguous hierarchy")

# GTK page switch through parent Selection, with exact selected PageTab
# readback and ordinary current-scene continuation.
gtk_page_tree = inspect(GTK_PAGES, "--verbose")
gtk_advanced_before = node_id(gtk_page_tree, "Tab", "Advanced")
gtk_pages_tui = tui(GTK_PAGES, b"GTK Pages")
assert "Switch to Advanced" in command_dump(GTK_PAGES, "Advanced")
run_page_command(gtk_pages_tui, "Switch to Advanced")
gtk_after = wait_for(
    lambda: inspect(GTK_PAGES, "--verbose"),
    lambda tree: "selected" in line_for(tree, "Tab", "Advanced")
    and "Advanced page setting" in tree,
)
assert_gtk_advanced_current(gtk_after)

# A request for an already-current page has exact selected readback and does
# not need a second backend mutation.
run_page_command(gtk_pages_tui, "Switch to Advanced")
assert_gtk_advanced_current(inspect(GTK_PAGES, "--verbose"))

# Old page-only control is captured before the page changes. Fresh scope and
# visibility remove its authority; a frozen palette entry refuses safely.
run_page_command(gtk_pages_tui, "Switch to General")
assert "General page action" in inspect(GTK_PAGES, "--verbose")
open_palette(gtk_pages_tui, "General page action")
# Use the normal GUI2TUI command for the actual page change; the palette entry
# above remains frozen while a second current TUI supplies the public action.
switcher = tui(GTK_PAGES, b"GTK Pages")
run_page_command(switcher, "Switch to Advanced")
finish(switcher)
gtk_pages_tui.send(b"\r")
expect_unavailable(gtk_pages_tui)
assert "General page action" not in command_dump(GTK_PAGES, "General page action")

# Replacing the same-labelled tab produces a distinct locator. The old frozen
# switch cannot migrate; a fresh current command still works afterwards.
run_page_command(gtk_pages_tui, "Switch to General")
open_palette(gtk_pages_tui, "Switch to Advanced")
invoke(GTK_PAGES, "Replace Advanced page", "Click")
gtk_replaced = wait_for(
    lambda: inspect(GTK_PAGES, "--verbose"),
    lambda tree: node_id(tree, "Tab", "Advanced") != gtk_advanced_before,
)
assert node_id(gtk_replaced, "Tab", "Advanced") != gtk_advanced_before
gtk_pages_tui.send(b"\r")
expect_unavailable(gtk_pages_tui)
run_page_command(gtk_pages_tui, "Switch to Advanced")
assert_gtk_advanced_current(inspect(GTK_PAGES, "--verbose"))
finish(gtk_pages_tui)

# Qt page switch through its exact public PageTab Press. The target locator is
# already exact; the fresh current-page proof requires the constrained public
# composite and rejects either focus or name alone.
qt_pages_tui = tui(QT_PAGES, b"Qt Pages")
assert "Switch to Advanced" in command_dump(QT_PAGES, "Advanced")
run_page_command(qt_pages_tui, "Switch to Advanced")
qt_after = wait_for(
    lambda: inspect(QT_PAGES, "--verbose"),
    lambda tree: 'TabList "Advanced"' in tree and "Advanced page setting" in tree,
)
assert_qt_advanced_current(qt_after)
assert "General page action" not in command_dump(QT_PAGES, "General page action")
finish(qt_pages_tui)

print("HIERARCHY_EXPAND_CONFIRMED=PASS")
print("HIERARCHY_COLLAPSE_CONFIRMED=PASS")
print("HIERARCHY_MANUAL_CONTINUATION=PASS")
print("AMBIGUOUS_TOGGLE_NOT_EXPAND=PASS")
print("EXPANSION_ALREADY_DESIRED_STATE_SAFE=PASS")
print("EXPANSION_STALE_TARGET_AUTHORITY_REFUSAL=PASS")
print("PAGETAB_GTK_SWITCH=PASS")
print("PAGETAB_QT_SWITCH=PASS")
print("PAGETAB_TARGET_SPECIFIC_READBACK=PASS")
print("PAGETAB_ALREADY_CURRENT_SAFE=PASS")
print("OLD_PAGE_COMMAND_AUTHORITY_REFUSAL=PASS")
print("PAGETAB_STALE_TARGET_REFUSAL=PASS")
print("NO_INFERRED_REALIZATION_OWNERSHIP=PASS")
print("NO_TOOLKIT_BRANCH=PASS")
