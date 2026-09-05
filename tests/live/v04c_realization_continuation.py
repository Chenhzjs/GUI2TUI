#!/usr/bin/env python3
"""Bounded live 0.4C realization checks under X11 and public AT-SPI."""

import json
import os
import pathlib
import re
import subprocess
import time
from collections.abc import Callable
from dataclasses import dataclass

import pexpect
import pyte


INSPECT = os.environ["INSPECT"]
GUI2TUI = os.environ["GUI2TUI"]
QT_APP = "gui2tui-qt-fixture"
GTK_DEMO = "gtk4-demo"
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


def find_action(application: str, label: str) -> str:
    tree = inspect(application, "--verbose")
    for role in ("Button", "CheckBox", "ToggleButton", "MenuItem"):
        if re.search(rf'{role} "{re.escape(label)}"', tree):
            return node_id(tree, role, label)
    raise AssertionError(f"missing action target {label!r}\n{tree}")


def invoke(application: str, label: str, action: str) -> None:
    subprocess.run(
        [INSPECT, "--action-name", find_action(application, label), action],
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


@dataclass(frozen=True)
class TreeRecord:
    locator: str
    role: str
    name: str | None
    parent: str | None


def records(tree: str) -> dict[str, TreeRecord]:
    result: dict[str, TreeRecord] = {}
    stack: dict[int, str] = {}
    pattern = re.compile(
        r'^(?P<prefix>.*?)(?P<role>Application|Window|Dialog|Container|Button|ToggleButton|'
        r'CheckBox|List|ListItem|Tree|TreeItem|Label|Unknown)(?: "(?P<name>[^"]*)")?.* id=(?P<id>[^ ]+)'
    )
    for line in tree.splitlines():
        match = pattern.match(line)
        if not match:
            continue
        depth = len(match.group("prefix")) // 4
        locator = match.group("id")
        parent = stack.get(depth - 1)
        result[locator] = TreeRecord(
            locator=locator,
            role=match.group("role"),
            name=match.group("name"),
            parent=parent,
        )
        stack[depth] = locator
        for stale_depth in [value for value in stack if value > depth]:
            del stack[stale_depth]
    return result


def ancestors(index: dict[str, TreeRecord], locator: str) -> list[TreeRecord]:
    output: list[TreeRecord] = []
    current = index[locator].parent
    while current is not None:
        record = index[current]
        output.append(record)
        current = record.parent
    return output


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


def pump(child: pexpect.spawn, seconds: float = 0.3) -> str:
    screen = pyte.Screen(200, 90)
    stream = pyte.Stream(screen)
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=0.05).decode("utf-8", "replace"))
        except pexpect.TIMEOUT:
            pass
    return "\n".join(screen.display)


def runtime_status(child: pexpect.spawn) -> dict[str, object]:
    child.send(b"\x1b[24~")  # F12: contents-free runtime status.
    frame = pump(child)
    clean = "\n".join(line.strip().strip("│").strip() for line in frame.splitlines())
    raw = clean[clean.index("{") : clean.rindex("}") + 1]
    value = json.loads(raw)
    child.send(b"\x1b")
    pump(child, 0.1)
    return value


def reports() -> list[str]:
    log = RUNTIME_DIR / "gui2tui" / "product.log"
    if not log.exists():
        return []
    return [
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if "semantic transition observation completed" in line
    ]


def open_palette(child: pexpect.spawn, query: str) -> None:
    child.send(b":")
    child.expect(b"Command palette", timeout=6)
    child.send(query.encode())
    # Synchronizes the validation client only. Product success below comes
    # from fresh semantic state or explicit refusal, never from this delay.
    time.sleep(0.2)


def command(child: pexpect.spawn, query: str, condition: str, outcome: str = "Confirmed") -> None:
    before = len(reports())
    open_palette(child, query)
    child.send(b"\r")
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        current = reports()
        if len(current) > before and any(
            f'condition="{condition}"' in line and f"outcome={outcome}" in line
            for line in current[before:]
        ):
            child.expect(b"confirmed" if outcome == "Confirmed" else b"deadline", timeout=5)
            return
        time.sleep(0.05)
    raise AssertionError(f"transition report missing for {query!r}: {reports()}")


