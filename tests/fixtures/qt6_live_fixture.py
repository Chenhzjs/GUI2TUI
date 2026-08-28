#!/usr/bin/env python3
"""Small, non-destructive Qt6 Widgets application for AT-SPI validation."""

import sys

from PyQt6.QtCore import QCoreApplication
from PyQt6.QtWidgets import (
    QApplication,
    QCheckBox,
    QLabel,
    QLineEdit,
    QListWidget,
    QMainWindow,
    QPushButton,
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


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Qt Fixture")
    window = QtFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
