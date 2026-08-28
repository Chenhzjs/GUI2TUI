#!/usr/bin/env python3
"""Small, non-destructive GTK4 application for manual AT-SPI validation."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-live-fixture")
GLib.set_application_name("GUI2TUI Live Fixture")


class LiveFixture(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="org.gui2tui.LiveFixture")

    def do_activate(self) -> None:
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI Live Fixture")
        window.set_default_size(420, 260)

        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        content.set_margin_top(16)
        content.set_margin_bottom(16)
        content.set_margin_start(16)
        content.set_margin_end(16)

        heading = Gtk.Label(label="Phase 0 live validation")
        content.append(heading)

        username = Gtk.Entry()
        username.set_placeholder_text("Username")
        username.set_text("alice")
        username.update_property([Gtk.AccessibleProperty.LABEL], ["Username"])
        content.append(username)
        username_ref = {"widget": username}

        long_input = Gtk.Entry()
        long_input.set_text("L" * 300 + "-gtk-tail")
        long_input.update_property([Gtk.AccessibleProperty.LABEL], ["Long input"])
        content.append(long_input)

        content.append(Gtk.Label(label="Password"))
        password = Gtk.PasswordEntry()
        password.set_text("phase-zero-secret")
        password.update_property([Gtk.AccessibleProperty.LABEL], ["Password"])
        content.append(password)

        checkbox = Gtk.CheckButton(label="Enable feature")
        content.append(checkbox)

        status = Gtk.Label(label="Status: idle")
        content.append(status)

        activate = Gtk.Button(label="Activate safely")

        def on_activate(_button: Gtk.Button) -> None:
            checkbox.set_active(True)
            status.set_label("Status: activated")

        activate.connect("clicked", on_activate)
        content.append(activate)

        external = Gtk.Button(label="Change username externally")
        external.connect(
            "clicked",
            lambda _button: username_ref["widget"].set_text("external-gtk"),
        )
        content.append(external)

        replace = Gtk.Button(label="Replace username control")

        def replace_username(_button: Gtk.Button) -> None:
            old = username_ref["widget"]
            replacement = Gtk.Entry()
            replacement.set_text("replacement-gtk")
            replacement.update_property([Gtk.AccessibleProperty.LABEL], ["Username"])
            content.remove(old)
            content.insert_child_after(replacement, heading)
            username_ref["widget"] = replacement

        replace.connect("clicked", replace_username)
        content.append(replace)

        items = Gtk.StringList.new(["Alpha", "Beta", "Gamma"])
        selection = Gtk.SingleSelection.new(items)
        factory = Gtk.SignalListItemFactory()
        factory.connect(
            "setup",
            lambda _factory, list_item: list_item.set_child(Gtk.Label()),
        )
        factory.connect(
            "bind",
            lambda _factory, list_item: list_item.get_child().set_label(
                list_item.get_item().get_string()
            ),
        )
        item_list = Gtk.ListView.new(selection, factory)
        item_list.update_property([Gtk.AccessibleProperty.LABEL], ["Demo items"])
        content.append(item_list)

        window.set_child(content)
        window.present()


if __name__ == "__main__":
    raise SystemExit(LiveFixture().run(None))
