#!/usr/bin/env python3
"""Check required design records and repository-local Markdown links."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote


ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"(?<!!)\[[^]]+\]\(([^)]+)\)")
REQUIRED = (
    "docs/mojocolib-training-prd.md",
    "docs/threat-model.md",
    "docs/adr/0001-repository-boundary.md",
    "docs/adr/0002-canonical-encoding.md",
    "docs/adr/0003-reference-training-lock.md",
    "docs/adr/0004-executor-isolation.md",
    "docs/adr/0005-ledger-units.md",
    "docs/adr/0006-checkpoint-compensation.md",
    "docs/adr/0007-operator-interface.md",
    "docs/adr/0008-tariff-comparator.md",
    "docs/adr/0009-licensing-posture.md",
)
REQUIRED_LICENSE_FILES = (
    "LICENSE",
    "NOTICE",
    "THIRD_PARTY_NOTICES.md",
    "engine/colibri/LICENSE",
    "fixtures/tiny_training/LICENSE",
    "licenses/shadcn-ui-MIT.txt",
    "licenses/Unicode-3.0.txt",
)


def markdown_files() -> list[Path]:
    candidates = list(ROOT.glob("*.md"))
    for directory in (ROOT / "docs", ROOT / "fixtures"):
        candidates.extend(directory.rglob("*.md"))
    return sorted(path for path in candidates if path.is_file())


def local_target(source: Path, raw_target: str) -> Path | None:
    target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
    if target.startswith(("http://", "https://", "mailto:", "#")):
        return None
    target = unquote(target.split("#", maxsplit=1)[0])
    if not target:
        return None
    return (source.parent / target).resolve()


def main() -> int:
    failures: list[str] = []
    for required in REQUIRED:
        if not (ROOT / required).is_file():
            failures.append(f"missing required document: {required}")

    for required in REQUIRED_LICENSE_FILES:
        if not (ROOT / required).is_file():
            failures.append(f"missing required licence file: {required}")

    for source in markdown_files():
        text = source.read_text(encoding="utf-8")
        for line_number, line in enumerate(text.splitlines(), start=1):
            for match in LINK.finditer(line):
                target = local_target(source, match.group(1))
                if target is not None and not target.exists():
                    failures.append(
                        f"{source.relative_to(ROOT)}:{line_number}: missing link target "
                        f"{match.group(1)}"
                    )

    if failures:
        print("documentation checks failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print(f"documentation checks passed ({len(markdown_files())} Markdown files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