def expect_unavailable(child: pexpect.spawn) -> None:
    for word in (b"available", b"current", b"semantic", b"surface"):
        child.expect(word, timeout=8)


def finish(child: pexpect.spawn) -> None:
    child.send(b"q")
    child.expect(pexpect.EOF, timeout=8)


def command_dump(application: str, query: str) -> str:
    return inspect(application, "--dump-commands", "--command-query", query)


# Required GTK sibling realization: state and current scene change, but no
# trigger-to-row ownership is invented from temporal adjacency.
gtk_before = inspect(GTK_DEMO, "--verbose")
constraints = node_id(gtk_before, "Button", "Constraints")
old_simple = node_id(gtk_before, "Button", "Simple Constraints")
before_index = records(gtk_before)
trigger_parent = before_index[constraints].parent
simple_parent = before_index[old_simple].parent
assert trigger_parent and simple_parent and trigger_parent != simple_parent
assert before_index[trigger_parent].parent == before_index[simple_parent].parent
trigger_relations = inspect(GTK_DEMO, "--relations", constraints)
simple_relations = inspect(GTK_DEMO, "--relations", old_simple)
assert "ControllerFor" not in trigger_relations and "ControlledBy" not in simple_relations

gtk_tui = tui(GTK_DEMO, b"GTK Demo")
subprocess.run(
    [INSPECT, "--action-name", constraints, "listitem.collapse"],
    check=True,
    env=os.environ,
    stdout=subprocess.DEVNULL,
)
collapsed = wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: not re.search(r'Button "Constraints".*\bexpanded\b', tree)
    and "Simple Constraints" not in tree,
)
assert "Simple Constraints" not in inspect(GTK_DEMO, "--dump-choices")
assert actions(old_simple).returncode != 0

subprocess.run(
    [INSPECT, "--action-name", constraints, "listitem.expand"],
    check=True,
    env=os.environ,
    stdout=subprocess.DEVNULL,
)
restored = wait_for(
    lambda: inspect(GTK_DEMO, "--verbose"),
    lambda tree: bool(re.search(r'Button "Constraints".*\bexpanded\b', tree))
    and "Simple Constraints" in tree,
)
new_simple = node_id(restored, "Button", "Simple Constraints")
assert new_simple != old_simple
gtk_choices = inspect(GTK_DEMO, "--dump-choices")
assert "Simple Constraints" in gtk_choices
choice_owner = int(re.search(r"Choice owner=(\d+).*name=Some\(\"Demo list\"\)", gtk_choices).group(1))
# GTK exposes the demo list as one semantic Choice. The ordinary scene keeps
# its terminal-native Choice owner rather than duplicating every option as a
# standalone control. Opening that existing overlay proves the user can reach
# the newly realized row from fresh current semantics without adding Tree or
# Selection behavior. Because validation invoked an out-of-product Expand
# action, use the existing explicit full-semantic refresh before continuing;
# no production Expand observer is introduced by this phase.
gtk_tui.send(b"r")
for _ in range(24):
    current = runtime_status(gtk_tui)["focused_runtime"]
    if current == choice_owner:
        break
    gtk_tui.send(b"\t")
    pump(gtk_tui, 0.1)
else:
    raise AssertionError(f"Demo list choice was not reachable; focused_runtime={current}")
gtk_tui.send(b"\r")
assert "Simple Constraints" in pump(gtk_tui, 1.0)
gtk_tui.send(b"\x1b")
finish(gtk_tui)
print("SIBLING_REALIZATION_MANUAL_CONTINUATION=PASS")
print("AMBIGUOUS_REALIZATION_OWNERSHIP_REFUSAL=PASS")


