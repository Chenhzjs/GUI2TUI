"""Packaged, harmless Qt6 Value fixture; no repository-relative assets."""

import sys

try:
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

    HORIZONTAL = Qt.Orientation.Horizontal
except ImportError:
    from PyQt5.QtCore import QCoreApplication, Qt
    from PyQt5.QtWidgets import (
        QApplication,
        QLabel,
        QMainWindow,
        QProgressBar,
        QSlider,
        QVBoxLayout,
        QWidget,
    )

    HORIZONTAL = Qt.Horizontal


QCoreApplication.setApplicationName("gui2tui-release-value-demo")


class ValueDemo(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("GUI2TUI Release Value Demo")
        content = QWidget()
        layout = QVBoxLayout(content)
        value = QSlider(HORIZONTAL)
        value.setAccessibleName("Release value")
        value.setRange(0, 10)
        value.setSingleStep(1)
        value.setValue(4)
        progress = QProgressBar()
        progress.setAccessibleName("Release progress")
        progress.setRange(0, 10)
        progress.setValue(4)
        layout.addWidget(QLabel("Bounded release Value"))
        layout.addWidget(value)
        layout.addWidget(QLabel("Informational progress remains read only"))
        layout.addWidget(progress)
        self.setCentralWidget(content)


app = QApplication(sys.argv)
window = ValueDemo()
window.show()
raise SystemExit(app.exec())
