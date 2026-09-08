#!/usr/bin/env python3
"""Bounded GTK4 selection fixture for v0.5A live qualification."""

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402


GLib.set_prgname("gui2tui-v05a-gtk-selection")
GLib.set_application_name("GUI2TUI v0.5A GTK Selection")


def list_factory() -> Gtk.SignalListItemFactory:
    factory = Gtk.SignalListItemFactory()
    factory.connect("setup", lambda _factory, item: item.set_child(Gtk.Label()))
    factory.connect(
        "bind",
        lambda _factory, item: item.get_child().set_label(
            item.get_item().get_string()
        ),
    )
    return factory


class SelectionFixture(Gtk.Application):
    def __init__(self) -> None:
        super().__init__(application_id="org.gui2tui.V05aGtkSelection")

    def do_activate(self) -> None:
        window = Gtk.ApplicationWindow(application=self)
        window.set_title("GUI2TUI v0.5A GTK Selection")
        window.set_default_size(520, 440)
        content = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)

        self.items = Gtk.StringList.new(["Alpha", "Beta", "Gamma"])
        self.selection = Gtk.SingleSelection.new(self.items)
        self.selection.set_selected(0)
        self.list_view = Gtk.ListView.new(self.selection, list_factory())
        self.list_view.update_property(
            [Gtk.AccessibleProperty.LABEL], ["Current single-selection items"]
        )
        content.append(self.list_view)

        self.status = Gtk.Label(label="Selected: Alpha")
        self.selection.connect("selection-changed", self.selection_changed)
        content.append(self.status)

        reorder = Gtk.Button(label="Reorder current items")
        reorder.connect("clicked", self.reorder)
        content.append(reorder)

        replace = Gtk.Button(label="Replace Beta with a new Beta")
        replace.connect("clicked", self.replace_beta)
        content.append(replace)

        reset = Gtk.Button(label="Reset current items")
        reset.connect("clicked", self.reset)
        content.append(reset)

        virtual_model = Gtk.StringList.new([f"Virtual {index:03d}" for index in range(200)])
        self.virtual_selection = Gtk.SingleSelection.new(virtual_model)
        virtual_view = Gtk.ListView.new(self.virtual_selection, list_factory())
        virtual_view.update_property(
            [Gtk.AccessibleProperty.LABEL], ["Virtualized two-hundred items"]
        )
        scroller = Gtk.ScrolledWindow()
        scroller.set_min_content_height(120)
        scroller.set_max_content_height(120)
        scroller.set_child(virtual_view)
        content.append(scroller)

        multi_model = Gtk.StringList.new(["Multi Alpha", "Multi Beta", "Multi Gamma"])
        self.multi_selection = Gtk.MultiSelection.new(multi_model)
        multi_view = Gtk.ListView.new(self.multi_selection, list_factory())
        multi_view.update_property(
            [Gtk.AccessibleProperty.LABEL], ["Explicit multiselect items"]
        )
        multi_scroller = Gtk.ScrolledWindow()
        multi_scroller.set_min_content_height(72)
        multi_scroller.set_max_content_height(72)
        multi_scroller.set_child(multi_view)
        content.append(multi_scroller)

        window.set_child(content)
        window.present()

    def selection_changed(self, _selection, _position, _count) -> None:
        selected = self.selection.get_selected_item()
        label = selected.get_string() if selected is not None else "none"
        self.status.set_label(f"Selected: {label}")

    def reorder(self, _button) -> None:
        current = [self.items.get_string(index) for index in range(self.items.get_n_items())]
        reordered = [current[1], current[0], current[2]]
        self.items.splice(0, self.items.get_n_items(), reordered)
        self.selection.set_selected(2)
        self.status.set_label("Structure: reordered; selected Gamma")

    def replace_beta(self, _button) -> None:
        current = [self.items.get_string(index) for index in range(self.items.get_n_items())]
        beta = next((index for index, label in enumerate(current) if label == "Beta"), None)
        if beta is not None:
            self.items.splice(beta, 1, ["Beta"])
        self.selection.set_selected(2)
        self.status.set_label("Structure: Beta replaced; selected Gamma")

    def reset(self, _button) -> None:
        self.items.splice(0, self.items.get_n_items(), ["Alpha", "Beta", "Gamma"])
        self.selection.set_selected(0)
        self.status.set_label("Selected: Alpha")


if __name__ == "__main__":
    raise SystemExit(SelectionFixture().run(None))
