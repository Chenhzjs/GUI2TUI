#!/usr/bin/env python3
"""Small GTK3 surface for external and terminal lifecycle evidence."""

import pathlib

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-v06d-lifecycle")
GLib.set_application_name("GUI2TUI v0.6D Lifecycle")

window = Gtk.Window(title="GUI2TUI v0.6D Lifecycle")
window.set_default_size(480, 420)
window.connect("destroy", Gtk.main_quit)

layout = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
window.add(layout)

external_text = Gtk.TextView()
external_text.set_editable(True)
external_text.get_buffer().set_text("lifecycle alpha\nlifecycle beta\n")
external_text.get_accessible().set_name("Lifecycle external text")
layout.pack_start(external_text, True, True, 0)

change_external_text = Gtk.Button(label="Change lifecycle text independently")
change_external_text.connect(
    "clicked",
    lambda _button: external_text.get_buffer().set_text("lifecycle authoritative B\n"),
)
layout.pack_start(change_external_text, False, False, 0)

status = Gtk.Label(label="Lifecycle status: idle")
layout.pack_start(status, False, False, 0)
activate = Gtk.Button(label="Lifecycle activate")
activate.connect("clicked", lambda _button: status.set_text("Lifecycle status: activated"))
layout.pack_start(activate, False, False, 0)

password = Gtk.Entry()
password.set_text("v06d-private-secret")
password.set_visibility(False)
password.get_accessible().set_name("Lifecycle password")
layout.pack_start(password, False, False, 0)

picture = Gtk.Image.new_from_file(
    str(pathlib.Path("tests/fixtures/modality/architecture.svg"))
)
picture.get_accessible().set_name("Lifecycle diagram")
layout.pack_start(picture, True, True, 0)

window.show_all()
Gtk.main()
