#!/usr/bin/env python3
"""Qt6 surface/resource churn fixture using public accessibility semantics."""

import argparse
import json
import pathlib
import sys

from PyQt6.QtCore import QCoreApplication, Qt, QTimer
from PyQt6.QtWidgets import (
    QApplication,
    QCheckBox,
    QDialog,
    QLabel,
    QMainWindow,
    QPushButton,
    QVBoxLayout,
    QWidget,
)


class ResourceFixture(QMainWindow):
    def __init__(self, state_file: pathlib.Path) -> None:
        super().__init__()
        self.state_file = state_file
        self.dialog = None
        self.secondary = None
        self.state = {
            "fresh_toggles": 0,
            "modal_opened": 0,
            "modal_closed": 0,
            "modal_open": False,
            "secondary_opened": 0,
            "secondary_closed": 0,
            "secondary_open": False,
            "secondary_toggles": 0,
        }
        self.setWindowTitle("Continuity Window")
        self.setMinimumSize(480, 320)
        content = QWidget()
        layout = QVBoxLayout(content)
        layout.addWidget(QLabel("v0.6C bounded continuity fixture"))
        self.status = QLabel("Status: current")
        layout.addWidget(self.status)

        fresh = QCheckBox("Fresh operation")
        fresh.stateChanged.connect(self.fresh_operation)
        layout.addWidget(fresh)

        open_modal = QPushButton("Open churn modal")
        open_modal.clicked.connect(self.open_modal)
        layout.addWidget(open_modal)

        open_secondary = QPushButton("Open secondary surface")
        open_secondary.clicked.connect(self.open_secondary)
        layout.addWidget(open_secondary)
        self.setCentralWidget(content)
        self.write_state()

    def write_state(self) -> None:
        temporary = self.state_file.with_suffix(".tmp")
        temporary.write_text(json.dumps(self.state, sort_keys=True), encoding="utf-8")
        temporary.replace(self.state_file)

    def fresh_operation(self, _value: int) -> None:
        self.state["fresh_toggles"] += 1
        self.status.setText(f"Status: fresh {self.state['fresh_toggles']}")
        self.write_state()

    def open_modal(self) -> None:
        if self.dialog is not None:
            return
        dialog = QDialog(self)
        dialog.setWindowTitle("Continuity Dialog")
        dialog.setModal(True)
        dialog.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        layout = QVBoxLayout(dialog)
        layout.addWidget(QLabel("Same-looking transient surface"))
        close = QPushButton("Close churn modal")
        close.clicked.connect(self.close_modal)
        layout.addWidget(close)
        dialog.destroyed.connect(self.modal_destroyed)
        self.dialog = dialog
        self.state["modal_opened"] += 1
        self.state["modal_open"] = True
        self.status.setText(f"Status: modal {self.state['modal_opened']}")
        self.write_state()
        dialog.open()
        QTimer.singleShot(0, close.setFocus)

    def close_modal(self) -> None:
        if self.dialog is not None:
            self.dialog.close()

    def modal_destroyed(self) -> None:
        self.dialog = None
        self.state["modal_closed"] += 1
        self.state["modal_open"] = False
        self.status.setText(f"Status: modal closed {self.state['modal_closed']}")
        self.write_state()
        self.activateWindow()

    def open_secondary(self) -> None:
        if self.secondary is not None:
            return
        secondary = QMainWindow(self)
        # Every replacement intentionally has the same descriptive surface.
        secondary.setWindowTitle("Continuity Window")
        secondary.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        content = QWidget()
        layout = QVBoxLayout(content)
        layout.addWidget(QLabel("Same-looking secondary surface"))
        operation = QCheckBox("Secondary surface operation")
        operation.stateChanged.connect(self.secondary_operation)
        layout.addWidget(operation)
        close = QPushButton("Close secondary surface")
        close.clicked.connect(secondary.close)
        layout.addWidget(close)
        secondary.setCentralWidget(content)
        secondary.destroyed.connect(self.secondary_destroyed)
        self.secondary = secondary
        self.state["secondary_opened"] += 1
        self.state["secondary_open"] = True
        self.status.setText(f"Status: secondary {self.state['secondary_opened']}")
        self.write_state()
        secondary.show()
        secondary.raise_()
        secondary.activateWindow()
        QTimer.singleShot(0, operation.setFocus)

    def secondary_operation(self, _value: int) -> None:
        self.state["secondary_toggles"] += 1
        self.write_state()

    def secondary_destroyed(self) -> None:
        self.secondary = None
        self.state["secondary_closed"] += 1
        self.state["secondary_open"] = False
        self.status.setText(
            f"Status: secondary closed {self.state['secondary_closed']}"
        )
        self.write_state()
        self.activateWindow()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--state-file", type=pathlib.Path, required=True)
    args = parser.parse_args()
    QCoreApplication.setApplicationName("gui2tui-v06c-resource")
    application = QApplication(sys.argv[:1])
    application.setApplicationDisplayName("gui2tui-v06c-resource")
    window = ResourceFixture(args.state_file)
    window.show()
    return application.exec()


if __name__ == "__main__":
    raise SystemExit(main())
