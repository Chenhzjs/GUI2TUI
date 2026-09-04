#!/usr/bin/env python3
"""Small harmless Qt6 Value fixture used only by the v0.3 public demo."""

import sys

from PyQt6.QtCore import QCoreApplication, Qt
from PyQt6.QtWidgets import (
    QApplication,
    QLabel,
    QMainWindow,
    QProgressBar,
    QSlider,
    QVBoxLayout,
    QWidget,
)


QCoreApplication.setApplicationName("gui2tui-v03-value-demo")


class ValueDemo(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("GUI2TUI v0.3 — Verified Value")
        self.setMinimumSize(520, 420)

        content = QWidget()
        layout = QVBoxLayout(content)
        layout.setSpacing(24)

        heading = QLabel("Verified native Value")
        heading.setStyleSheet("font-size: 24px; font-weight: bold")
        layout.addWidget(heading)

        explanation = QLabel(
            "This harmless slider is authoritative GUI state.\n"
            "GUI2TUI changes it through public AT-SPI Value semantics."
        )
        explanation.setWordWrap(True)
        layout.addWidget(explanation)

        self.current = QLabel("Authoritative GUI value: 4")
        self.current.setStyleSheet("font-size: 20px")
        layout.addWidget(self.current)

        value = QSlider(Qt.Orientation.Horizontal)
        value.setAccessibleName("Demo value")
        value.setRange(0, 10)
        value.setSingleStep(1)
        value.setValue(4)
        value.valueChanged.connect(
            lambda number: self.current.setText(
                f"Authoritative GUI value: {number}"
            )
        )
        layout.addWidget(value)

        layout.addWidget(QLabel("Informational progress remains read only"))
        progress = QProgressBar()
        progress.setAccessibleName("Informational progress")
        progress.setRange(0, 10)
        progress.setValue(4)
        progress.setFormat("Read-only status: %v")
        layout.addWidget(progress)

        layout.addStretch()
        layout.addWidget(QLabel("Synthetic state only · no system settings"))
        self.setCentralWidget(content)


app = QApplication(sys.argv)
window = ValueDemo()
window.show()
raise SystemExit(app.exec())
