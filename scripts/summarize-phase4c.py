#!/usr/bin/env python3
"""Curate counts-only live evidence; do not copy profiles, payloads or GUI logs."""
import json
import pathlib
import sys

destination = pathlib.Path(sys.argv[1])
reports = []
for directory in sys.argv[2:]:
    root = pathlib.Path(directory)
    report = json.loads((root / "report.json").read_text())
    report["evidence_directory"] = root.name
    tree = root / "tree.txt"
    if tree.exists():
        lines = tree.read_text().splitlines()
        # Normal Inspector only prints locator IDs for nodes with actions.
        # Counting IDs is NOT a semantic node count; correct early harness reports.
        report["nodes"] = sum(bool(line) and "… [" not in line for line in lines)
        report["advertised_action_nodes"] = sum("id=atspi1_" in line for line in lines)
    reports.append(report)
destination.write_text(json.dumps(reports, indent=2) + "\n")
