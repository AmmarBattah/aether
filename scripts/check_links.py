#!/usr/bin/env python3
"""Every relative link and `#anchor` in the tracked Markdown documents resolves.

    python3 scripts/check_links.py [root]

A link to another site is not followed. Exits 1 and names each broken link.
"""
from pathlib import Path
import re
import subprocess
import sys
import unicodedata

FENCE = re.compile(r"^\s*(```|~~~)")
INLINE_CODE = re.compile(r"`[^`\n]*`")
HEADING = re.compile(r"^ {0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
INLINE_LINK = re.compile(r"\[(?:[^\]\[]|\[[^\]]*\])*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
REFERENCE = re.compile(r"^ {0,3}\[[^\]]+\]:\s*<?(\S+?)>?(?:\s|$)")
HTML_TARGET = re.compile(r"""<(?:a|img)\b[^>]*?\b(?:href|src)\s*=\s*["']([^"']+)["']""", re.I)
HTML_ANCHOR = re.compile(r"""<[a-z][^>]*?\b(?:id|name)\s*=\s*["']([^"']+)["']""", re.I)
OTHER_SITE = re.compile(r"^(?:[a-z][a-z0-9+.-]*:|//)", re.I)
# Fixtures are inputs to tests, not documents anyone follows.
SKIPPED = ("evals/fixtures/",)


def prose_lines(text):
    """The lines outside fenced code, each with its 1-based number."""
    fenced = False
    for number, line in enumerate(text.split("\n"), start=1):
        if FENCE.match(line):
            fenced = not fenced
            continue
        if not fenced:
            yield number, line


def slug(heading):
    """The anchor GitHub gives a heading, before duplicates are numbered."""
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", heading)
    text = re.sub(r"<[^>]+>", "", text).replace("`", "").strip().lower()
    kept = [c for c in text if c in " -_" or unicodedata.category(c)[0] in "LNM"]
    return "".join(kept).replace(" ", "-")


def anchors(text):
    """Every anchor a document offers: its headings, and explicit ids and names."""
    found, seen = set(), {}
    for _, line in prose_lines(text):
        found.update(HTML_ANCHOR.findall(line))
        match = HEADING.match(line)
        if not match:
            continue
        base = slug(match.group(2))
        count = seen.get(base, 0)
        seen[base] = count + 1
        found.add(base if count == 0 else f"{base}-{count}")
    return found


def targets(text):
    """Each link target in the prose with its line; code spans are not links."""
    for number, line in prose_lines(text):
        bare = INLINE_CODE.sub("", line)
        for pattern in (INLINE_LINK, REFERENCE, HTML_TARGET):
            for target in pattern.findall(bare):
                yield number, target


def broken(root, document, known_anchors):
    """The broken relative links of one document, as `path:line: reason`."""
    assert document.is_file(), f"{document} is not a file"
    problems = []
    for number, target in targets(document.read_text(encoding="utf-8")):
        if OTHER_SITE.match(target):
            continue
        path_part, _, anchor = target.partition("#")
        path_part = path_part.split("?", 1)[0]
        linked = (document.parent / path_part).resolve() if path_part else document
        where = f"{document.relative_to(root)}:{number}"
        if root not in (linked, *linked.parents):
            problems.append(f"{where}: {target} leaves the repository")
        elif not linked.exists():
            problems.append(f"{where}: {target} does not exist")
        elif anchor and linked.suffix == ".md" and anchor.lower() not in known_anchors(linked):
            problems.append(f"{where}: {target} names no heading of {linked.relative_to(root)}")
    return problems


def documents(root):
    listing = subprocess.run(["git", "-C", str(root), "ls-files", "-z", "--", "*.md"],
                             capture_output=True, text=True, check=True).stdout
    names = [name for name in listing.split("\0") if name and not name.startswith(SKIPPED)]
    assert names, f"git lists no Markdown document under {root}"
    return [root / name for name in names]


def check(root):
    cache = {}

    def known_anchors(path):
        if path not in cache:
            cache[path] = anchors(path.read_text(encoding="utf-8"))
        return cache[path]

    checked = documents(root)
    problems = [problem for document in checked for problem in broken(root, document, known_anchors)]
    return len(checked), problems


def main():
    root = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).parent.parent).resolve()
    if len(sys.argv) > 2 or not root.is_dir():
        sys.exit("usage: check_links.py [root]")
    count, problems = check(root)
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        sys.exit(f"{len(problems)} broken link(s) in {count} documents")
    print(f"links and anchors resolve in {count} documents")


if __name__ == "__main__":
    main()
