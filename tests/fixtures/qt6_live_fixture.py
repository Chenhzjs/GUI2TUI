#!/usr/bin/env python3
"""Small, non-destructive Qt6 Widgets application for AT-SPI validation."""

import sys

from PyQt6.QtCore import QCoreApplication, Qt
from PyQt6.QtWidgets import (
    QApplication,
    QCheckBox,
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QButtonGroup,
    QLabel,
    QLineEdit,
    QListWidget,
    QMainWindow,
    QProgressBar,
    QPushButton,
    QRadioButton,
    QSlider,
    QVBoxLayout,
    QWidget,
)


QCoreApplication.setApplicationName("gui2tui-qt-fixture")


class QtFixture(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("GUI2TUI Qt Fixture")
        self.setMinimumSize(420, 420)

        content = QWidget()
        layout = QVBoxLayout(content)

        layout.addWidget(QLabel("Phase 2 Qt compatibility"))

        self.layout = layout
        self.username_label = QLabel("Username")
        layout.addWidget(self.username_label)
        self.username = QLineEdit("alice")
        self.username.setAccessibleName("Username")
        self.username.setAccessibleDescription("Account name used by the fixture")
        self.username_label.setBuddy(self.username)
        layout.addWidget(self.username)

        long_input = QLineEdit("L" * 300 + "-qt-tail")
        long_input.setAccessibleName("Long input")
        layout.addWidget(long_input)

        layout.addWidget(QLabel("Password"))
        password = QLineEdit("phase-two-secret")
        password.setEchoMode(QLineEdit.EchoMode.Password)
        password.setAccessibleName("Password")
        layout.addWidget(password)

        self.checkbox = QCheckBox("Enable feature")
        layout.addWidget(self.checkbox)

        layout.addWidget(QLabel("Theme"))
        group = QButtonGroup(self)
        light = QRadioButton("Light")
        dark = QRadioButton("Dark")
        light.setChecked(True)
        group.addButton(light)
        group.addButton(dark)
        layout.addWidget(light)
        layout.addWidget(dark)

        combo = QComboBox()
        combo.setAccessibleName("Demo choice")
        combo.addItems(["Alpha", "Beta", "Gamma"])
        layout.addWidget(combo)

        value = QSlider(Qt.Orientation.Horizontal)
        value.setAccessibleName("Probe value")
        value.setRange(0, 10)
        value.setSingleStep(1)
        value.setValue(4)
        layout.addWidget(value)

        progress = QProgressBar()
        progress.setAccessibleName("Probe progress")
        progress.setRange(0, 10)
        progress.setValue(4)
        layout.addWidget(progress)

        self.status = QLabel("Status: idle")
        layout.addWidget(self.status)

        activate = QPushButton("Activate safely")
        activate.clicked.connect(self.activate_safely)
        layout.addWidget(activate)

        external = QPushButton("Change username externally")
        external.clicked.connect(lambda: self.username.setText("external-qt"))
        layout.addWidget(external)

        replace = QPushButton("Replace username control")
        replace.clicked.connect(self.replace_username)
        layout.addWidget(replace)

        dialog_button = QPushButton("Open modal dialog")
        dialog_button.clicked.connect(self.open_dialog)
        layout.addWidget(dialog_button)

        items = QListWidget()
        items.setAccessibleName("Demo items")
        items.addItems(["Alpha", "Beta", "Gamma"])
        layout.addWidget(items)

        tools = self.menuBar().addMenu("Tools")
        demo = tools.addAction("Activate Demo")
        demo.triggered.connect(lambda: self.status.setText("Status: menu activated"))

        self.setCentralWidget(content)

    def activate_safely(self) -> None:
        self.checkbox.setChecked(True)
        self.status.setText("Status: activated")

    def replace_username(self) -> None:
        old = self.username
        replacement = QLineEdit("replacement-qt")
        replacement.setAccessibleName("Username")
        self.layout.replaceWidget(old, replacement)
        old.deleteLater()
        self.username = replacement

    def open_dialog(self) -> None:
        dialog = QDialog(self)
        dialog.setWindowTitle("Qt Fixture Dialog")
        dialog.setModal(True)
        layout = QVBoxLayout(dialog)
        layout.addWidget(QLabel("Dialog content"))
        buttons = QDialogButtonBox(QDialogButtonBox.StandardButton.Close)
        buttons.rejected.connect(dialog.reject)
        layout.addWidget(buttons)
        dialog.exec()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Qt Fixture")
    window = QtFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
