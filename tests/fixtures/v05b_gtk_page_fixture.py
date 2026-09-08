#!/usr/bin/env python3
"""Small GTK4 notebook fixture for exact current-page validation."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-v05b-gtk-pages")
GLib.set_application_name("GUI2TUI v0.5B GTK Pages")


class PageFixture(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="org.gui2tui.V05bGtkPages")

    def do_activate(self) -> None:
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI v0.5B GTK Pages")
        window.set_default_size(520, 320)

        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        status = Gtk.Label(label="Status: General")
        root.append(status)

        notebook = Gtk.Notebook()
        notebook.update_property([Gtk.AccessibleProperty.LABEL], ["Task pages"])

        general = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        general_action = Gtk.Button(label="General page action")
        general_action.connect(
            "clicked", lambda _button: status.set_label("Status: General action")
        )
        general.append(general_action)
        notebook.append_page(general, Gtk.Label(label="General"))

        def advanced_page() -> Gtk.Box:
            advanced = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
            advanced_action = Gtk.CheckButton(label="Advanced page setting")
            advanced_action.connect(
                "toggled",
                lambda button: status.set_label(
                    f"Status: Advanced setting {button.get_active()}"
                ),
            )
            advanced.append(advanced_action)
            return advanced

        notebook.append_page(advanced_page(), Gtk.Label(label="Advanced"))

        def replace_advanced(_button: Gtk.Button) -> None:
            # Validation-only lifecycle control: the replacement keeps the
            # descriptive label but receives a new accessible object. It
            # exercises stale PageTab authority without production guessing.
            notebook.remove_page(1)
            notebook.insert_page(advanced_page(), Gtk.Label(label="Advanced"), 1)
            notebook.set_current_page(0)
            status.set_label("Status: Advanced page replaced")

        replace_button = Gtk.Button(label="Replace Advanced page")
        replace_button.connect("clicked", replace_advanced)
        general.append(replace_button)

        root.append(notebook)
        window.set_child(root)
        window.present()


if __name__ == "__main__":
    raise SystemExit(PageFixture().run(None))
