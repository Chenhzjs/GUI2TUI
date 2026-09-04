"""Packaged GTK3 complete-text fixture with an inert synthetic backing file."""

import pathlib
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-release-text-demo")
GLib.set_application_name("GUI2TUI Release Text Demo")


backing = pathlib.Path(sys.argv[1])
window = Gtk.Window(title="GUI2TUI Release Text Demo")
window.set_default_size(420, 260)
window.connect("destroy", Gtk.main_quit)
text = Gtk.TextView()
text.set_editable(True)
text.get_buffer().set_text(backing.read_text(encoding="utf-8"))
text.get_accessible().set_name("Release external text")
window.add(text)
window.show_all()
Gtk.main()
