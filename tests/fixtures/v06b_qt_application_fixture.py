#!/usr/bin/env python3
"""Exact-current-application fixture for v0.6B recovery evidence."""

import argparse
import pathlib
import sys

from PyQt6.QtCore import QCoreApplication
from PyQt6.QtWidgets import QApplication, QCheckBox, QLabel, QMainWindow, QVBoxLayout, QWidget


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--title", required=True)
    parser.add_argument("--control", required=True)
    parser.add_argument("--state-file", required=True, type=pathlib.Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    QCoreApplication.setApplicationName(args.name)
    app = QApplication(sys.argv[:1])
    app.setApplicationDisplayName(args.name)

    window = QMainWindow()
    window.setWindowTitle(args.title)
    content = QWidget()
    layout = QVBoxLayout(content)
    layout.addWidget(QLabel("Runtime recovery fixture"))
    control = QCheckBox(args.control)
    layout.addWidget(control)
    window.setCentralWidget(content)

    def record(checked: bool) -> None:
        args.state_file.write_text(
            f"checked={str(checked).lower()}\n", encoding="utf-8"
        )

    record(False)
    control.toggled.connect(record)
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
