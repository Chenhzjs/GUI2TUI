#!/usr/bin/env python3
"""Validation-only fixture for the v1.1 native-input authority spike."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk


GLib.set_prgname("gui2tui-native-authority-fixture")


class Fixture(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="org.gui2tui.NativeAuthorityFixture")

    def do_activate(self):
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI Native Authority Fixture")
        window.set_default_size(640, 360)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        content.set_margin_top(24)
        content.set_margin_bottom(24)
        content.set_margin_start(24)
        content.set_margin_end(24)

        entry_label = Gtk.Label(label="Single-line submission target", xalign=0)
        entry = Gtk.Entry()
        entry.set_placeholder_text("Type a value and submit")
        result = Gtk.Label(label="not submitted", xalign=0)
        result.set_selectable(True)

        def submitted(control):
            result.set_text(f"submitted:{control.get_text()}")

        entry.connect("activate", submitted)

        multiline_label = Gtk.Label(label="Multiline raw-Enter target", xalign=0)
        multiline = Gtk.TextView()
        multiline.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        multiline.set_vexpand(True)

        content.append(entry_label)
        content.append(entry)
        content.append(result)
        content.append(multiline_label)
        content.append(multiline)
        window.set_child(content)
        window.present()


Fixture().run()
