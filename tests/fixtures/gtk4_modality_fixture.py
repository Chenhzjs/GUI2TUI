#!/usr/bin/env python3
"""GTK4 image fixture; production discovery may use AT-SPI evidence only."""
import pathlib
import os
import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402

RESOURCE = pathlib.Path(__file__).parent / "modality" / "architecture.svg"

class Fixture(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="org.gui2tui.GtkModalityFixture")

    def do_activate(self):
        window = Gtk.ApplicationWindow(application=self, title="GUI2TUI GTK Modality Fixture")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.append(Gtk.Label(label="Local architecture image"))
        image = Gtk.Picture.new_for_filename(str(RESOURCE))
        image.update_property([Gtk.AccessibleProperty.LABEL], ["Architecture diagram"])
        box.append(image)
        # Dedicated visual-only client is also a real Image. This variant makes
        # its bounds independently checkable against the native X11 client.
        # The original mixed layout remains a negative coordinate-safety probe.
        if os.environ.get("VISUAL_ONLY") == "1":
            box.remove(image)
            window.set_child(image)
        else:
            window.set_child(box)
        window.present()

Fixture().run(None)
