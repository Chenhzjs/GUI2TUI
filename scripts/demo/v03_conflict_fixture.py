#!/usr/bin/env python3
"""Controlled complete-text conflict fixture used only by the v0.3 demo."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-v03-conflict-demo")
GLib.set_application_name("GUI2TUI v0.3 Conflict Demo")


TEXT_A = "Authoritative GUI text A\n\nThe external candidate starts from A.\n"
TEXT_B = "Authoritative GUI text B\n\nThe GUI changed while editing.\n"


class ConflictDemo(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="org.gui2tui.V03ConflictDemo")

    def do_activate(self) -> None:
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI v0.3 — Conflict Refusal")
        window.set_default_size(540, 520)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        content.set_margin_top(24)
        content.set_margin_bottom(24)
        content.set_margin_start(24)
        content.set_margin_end(24)

        heading = Gtk.Label(label="Conflict refusal")
        heading.add_css_class("title-1")
        content.append(heading)

        explanation = Gtk.Label(
            label="The GUI changes from A to B while a private candidate C is edited."
        )
        explanation.set_wrap(True)
        content.append(explanation)

        text = Gtk.TextView()
        text.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        text.set_size_request(-1, 240)
        text.get_buffer().set_text(TEXT_A)
        text.update_property(
            [Gtk.AccessibleProperty.LABEL], ["Conflict demonstration text"]
        )
        content.append(text)

        change = Gtk.Button(label="Change authoritative text to B")
        change.connect("clicked", lambda _button: text.get_buffer().set_text(TEXT_B))
        content.append(change)

        note = Gtk.Label(
            label="GUI2TUI must refuse C and preserve it privately instead of overwriting B."
        )
        note.set_wrap(True)
        content.append(note)

        window.set_child(content)
        window.present()


raise SystemExit(ConflictDemo().run(None))
