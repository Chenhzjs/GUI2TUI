#!/usr/bin/env python3
"""Safe GTK4 fixture for PreserveModality transcompiler validation."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402


class OpaqueFixture(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="org.gui2tui.OpaqueFixture")

    def do_activate(self):
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI Opaque Fixture")
        window.set_default_size(640, 480)

        layout = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        layout.set_margin_top(12)
        layout.set_margin_bottom(12)
        layout.set_margin_start(12)
        layout.set_margin_end(12)
        layout.append(Gtk.Label(label="Semantic controls around graphical content"))

        drawing = Gtk.DrawingArea()
        drawing.set_accessible_role(Gtk.AccessibleRole.IMG)
        drawing.update_property(
            [Gtk.AccessibleProperty.LABEL],
            ["Preview canvas"],
        )
        drawing.set_content_width(480)
        drawing.set_content_height(280)
        drawing.set_draw_func(
            lambda _area, context, width, height: (
                context.set_source_rgb(0.1, 0.25, 0.45),
                context.rectangle(0, 0, width, height),
                context.fill(),
            )
        )
        layout.append(drawing)

        status = Gtk.Label(label="Status: idle")
        layout.append(status)
        button = Gtk.Button(label="Activate safely")
        button.connect("clicked", lambda _button: status.set_label("Status: activated"))
        layout.append(button)
        window.set_child(layout)
        window.present()


OpaqueFixture().run(None)
