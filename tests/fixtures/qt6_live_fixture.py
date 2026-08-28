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

        layout.addWidget(QLabel("Username"))
        username = QLineEdit("alice")
        username.setAccessibleName("Username")
        layout.addWidget(username)

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


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Qt Fixture")
    window = QtFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
