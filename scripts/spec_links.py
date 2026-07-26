#!/usr/bin/env python3
"""Check (and export) the spec anchors that tie Tel's docs to its code.

A *spec anchor* is a SCREAMING_SNAKE_CASE id naming one rule of the language.
It is declared once in the documentation, in ``doc/book/src``::

    {{#spec SAME_SCOPE_REDECLARATION}}

and claimed by any number of implementation sites::

    spec!(SAME_SCOPE_REDECLARATION);
    spec!(IDENTIFIER_SHAPE, "leading character checked separately");
    // spec: SAME_SCOPE_REDECLARATION — comment form, for non-Rust files

This script verifies that both sides line up:

* every code claim points at an id the docs actually declare  (error)
* no id is declared twice in the docs                          (error)
* every id on either side is well formed                       (error)
* declared rules that no code claims yet                       (info; error under --strict)

With ``--write-links`` it also exports the code locations as JSON for the
book's ``spec-anchors`` preprocessor, which renders them as links from the
rule back to the code that implements it.

Usage::

    scripts/spec_links.py                       # check both sides
    scripts/spec_links.py --unimplemented       # also list rules without code
    scripts/spec_links.py --write-links         # refresh the book's link data
    scripts/spec_links.py --docs OTHER/src --code OTHER   # non-default layout

Exit status: 0 clean, 1 problems found, 2 bad invocation.

spec-links: ignore — this file contains the marker patterns themselves.
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

# The id shape: SCREAMING_SNAKE_CASE, 3..64 chars, no leading/trailing/double _.
ID_RE = re.compile(r'^[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)*$')
ID_MIN, ID_MAX = 3, 64

# Documentation side: {{#spec ID}}. Captured loosely so a malformed id is
# reported rather than silently ignored.
DOC_MARKER_RE = re.compile(r'\{\{#spec\s+([^\s{}]+)\s*\}\}')

# Markers shown inside code — a page explaining the convention — are not
# declarations. The book's preprocessor skips these too, so the two agree.
DOC_FENCE_RE = re.compile(r'(?ms)^(`{3,}|~{3,})[^\n]*\n.*?^\1[ \t]*$')
DOC_INLINE_CODE_RE = re.compile(r'(`+)(?!`).*?\1')

# Code side: the Rust macro, optionally namespaced (crate::spec!, tel_common::spec!).
CODE_MACRO_RE = re.compile(
    r'(?:\w+::)*\bspec!\s*\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*'
    r'(?:,\s*"((?:[^"\\]|\\.)*)"\s*)?,?\s*\)'
)
# Code side: the comment form, for files where a Rust macro cannot go.
CODE_COMMENT_RE = re.compile(
    r'(?://|/\*|\#|--)\s*spec:\s*([A-Za-z_][A-Za-z0-9_]*)'
    r'(?:\s*(?:[-—:]|—)\s*(.*?))?\s*(?:\*/)?\s*$'
)

# Any file containing this marker is skipped entirely — for files that talk
# *about* the convention instead of using it.
SKIP_MARKER = 'spec-links: ignore'

DOC_SUFFIXES = {'.md'}
CODE_SUFFIXES = {'.rs', '.lalrpop', '.tel', '.py', '.sh', '.toml'}
SKIP_DIRS = {
    '.git', '.idea', '.intellijPlatform', '__pycache__', 'node_modules',
    'target', 'out', 'build', 'dist', 'venv', '.venv',
}

DEFAULT_SOURCE_URL = 'https://github.com/mverleg/tel/blob/main/{path}#L{line}'


class Problem:
    def __init__(self, where, message):
        self.where = where
        self.message = message

    def __str__(self):
        return f'{self.where}: {self.message}'


def bad_id(an_id):
    """Return a complaint about `an_id`, or None when it is well formed."""
    if not ID_RE.match(an_id):
        return 'must be SCREAMING_SNAKE_CASE (letters, digits, single underscores)'
    if not ID_MIN <= len(an_id) <= ID_MAX:
        return f'must be {ID_MIN}..{ID_MAX} characters'
    return None


def walk_files(root, suffixes):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS and not d.startswith('.'))
        for filename in sorted(filenames):
            path = Path(dirpath) / filename
            if path.suffix in suffixes:
                yield path


def read_text(path):
    try:
        return path.read_text(encoding='utf-8')
    except (UnicodeDecodeError, OSError):
        return None


def blank_code(text):
    """Blank out code spans, keeping every line number intact."""
    def blank(a_match):
        return re.sub(r'[^\n]', ' ', a_match.group(0))

    return DOC_INLINE_CODE_RE.sub(blank, DOC_FENCE_RE.sub(blank, text))


def collect_doc_anchors(docs_root, problems):
    """id -> (relative path, line). Duplicates are reported, first wins."""
    anchors = {}
    for path in walk_files(docs_root, DOC_SUFFIXES):
        text = read_text(path)
        if text is None or SKIP_MARKER in text:
            continue
        rel = path.relative_to(docs_root).as_posix()
        for lineno, line in enumerate(blank_code(text).splitlines(), start=1):
            for an_id in DOC_MARKER_RE.findall(line):
                where = f'{rel}:{lineno}'
                complaint = bad_id(an_id)
                if complaint:
                    problems.append(Problem(where, f'malformed spec id {an_id!r}: {complaint}'))
                    continue
                if an_id in anchors:
                    first = anchors[an_id]
                    problems.append(Problem(
                        where, f'{an_id} is already declared at {first[0]}:{first[1]}'))
                    continue
                anchors[an_id] = (rel, lineno)
    return anchors


def collect_code_claims(code_root, problems):
    """id -> list of {path, line, note}, in scan order."""
    claims = {}
    for path in walk_files(code_root, CODE_SUFFIXES):
        text = read_text(path)
        if text is None or SKIP_MARKER in text:
            continue
        rel = path.relative_to(code_root).as_posix()
        for lineno, line in enumerate(text.splitlines(), start=1):
            for match in CODE_MACRO_RE.finditer(line):
                _record(claims, problems, match, rel, lineno)
            for match in CODE_COMMENT_RE.finditer(line):
                _record(claims, problems, match, rel, lineno)
    return claims


def _record(claims, problems, match, rel, lineno):
    an_id, note = match.group(1), (match.group(2) or '').strip()
    where = f'{rel}:{lineno}'
    complaint = bad_id(an_id)
    if complaint:
        problems.append(Problem(where, f'malformed spec id {an_id!r}: {complaint}'))
        return
    entry = {'path': rel, 'line': lineno}
    if note:
        entry['note'] = note
    claims.setdefault(an_id, []).append(entry)


def check(anchors, claims, problems):
    """Cross-check the two sides; returns the ids that no code claims."""
    for an_id, sites in sorted(claims.items()):
        if an_id not in anchors:
            for site in sites:
                problems.append(Problem(
                    f'{site["path"]}:{site["line"]}',
                    f'{an_id} is not declared in the docs '
                    f'(add {{{{#spec {an_id}}}}} to the topic that states the rule)'))
    return sorted(an_id for an_id in anchors if an_id not in claims)


def write_links(target, anchors, claims, source_url):
    payload = {
        'generated_by': 'scripts/spec_links.py — do not edit by hand',
        'source_url': source_url,
        'anchors': {
            an_id: claims.get(an_id, [])
            for an_id in sorted(anchors)
        },
    }
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(payload, indent=2, sort_keys=False) + '\n', encoding='utf-8')


def default_docs(code_root):
    """The language documentation lives in this same repo, under doc/."""
    return code_root / 'doc' / 'book' / 'src'


def main():
    here = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--code', type=Path, default=here,
                        help='root of the implementation (default: this repo)')
    parser.add_argument('--docs', type=Path, default=None,
                        help='book src/ holding the anchors (default: <code>/doc/book/src)')
    parser.add_argument('--write-links', nargs='?', type=Path, const=Path('-'),
                        metavar='FILE',
                        help='write the code locations as JSON for the book '
                             '(default: <docs>/../spec-links.json)')
    parser.add_argument('--source-url', default=DEFAULT_SOURCE_URL,
                        help='link template for exported code locations')
    parser.add_argument('--unimplemented', action='store_true',
                        help='list documented rules that no code claims yet')
    parser.add_argument('--strict', action='store_true',
                        help='treat unclaimed documented rules as errors')
    parser.add_argument('--json', action='store_true',
                        help='print the full anchor table as JSON instead of a report')
    args = parser.parse_args()

    code_root = args.code.resolve()
    docs_root = (args.docs or default_docs(code_root)).resolve()
    if not code_root.is_dir():
        print(f'no such code directory: {code_root}', file=sys.stderr)
        return 2
    if not docs_root.is_dir():
        print(f'no such docs directory: {docs_root}\n'
              f'point --docs at the book src/ that holds the {{{{#spec}}}} markers',
              file=sys.stderr)
        return 2

    problems = []
    anchors = collect_doc_anchors(docs_root, problems)
    claims = collect_code_claims(code_root, problems)
    unclaimed = check(anchors, claims, problems)

    if args.json:
        json.dump({
            'docs': str(docs_root), 'code': str(code_root),
            'anchors': {
                an_id: {'doc': f'{loc[0]}:{loc[1]}', 'code': claims.get(an_id, [])}
                for an_id, loc in sorted(anchors.items())
            },
            'problems': [str(p) for p in problems],
        }, sys.stdout, indent=2)
        sys.stdout.write('\n')
        return 1 if problems else 0

    if args.write_links is not None:
        target = args.write_links
        if target == Path('-'):
            target = docs_root.parent / 'spec-links.json'
        write_links(target, anchors, claims, args.source_url)
        print(f'wrote {target}')

    claimed = len(anchors) - len(unclaimed)
    print(f'spec anchors: {len(anchors)} documented, {claimed} implemented, '
          f'{sum(len(v) for v in claims.values())} code sites')

    if args.unimplemented and unclaimed:
        print('\nno implementation claims these yet:')
        for an_id in unclaimed:
            rel, lineno = anchors[an_id]
            print(f'  {an_id:<40} {rel}:{lineno}')

    if args.strict and unclaimed:
        for an_id in unclaimed:
            rel, lineno = anchors[an_id]
            problems.append(Problem(f'{rel}:{lineno}', f'{an_id} has no implementation'))

    if problems:
        print(f'\n{len(problems)} problem(s):', file=sys.stderr)
        for problem in problems:
            print(f'  {problem}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
