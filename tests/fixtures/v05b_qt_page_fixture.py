#!/usr/bin/env python3
"""Small Qt6 tab and ambiguous-tree fixture for v0.5B validation."""

import sys

from PyQt6.QtCore import QCoreApplication
from PyQt6.QtWidgets import (
    QApplication,
    QCheckBox,
    QLabel,
    QMainWindow,
    QPushButton,
    QTabWidget,
    QTreeWidget,
    QTreeWidgetItem,
    QVBoxLayout,
    QWidget,
)


QCoreApplication.setApplicationName("gui2tui-v05b-qt-pages")


class PageFixture(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("GUI2TUI v0.5B Qt Pages")
        self.setMinimumSize(520, 420)

        root = QWidget()
        layout = QVBoxLayout(root)
        self.status = QLabel("Status: General")
        layout.addWidget(self.status)

        self.tabs = QTabWidget()
        self.tabs.setAccessibleName("Task pages")
        self.tabs.currentChanged.connect(self.page_changed)

        general = QWidget()
        general_layout = QVBoxLayout(general)
        general_action = QPushButton("General page action")
        general_action.clicked.connect(
            lambda: self.status.setText("Status: General action")
        )
        general_layout.addWidget(general_action)
        self.tabs.addTab(general, "General")

        advanced = QWidget()
        advanced_layout = QVBoxLayout(advanced)
        advanced_action = QCheckBox("Advanced page setting")
        advanced_action.toggled.connect(
            lambda checked: self.status.setText(
                f"Status: Advanced setting {checked}"
            )
        )
        advanced_layout.addWidget(advanced_action)
        self.tabs.addTab(advanced, "Advanced")
        layout.addWidget(self.tabs)

        tree = QTreeWidget()
        tree.setAccessibleName("Ambiguous hierarchy")
        tree.setHeaderHidden(True)
        parent = QTreeWidgetItem(["Expandable parent"])
        parent.addChild(QTreeWidgetItem(["Tree child"]))
        tree.addTopLevelItem(parent)
        # Qt exposes realized tree rows only after the fixture has made the
        # branch current. The phase checks their public metadata; it never
        # invokes this widget through coordinate or input emulation.
        tree.expandItem(parent)
        layout.addWidget(tree)

        self.setCentralWidget(root)

    def page_changed(self, index: int) -> None:
        self.status.setText(f"Status: {self.tabs.tabText(index)}")


def main() -> int:
    app = QApplication(sys.argv)
    app.setApplicationDisplayName("GUI2TUI v0.5B Qt Pages")
    window = PageFixture()
    window.show()
    return app.exec()


if __name__ == "__main__":
    raise SystemExit(main())
