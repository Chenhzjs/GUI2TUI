#!/usr/bin/env python3
"""Qt6 image fixture; production discovery may use AT-SPI evidence only."""
import pathlib
import sys
from PyQt6.QtGui import QPixmap
from PyQt6.QtWidgets import QApplication, QLabel, QVBoxLayout, QWidget

resource = pathlib.Path(__file__).parent / "modality" / "architecture.svg"
app = QApplication(sys.argv)
window = QWidget()
window.setWindowTitle("GUI2TUI Qt Modality Fixture")
layout = QVBoxLayout(window)
layout.addWidget(QLabel("Local architecture image"))
image = QLabel()
image.setAccessibleName("Architecture diagram")
image.setPixmap(QPixmap(str(resource)))
layout.addWidget(image)
window.show()
sys.exit(app.exec())