# Qt supplies an independent true-descendant shape with an already-supported
# current CheckBox operation. No production tree or Expand capability is added.
qt_before = inspect(QT_APP, "--verbose")
assert "Realized descendant toggle" not in qt_before
qt_tui = tui(QT_APP, b"Username")
invoke(QT_APP, "Toggle descendant realization", "Press")
realized = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Realized descendant toggle" in tree,
)
old_realized = node_id(realized, "CheckBox", "Realized descendant toggle")
realized_index = records(realized)
assert any(record.name == "Descendant container" for record in ancestors(realized_index, old_realized))
assert "Realized descendant toggle" in command_dump(QT_APP, "Realized descendant toggle")
command(qt_tui, "Realized descendant toggle", "exact-node-state")
checked = inspect(QT_APP, "--verbose")
assert re.search(r'CheckBox "Realized descendant toggle".*\bchecked\b', checked), checked

# Freeze the exact current binding, then remove the node through another exact
# public action. The old entry cannot follow later realization.
open_palette(qt_tui, "Realized descendant toggle")
invoke(QT_APP, "Toggle descendant realization", "Press")
removed = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Realized descendant toggle" not in tree,
)
assert "Status: descendant removed" in removed
assert actions(old_realized).returncode != 0
assert "Realized descendant toggle" not in command_dump(QT_APP, "Realized descendant toggle")

invoke(QT_APP, "Toggle descendant realization", "Press")
rerealized = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Realized descendant toggle" in tree,
)
new_realized = node_id(rerealized, "CheckBox", "Realized descendant toggle")
assert new_realized != old_realized
assert not re.search(r'CheckBox "Realized descendant toggle".*\bchecked\b', rerealized)
# The replacement exists when the frozen L1 entry is submitted. Exact current
# authority must refuse rather than transfer the entry to L2.
qt_tui.send(b"\r")
expect_unavailable(qt_tui)
assert not re.search(
    r'CheckBox "Realized descendant toggle".*\bchecked\b',
    inspect(QT_APP, "--verbose"),
)
command(qt_tui, "Realized descendant toggle", "exact-node-state")
assert re.search(
    r'CheckBox "Realized descendant toggle".*\bchecked\b',
    inspect(QT_APP, "--verbose"),
)
print("STRUCTURAL_DESCENDANT_CONTINUATION=PASS")
print("COLLAPSED_NODE_STALE_BINDING_REFUSAL=PASS")
print("REREALIZED_LOCATOR_AUTHORITY_SEPARATION=PASS")
print("FRESH_REALIZED_BINDING_CURRENT_AUTHORITY=PASS")
print("FOCUS_AFTER_REALIZATION_CHANGE=PASS")


# A separately triggered subtree realizes concurrently in the same current
# Window. Fresh public ancestry places it under its own container; neither
# trigger timing nor names create an association with the first realization.
invoke(QT_APP, "Toggle unrelated realization", "Press")
unrelated = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Unrelated realized action" in tree,
)
unrelated_id = node_id(unrelated, "Button", "Unrelated realized action")
unrelated_index = records(unrelated)
unrelated_ancestors = ancestors(unrelated_index, unrelated_id)
assert any(record.name == "Unrelated container" for record in unrelated_ancestors)
assert all(record.name != "Descendant container" for record in unrelated_ancestors)
assert "Unrelated realized action" in command_dump(QT_APP, "Unrelated realized action")
unrelated_relations = inspect(QT_APP, "--relations", unrelated_id)
assert "ControllerFor" not in unrelated_relations and "ControlledBy" not in unrelated_relations
print("UNRELATED_REALIZATION_NO_ATTRIBUTION=PASS")
print("HIERARCHY_USES_PUBLIC_STRUCTURE_ONLY=PASS")


# Restore both controlled fixtures to their initial state authoritatively.
invoke(QT_APP, "Toggle unrelated realization", "Press")
wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Unrelated realized action" not in tree,
)
invoke(QT_APP, "Toggle descendant realization", "Press")
restored_qt = wait_for(
    lambda: inspect(QT_APP, "--verbose"),
    lambda tree: "Realized descendant toggle" not in tree,
)
assert "Status: descendant removed" in restored_qt
finish(qt_tui)
