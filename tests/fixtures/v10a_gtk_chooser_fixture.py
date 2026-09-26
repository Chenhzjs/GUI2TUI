#!/usr/bin/env python3
"""Bounded GTK3 caller and native chooser for v1.0A final-package evidence.

This is validation-only.  The caller exposes the result through an ordinary
accessible label; GUI2TUI production must not inspect the fixture's files or
use this fixture's names as semantic rules.
"""

from __future__ import annotations

import pathlib
import shutil
import tempfile

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk


class ChooserFixture(Gtk.Window):
    def __init__(self) -> None:
        super().__init__(title="GUI2TUI v1.0A GTK Chooser Fixture")
        self.set_default_size(520, 260)
        self.root = pathlib.Path(tempfile.mkdtemp(prefix="gui2tui-v10a-chooser-"))
        (self.root / "alpha.txt").write_text("alpha\n", encoding="utf-8")
        (self.root / "beta.txt").write_text("beta\n", encoding="utf-8")
        (self.root / "folder-a").mkdir()
        (self.root / "folder-a" / "nested.txt").write_text("nested\n", encoding="utf-8")
        (self.root / "folder-b").mkdir()

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        box.set_border_width(16)
        self.add(box)
        box.pack_start(Gtk.Label(label="v1.0A public GTK chooser caller"), False, False, 0)

        open_button = Gtk.Button(label="Open file")
        open_button.connect("clicked", self.open_file)
        box.pack_start(open_button, False, False, 0)

        folder_button = Gtk.Button(label="Choose folder")
        folder_button.connect("clicked", self.choose_folder)
        box.pack_start(folder_button, False, False, 0)

        cancel_button = Gtk.Button(label="Reset result")
        cancel_button.connect("clicked", lambda _button: self.set_result("CANCELLED"))
        box.pack_start(cancel_button, False, False, 0)

        self.result = Gtk.Label(label="Result: idle")
        self.result.set_selectable(False)
        box.pack_start(self.result, False, False, 0)

        self.connect("destroy", self._destroy)

    def set_result(self, value: str) -> None:
        self.result.set_text(f"Result: {value}")

    def chooser(self, action: Gtk.FileChooserAction, title: str, accept: str) -> None:
        dialog = Gtk.FileChooserDialog(
            title=title,
            parent=self,
            action=action,
            buttons=(
                "Cancel",
                Gtk.ResponseType.CANCEL,
                accept,
                Gtk.ResponseType.ACCEPT,
            ),
        )
        dialog.set_current_folder(str(self.root))
        dialog.set_select_multiple(False)
        def respond(_dialog: Gtk.FileChooserDialog, response: Gtk.ResponseType) -> None:
            if response == Gtk.ResponseType.ACCEPT:
                selected = dialog.get_filename()
                if selected:
                    self.set_result(
                        f"{'OPEN' if action == Gtk.FileChooserAction.OPEN else 'FOLDER'}:{pathlib.Path(selected).name}"
                    )
                else:
                    self.set_result("UNVERIFIED")
            elif response == Gtk.ResponseType.CANCEL:
                self.set_result("CANCELLED")
            dialog.destroy()

        dialog.connect("response", respond)
        dialog.show_all()

    def open_file(self, _button: Gtk.Button) -> None:
        self.chooser(Gtk.FileChooserAction.OPEN, "Open synthetic file", "Open")

    def choose_folder(self, _button: Gtk.Button) -> None:
        self.chooser(
            Gtk.FileChooserAction.SELECT_FOLDER,
            "Choose synthetic folder",
            "Select",
        )

    def _destroy(self, _window: Gtk.Window) -> None:
        shutil.rmtree(self.root, ignore_errors=True)
        Gtk.main_quit()


window = ChooserFixture()
window.show_all()
Gtk.main()
