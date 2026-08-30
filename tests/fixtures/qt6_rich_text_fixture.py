#!/usr/bin/env python3
"""Isolated Qt6 QTextEdit probe for AT-SPI rich-text compatibility.

This fixture is deliberately separate from qt6_live_fixture.py. Some Qt 6.4
builds crash in the accessibility bridge while serving Text interface calls
for QTextEdit; isolating it keeps the stable Qt controls regression usable.
"""

import sys

from PyQt6.QtCore import QCoreApplication
from PyQt6.QtWidgets import QApplication, QTextEdit


QCoreApplication.setApplicationName("gui2tui-qt-rich-text-fixture")


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI Qt Rich Text Fixture")
    article = QTextEdit()
    article.setWindowTitle("GUI2TUI Qt Rich Text Fixture")
    article.setReadOnly(True)
    article.setAccessibleName("Qt rich text article")
    article.setPlainText(
        "Qt semantic content first paragraph.\n\n"
        "Second paragraph is loaded through the generic AT-SPI Text interface.\n\n"
        "Third paragraph proves that a Document role is not required."
    )
    article.resize(560, 240)
    article.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
