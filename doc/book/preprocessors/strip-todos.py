#!/usr/bin/env python3
"""mdbook preprocessor that hides TODO notes from the rendered site.

Removes the patterns the authoring workflow leaves behind:
- standalone paragraphs starting with ``TODO`` or `` `TODO ``;
- list items whose first non-marker token is ``TODO`` (plus indented
  continuations);
- backtick-wrapped ``TODO`` spans, single-line and multi-line.

Fenced code blocks are left untouched.
"""
import json
import re
import sys


CODE_FENCE_RE = re.compile(r'(?ms)^(`{3,}|~{3,})[^\n]*\n.*?^\1[ \t]*$')


def _stash_code(text):
    blocks = []

    def stash(a_match):
        blocks.append(a_match.group(0))
        return f'\x00B{len(blocks) - 1}\x00'

    return CODE_FENCE_RE.sub(stash, text), blocks


def _unstash_code(text, blocks):
    return re.sub(r'\x00B(\d+)\x00', lambda m: blocks[int(m.group(1))], text)


def _strip_inline_todos(text):
    text = re.sub(r'`TODO[^`\n]*`', '', text)
    text = re.sub(
        r'`TODO\b[\s\S]*?`(?=[ \t]*(?:\n[ \t]*\n|$))',
        '',
        text,
    )
    return text


def _strip_block_todos(text):
    parts = re.split(r'(\n[ \t]*\n)', text)
    out = []
    for chunk in parts:
        if re.fullmatch(r'\n[ \t]*\n', chunk):
            out.append(chunk)
            continue
        first_nonws = chunk.lstrip()
        if re.match(r'`?TODO[\s:(]', first_nonws):
            out.append('')
            continue
        lines = chunk.split('\n')
        kept = []
        skipping = False
        skip_indent = -1
        for line in lines:
            stripped = line.lstrip()
            indent = len(line) - len(stripped)
            if skipping:
                if not stripped:
                    skipping = False
                elif indent > skip_indent:
                    continue
                else:
                    skipping = False
            if re.match(r'[-*+]\s+`?TODO[\s:(]', stripped):
                skipping = True
                skip_indent = indent
                continue
            kept.append(line)
        out.append('\n'.join(kept))
    return ''.join(out)


def strip_todos(text):
    text, blocks = _stash_code(text)
    text = _strip_inline_todos(text)
    text = _strip_block_todos(text)
    text = re.sub(r'\n{3,}', '\n\n', text)
    text = _unstash_code(text, blocks)
    return text


def walk(items):
    for item in items:
        if isinstance(item, dict) and 'Chapter' in item:
            chapter = item['Chapter']
            chapter['content'] = strip_todos(chapter['content'])
            if chapter.get('sub_items'):
                walk(chapter['sub_items'])


def main():
    if len(sys.argv) > 1 and sys.argv[1] == 'supports':
        sys.exit(0)
    _ctx, book = json.load(sys.stdin)
    walk(book.get('sections', []))
    json.dump(book, sys.stdout)


if __name__ == '__main__':
    main()
