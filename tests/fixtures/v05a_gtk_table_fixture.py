#!/usr/bin/env python3
"""Bounded GTK3 Table row-selection fixture for v0.5A live qualification."""

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-v05a-gtk-table")
GLib.set_application_name("GUI2TUI v0.5A GTK Table")


window = Gtk.Window(title="GUI2TUI v0.5A GTK Table")
window.set_default_size(520, 300)
window.connect("destroy", Gtk.main_quit)
content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
store = Gtk.ListStore(str, str)
for row in [("Alpha", "10"), ("Beta", "20"), ("Gamma", "30")]:
    store.append(row)
table = Gtk.TreeView(model=store)
table.get_accessible().set_name("Current semantic rows")
table.get_selection().set_mode(Gtk.SelectionMode.SINGLE)
table.get_selection().select_path(Gtk.TreePath.new_from_indices([0]))
for column, title in enumerate(["Item", "Value"]):
    renderer = Gtk.CellRendererText()
    table.append_column(Gtk.TreeViewColumn(title, renderer, text=column))
content.pack_start(table, True, True, 0)
status = Gtk.Label(label="Selected row: Alpha")
content.pack_start(status, False, False, 0)


def selection_changed(selection) -> None:
    model, iterator = selection.get_selected()
    label = model[iterator][0] if iterator is not None else "none"
    status.set_text(f"Selected row: {label}")


table.get_selection().connect("changed", selection_changed)


def reorder_rows(_button) -> None:
    store.clear()
    for row in [("Beta", "20"), ("Alpha", "10"), ("Gamma", "30")]:
        store.append(row)
    table.get_selection().select_path(Gtk.TreePath.new_from_indices([2]))
    status.set_text("Structure: rows reordered; selected Gamma")


reorder = Gtk.Button(label="Reorder table rows")
reorder.connect("clicked", reorder_rows)
content.pack_start(reorder, False, False, 0)
window.add(content)
window.show_all()
Gtk.main()
