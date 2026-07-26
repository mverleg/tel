#!/usr/bin/env python3
"""
Rough estimate of how many printed A4 pages the teldoc book would be.

This is a heuristic, not a real typesetting pass. It reads the book the same
way mdBook does -- only the files referenced from SUMMARY.md -- and optionally
the injected TIPs (src/tips/*.md) when run for the dev build (TEL_DEV set or
--dev). It then weighs prose, code blocks, diagrams and headings separately,
since each consumes vertical space at a very different rate.

Assumed layout: A4, ~2.5cm margins, ~11pt body text.

Usage:
    estimate-pages.py [BOOK_SRC_DIR]
    TEL_DEV=1 estimate-pages.py [BOOK_SRC_DIR]
    estimate-pages.py --dev   [BOOK_SRC_DIR]
"""

import os
import re
import sys

# --- heuristic constants (tweak these if the estimate drifts) -------------
WORDS_PER_PROSE_PAGE = 550   # body text words on a full A4 page
CODE_LINES_PER_PAGE  = 48    # monospace lines in a displayed code block
MERMAID_PAGE         = 0.40  # average rendered diagram footprint
HEADING_PAGE         = 0.06  # whitespace a heading adds around itself
TABLE_ROW_PAGE       = 1 / 40.0
# --------------------------------------------------------------------------

LINK_RE = re.compile(r"\]\(([^)]+\.md)\)")


def summary_files(src_dir):
    """Files reachable from SUMMARY.md, in order, deduped."""
    summary = os.path.join(src_dir, "SUMMARY.md")
    seen, files = set(), []
    with open(summary, encoding="utf-8") as fh:
        for line in fh:
            for m in LINK_RE.finditer(line):
                rel = m.group(1).strip()
                if rel in seen:
                    continue
                seen.add(rel)
                path = os.path.join(src_dir, rel)
                if os.path.isfile(path):
                    files.append(path)
    return files


def tip_files(src_dir):
    tips = os.path.join(src_dir, "tips")
    if not os.path.isdir(tips):
        return []
    return [os.path.join(tips, f)
            for f in sorted(os.listdir(tips)) if f.endswith(".md")]


def measure(files):
    prose_words = code_lines = mermaid = headings = table_rows = 0
    in_code = False
    for path in files:
        with open(path, encoding="utf-8") as fh:
            for raw in fh:
                s = raw.strip()
                if s.startswith("```"):
                    if not in_code:
                        in_code = True
                        if "mermaid" in s:
                            mermaid += 1
                    else:
                        in_code = False
                    continue
                if in_code:
                    code_lines += 1
                elif s.startswith("#"):
                    headings += 1
                elif s.startswith("|") and s.endswith("|"):
                    table_rows += 1
                else:
                    prose_words += len(s.split())
    return dict(prose_words=prose_words, code_lines=code_lines,
                mermaid=mermaid, headings=headings, table_rows=table_rows)


def pages(m):
    return (m["prose_words"] / WORDS_PER_PROSE_PAGE
            + m["code_lines"] / CODE_LINES_PER_PAGE
            + m["mermaid"] * MERMAID_PAGE
            + m["headings"] * HEADING_PAGE
            + m["table_rows"] * TABLE_ROW_PAGE)


def main(argv):
    dev = os.environ.get("TEL_DEV", "") != ""
    args = []
    for a in argv[1:]:
        if a == "--dev":
            dev = True
        elif a == "--prod":
            dev = False
        else:
            args.append(a)
    src_dir = args[0] if args else os.path.join(
        os.path.dirname(os.path.dirname(os.path.realpath(__file__))),
        "book", "src")

    files = summary_files(src_dir)
    if dev:
        files += tip_files(src_dir)

    m = measure(files)
    total = pages(m)
    kind = "dev (incl. TIPs)" if dev else "prod"
    print(f"~{round(total)} A4 pages  [{kind}, {len(files)} topics, "
          f"{m['prose_words']:,} prose words + {m['code_lines']:,} code lines]")


if __name__ == "__main__":
    main(sys.argv)
