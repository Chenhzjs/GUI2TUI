#!/usr/bin/env python3
"""Controlled same-looking Qt lifecycle fixture for v0.6A live evidence."""

import os
import pathlib
import sys
import time

from PyQt6.QtCore import QCoreApplication, QEventLoop
from PyQt6.QtWidgets import QApplication, QCheckBox, QLabel, QMainWindow, QVBoxLayout, QWidget


QCoreApplication.setApplicationName("gui2tui-v06a-fixture")


def marker(name: str) -> pathlib.Path:
    return pathlib.Path(os.environ[name])


def wait_for_marker(name: str) -> None:
    target = marker(name)
    while not target.exists():
        QCoreApplication.processEvents(QEventLoop.ProcessEventsFlag.AllEvents, 20)
        time.sleep(0.01)


class RuntimeFixture(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("GUI2TUI Runtime Continuity Fixture")
        content = QWidget()
        self.layout = QVBoxLayout(content)

        self.status = QLabel("Status: initial")
        self.layout.addWidget(self.status)

        self.delayed = QCheckBox("Delayed toggle")
        self.delayed.toggled.connect(self.delayed_toggle)
        self.layout.addWidget(self.delayed)

        self.fresh = QCheckBox("Fresh toggle")
        self.fresh.toggled.connect(
            lambda checked: self.status.setText(f"Status: fresh={checked}")
        )
        self.layout.addWidget(self.fresh)

        self.replaceable = QCheckBox("Replaceable toggle")
        self.replaceable.toggled.connect(self.replace_target)
        self.layout.addWidget(self.replaceable)

        self.setCentralWidget(content)

    def delayed_toggle(self, checked: bool) -> None:
        if not checked:
            return
        # The public Toggle invocation returns, but no desired state becomes
        # authoritative. GUI2TUI's bounded observer therefore remains the
        # operation owner until the application is terminated by the probe.
        self.delayed.blockSignals(True)
        self.delayed.setChecked(False)
        self.delayed.blockSignals(False)
        self.status.setText("Status: delayed pending")
        marker("V06A_DELAY_READY").touch()

    def replace_target(self, checked: bool) -> None:
        if not checked:
            return
        old = self.replaceable
        replacement = QCheckBox("Replaceable toggle")
        replacement.toggled.connect(
            lambda checked: self.status.setText(f"Status: replacement={checked}")
        )
        self.layout.replaceWidget(old, replacement)
        old.hide()
        old.setParent(None)
        self.replaceable = replacement
        replacement.show()
        marker("V06A_REPLACE_READY").touch()
        wait_for_marker("V06A_REPLACE_RESUME")
        old.deleteLater()


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Runtime Continuity Fixture")
    window = RuntimeFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
