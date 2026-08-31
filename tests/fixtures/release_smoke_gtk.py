"""Packaged, harmless GTK4 demo; no repository-relative assets."""
import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
GLib.set_prgname("gui2tui-release-demo")

class Demo(Gtk.Application):
    def do_activate(self):
        window = Gtk.ApplicationWindow(application=self, title="GUI2TUI Release Demo")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        entry = Gtk.Entry(text="alice")
        entry.update_property([Gtk.AccessibleProperty.LABEL], ["Username"])
        password = Gtk.PasswordEntry()
        password.set_text("release-password-sentinel")
        password.update_property([Gtk.AccessibleProperty.LABEL], ["Password"])
        status = Gtk.Label(label="Status: idle")
        check = Gtk.CheckButton(label="Enable feature")
        button = Gtk.Button(label="Activate safely")
        def activate(_):
            check.set_active(True)
            status.set_text("Status: activated")
        button.connect("clicked", activate)
        for child in [Gtk.Label(label="Release demo"), entry, password, check, status, button]:
            box.append(child)
        window.set_child(box)
        window.present()
Demo(application_id="org.gui2tui.ReleaseDemo").run(None)
