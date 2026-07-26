#!/usr/bin/env python3
"""mdbook preprocessor that adds the TIPs (Tel Improvement Proposals) to the
*dev* book only.

The TIP files in ``src/tips/`` are deliberately left out of ``SUMMARY.md`` so
they never appear in the public book. When the ``TEL_DEV`` environment variable
is set (the dev deployment / ``TEL_DEV=1 mdbook build``), this preprocessor
appends them as a "Proposals (TIPs)" part so reviewers can browse them on the
dev site.

When ``TEL_DEV`` is unset the book is passed through untouched, so the public
build is unaffected.

Ordered to run *after* ``strip-todos`` (see book.toml) so the TIPs keep their
``TODO(open):`` markers — the open questions are the whole point of a draft TIP.
"""
import json
import os
import re
import sys
from pathlib import Path

PART_TITLE = "Proposals (TIPs)"


def _title_of(text, fallback):
    """Use the file's first ``# `` heading as the chapter name."""
    for line in text.splitlines():
        m = re.match(r'#\s+(.*\S)\s*$', line)
        if m:
            return m.group(1)
    return fallback


def _chapter(path_in_src, content):
    """Build the Chapter dict mdbook's HTML renderer expects."""
    return {
        "Chapter": {
            "name": _title_of(content, path_in_src.stem),
            "content": content,
            "number": None,
            "sub_items": [],
            "path": path_in_src.as_posix(),
            "source_path": path_in_src.as_posix(),
            "parent_names": [],
        }
    }


def inject(ctx, book):
    tips_dir = Path(ctx["root"]) / ctx["config"]["book"].get("src", "src") / "tips"
    if not tips_dir.is_dir():
        return book

    sections = book.setdefault("sections", [])
    sections.append({"PartTitle": PART_TITLE})

    # README first (landing page), then the numbered TIPs in order.
    readme = tips_dir / "README.md"
    if readme.is_file():
        sections.append(_chapter(Path("tips/README.md"), readme.read_text(encoding="utf-8")))
    for tip in sorted(tips_dir.glob("[0-9]*.md")):
        sections.append(_chapter(Path("tips") / tip.name, tip.read_text(encoding="utf-8")))

    return book


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "supports":
        # Render TIPs for every renderer that asks.
        sys.exit(0)

    ctx, book = json.load(sys.stdin)
    if os.environ.get("TEL_DEV"):
        book = inject(ctx, book)
    json.dump(book, sys.stdout)


if __name__ == "__main__":
    main()
