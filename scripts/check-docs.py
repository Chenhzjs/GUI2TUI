#!/usr/bin/env python3
"""Check repository Markdown links and public landing-page privacy boundaries."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MARKDOWN_FILES = [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]
LINK_RE = re.compile(r"!?(?:\[[^]]*\])\(([^)]+)\)")
HTML_SRC_RE = re.compile(r"\b(?:src|href)=[\"']([^\"']+)[\"']")
EXTERNAL_PREFIXES = ("http://", "https://", "mailto:", "#")
PUBLIC_FILES = {
    ROOT / "README.md",
    ROOT / "docs" / "release-notes-v0.1.md",
    ROOT / "docs" / "release-notes-v0.1.0.md",
    ROOT / "docs" / "demo" / "README.md",
}
PRIVATE_PATTERNS = (
    "/Users/",
    "/home/chenhz",
    "/home/runner/work",
    "localhost:",
    "gho_",
    "fixture-secret",
    "phase-zero-secret",
    "browser-phase-secret",
)


def link_target(raw: str) -> str:
    target = raw.strip().strip("<>")
    if " " in target and not target.startswith(EXTERNAL_PREFIXES):
        target = target.split(" ", 1)[0]
    return target.split("#", 1)[0].split("?", 1)[0]


def main() -> int:
    failures: list[str] = []
    checked = 0
    for path in MARKDOWN_FILES:
        text = path.read_text(encoding="utf-8")
        raw_links = LINK_RE.findall(text) + HTML_SRC_RE.findall(text)
        for raw in raw_links:
            target = link_target(raw)
            if not target or target.startswith(EXTERNAL_PREFIXES):
                continue
            checked += 1
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                failures.append(f"{path.relative_to(ROOT)}: missing {raw}")
        if path in PUBLIC_FILES:
            for pattern in PRIVATE_PATTERNS:
                if pattern in text:
                    failures.append(
                        f"{path.relative_to(ROOT)}: private pattern {pattern!r}"
                    )

    if failures:
        print("DOCUMENT_AUDIT=FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(f"DOCUMENT_AUDIT=PASS files={len(MARKDOWN_FILES)} local_links={checked}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
