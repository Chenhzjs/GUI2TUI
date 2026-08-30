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

        username_label = Gtk.Label(label="_Username", use_underline=True)
        content.append(username_label)
        username = Gtk.Entry()
        username.set_placeholder_text("Username")
        username.set_text("alice")
        username.update_property([Gtk.AccessibleProperty.LABEL], ["Username"])
        username_label.set_mnemonic_widget(username)
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

        theme_label = Gtk.Label(label="Theme")
        content.append(theme_label)
        light = Gtk.CheckButton(label="Light")
        dark = Gtk.CheckButton(label="Dark")
        dark.set_group(light)
        light.set_active(True)
        content.append(light)
        content.append(dark)

        combo = Gtk.ComboBoxText()
        for item in ["Alpha", "Beta", "Gamma"]:
            combo.append_text(item)
        combo.set_active(0)
        combo.update_property([Gtk.AccessibleProperty.LABEL], ["Demo choice"])
        content.append(combo)

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

        open_dialog = Gtk.Button(label="Open modal dialog")

        def show_dialog(_button: Gtk.Button) -> None:
            dialog = Gtk.Window(title="GTK Fixture Dialog", transient_for=window, modal=True)
            box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
            box.append(Gtk.Label(label="Dialog content"))
            close = Gtk.Button(label="Close dialog")
            close.connect("clicked", lambda _close: dialog.close())
            box.append(close)
            dialog.set_child(box)
            dialog.present()

        open_dialog.connect("clicked", show_dialog)
        content.append(open_dialog)

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

        rich_text = Gtk.TextView()
        rich_text.set_editable(False)
        rich_text.get_buffer().set_text(
            "GTK semantic content first paragraph.\n\n"
            "Second paragraph is loaded through the generic AT-SPI Text interface.\n\n"
            "Third paragraph proves that a Document role is not required."
        )
        rich_text.update_property([Gtk.AccessibleProperty.LABEL], ["GTK rich text article"])
        content.append(rich_text)

        window.set_child(content)
        window.present()


if __name__ == "__main__":
    raise SystemExit(LiveFixture().run(None))
