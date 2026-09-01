#!/usr/bin/env python3
"""Small, harmless GTK4 application used only for public GUI2TUI recordings."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-demo-fixture")
GLib.set_application_name("GUI2TUI Demo Fixture")


class DemoFixture(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="org.gui2tui.DemoFixture")

    def do_activate(self) -> None:
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI Demo Fixture")
        window.set_default_size(500, 720)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        content.set_margin_top(24)
        content.set_margin_bottom(24)
        content.set_margin_start(24)
        content.set_margin_end(24)

        title = Gtk.Label()
        title.set_markup("<span size='x-large' weight='bold'>GUI2TUI semantic demo</span>")
        content.append(title)

        intro = Gtk.Label(
            label="This is the original GTK application.\n"
            "The terminal reads and operates it through AT-SPI."
        )
        intro.set_wrap(True)
        content.append(intro)

        article = Gtk.TextView()
        article.set_editable(False)
        article.set_cursor_visible(False)
        article.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        article.set_size_request(-1, 250)
        article.get_buffer().set_text(
            "Semantic Reader\n\n"
            "GUI2TUI recompiles accessibility semantics into terminal-native tasks.\n\n"
            "Search works on exposed semantic content, without copying GUI pixels.\n\n"
            "Safe actions are sent back to the original application."
        )
        article.update_property(
            [Gtk.AccessibleProperty.LABEL], ["GUI2TUI semantic article"]
        )
        content.append(article)

        feature = Gtk.CheckButton(label="Enable semantic feature")
        content.append(feature)

        status = Gtk.Label(label="Status: idle")
        content.append(status)

        activate = Gtk.Button(label="Activate safely")

        def activate_safely(_button: Gtk.Button) -> None:
            feature.set_active(True)
            status.set_label("Status: activated by GUI2TUI")

        activate.connect("clicked", activate_safely)
        content.append(activate)

        safety = Gtk.Label(
            label="Synthetic data only. No files, network, credentials, or system settings."
        )
        safety.set_wrap(True)
        content.append(safety)

        window.set_child(content)
        window.present()


if __name__ == "__main__":
    raise SystemExit(DemoFixture().run(None))
