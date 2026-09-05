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
    QGroupBox,
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

        realization_trigger = QPushButton("Toggle descendant realization")
        realization_trigger.clicked.connect(self.toggle_descendant_realization)
        layout.addWidget(realization_trigger)
        self.descendant_group = QGroupBox("Descendant container")
        self.descendant_layout = QVBoxLayout(self.descendant_group)
        self.realized_descendant = None
        layout.addWidget(self.descendant_group)

        unrelated_trigger = QPushButton("Toggle unrelated realization")
        unrelated_trigger.clicked.connect(self.toggle_unrelated_realization)
        layout.addWidget(unrelated_trigger)
        self.unrelated_group = QGroupBox("Unrelated container")
        self.unrelated_layout = QVBoxLayout(self.unrelated_group)
        self.unrelated_descendant = None
        layout.addWidget(self.unrelated_group)

        self.tools = self.menuBar().addMenu("Tools")
        demo = self.tools.addAction("Activate Demo")
        self.menu_activation_count = 0
        demo.triggered.connect(self.activate_menu_item)

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

    def activate_menu_item(self) -> None:
        self.menu_activation_count += 1
        self.status.setText(f"Status: menu activated {self.menu_activation_count}")
        self.tools.close()

    def toggle_descendant_realization(self) -> None:
        if self.realized_descendant is None:
            control = QCheckBox("Realized descendant toggle")
            control.stateChanged.connect(
                lambda state: self.status.setText(f"Status: descendant state {state}")
            )
            self.descendant_layout.addWidget(control)
            self.realized_descendant = control
            self.status.setText("Status: descendant realized")
        else:
            control = self.realized_descendant
            self.realized_descendant = None
            self.descendant_layout.removeWidget(control)
            control.setParent(None)
            control.deleteLater()
            self.status.setText("Status: descendant removed")

    def toggle_unrelated_realization(self) -> None:
        if self.unrelated_descendant is None:
            control = QPushButton("Unrelated realized action")
            control.clicked.connect(
                lambda: self.status.setText("Status: unrelated action activated")
            )
            self.unrelated_layout.addWidget(control)
            self.unrelated_descendant = control
            self.status.setText("Status: unrelated realized")
        else:
            control = self.unrelated_descendant
            self.unrelated_descendant = None
            self.unrelated_layout.removeWidget(control)
            control.setParent(None)
            control.deleteLater()
            self.status.setText("Status: unrelated removed")


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Qt Fixture")
    window = QtFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
