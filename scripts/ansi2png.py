#!/usr/bin/env python3
"""Render a tmux ANSI capture (capture-pane -e) to a PNG screenshot.

Usage: python3 scripts/ansi2png.py IN.ansi OUT.png [--font PATH] [--size 14]
                                   [--bg RRGGBB] [--fg RRGGBB]
Handles SGR reset, bold, dim, italic, underline, 16/256/24-bit colours.

The page ground defaults to whatever the capture itself paints most (see
`page_ground`), so a light theme does not come out on a dark page; --bg/--fg
force it.
"""

import argparse
import re
import sys

from PIL import Image, ImageDraw, ImageFont

BASE16 = [
    (30, 30, 36), (224, 108, 117), (152, 195, 121), (229, 192, 123),
    (97, 175, 239), (198, 120, 221), (86, 182, 194), (204, 204, 204),
    (90, 90, 100), (240, 130, 140), (170, 210, 140), (240, 210, 140),
    (120, 190, 250), (215, 140, 235), (110, 200, 210), (255, 255, 255),
]
DEFAULT_FG = (204, 204, 204)
DEFAULT_BG = (24, 24, 30)


def xterm256(n):
    if n < 16:
        return BASE16[n]
    if n < 232:
        n -= 16
        r, g, b = n // 36, (n // 6) % 6, n % 6
        conv = lambda v: 0 if v == 0 else 55 + v * 40
        return (conv(r), conv(g), conv(b))
    v = 8 + (n - 232) * 10
    return (v, v, v)


class Style:
    def __init__(self):
        self.reset()

    def reset(self):
        self.fg = None
        self.bg = None
        self.bold = False
        self.dim = False
        self.italic = False
        self.underline = False
        self.reverse = False

    def copy(self):
        s = Style()
        s.__dict__.update(self.__dict__)
        return s

    def apply(self, params):
        i = 0
        while i < len(params):
            p = params[i]
            if p == 0:
                self.reset()
            elif p == 1:
                self.bold = True
            elif p == 2:
                self.dim = True
            elif p == 3:
                self.italic = True
            elif p == 4:
                self.underline = True
            elif p == 7:
                self.reverse = True
            elif p == 22:
                self.bold = self.dim = False
            elif p == 23:
                self.italic = False
            elif p == 24:
                self.underline = False
            elif p == 27:
                self.reverse = False
            elif 30 <= p <= 37:
                self.fg = BASE16[p - 30]
            elif p == 39:
                self.fg = None
            elif 40 <= p <= 47:
                self.bg = BASE16[p - 40]
            elif p == 49:
                self.bg = None
            elif 90 <= p <= 97:
                self.fg = BASE16[p - 90 + 8]
            elif 100 <= p <= 107:
                self.bg = BASE16[p - 100 + 8]
            elif p in (38, 48):
                target = "fg" if p == 38 else "bg"
                if i + 1 < len(params) and params[i + 1] == 5 and i + 2 < len(params):
                    setattr(self, target, xterm256(params[i + 2]))
                    i += 2
                elif i + 1 < len(params) and params[i + 1] == 2 and i + 4 < len(params):
                    setattr(self, target, tuple(params[i + 2 : i + 5]))
                    i += 4
            i += 1


SGR = re.compile(r"\x1b\[([0-9;]*)m")
OTHER = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")


def parse(text):
    """Yield rows of (char, style) cells.

    Style carries across lines, the way a terminal's does: tmux only states
    what changed, so a line whose first run names a foreground alone is still
    on the background the line above ended with.
    """
    rows = []
    style = Style()
    for line in text.split("\n"):
        cells = []
        pos = 0
        for m in SGR.finditer(line):
            for ch in OTHER.sub("", line[pos : m.start()]):
                cells.append((ch, style.copy()))
            params = [int(x) if x else 0 for x in m.group(1).split(";")] if m.group(1) else [0]
            style.apply(params)
            pos = m.end()
        for ch in OTHER.sub("", line[pos:]):
            cells.append((ch, style.copy()))
        rows.append(cells)
    while rows and not rows[-1]:
        rows.pop()
    return rows


def page_ground(rows, default_fg, default_bg):
    """The colours an unstyled cell is drawn on.

    A themed TUI paints its own ground on every cell, but tmux records the
    dominant one as "the default" and never states it, so taking the built-in
    dark here put a light theme on a dark page. The ground is whatever the
    capture actually covers the most columns with; a capture that names no
    background at all (the mono theme) keeps the built-in default.
    """
    seen = {}
    for row in rows:
        for _, st in row:
            if st.bg is not None:
                seen[(st.fg, st.bg)] = seen.get((st.fg, st.bg), 0) + 1
    if not seen:
        return default_fg, default_bg
    fg, bg = max(seen, key=seen.get)
    return fg or default_fg, bg


def render(rows, font_path, size, out, cols=None, ground=None):
    font = ImageFont.truetype(font_path, size)
    bold_path = font_path.replace("Regular", "Bold")
    try:
        bold = ImageFont.truetype(bold_path, size)
    except OSError:
        bold = font
    bbox = font.getbbox("M")
    cw = bbox[2] - bbox[0]
    ch = int(size * 1.35)
    width = cols or max((len(r) for r in rows), default=80)
    default_fg, default_bg = ground or (DEFAULT_FG, DEFAULT_BG)
    pad = 16
    img = Image.new("RGB", (width * cw + pad * 2, len(rows) * ch + pad * 2), default_bg)
    draw = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        for x, (c, st) in enumerate(row):
            if x >= width:
                break
            fg = st.fg or default_fg
            bg = st.bg or default_bg
            if st.reverse:
                fg, bg = bg, fg
            if st.dim:
                # Toward the cell's own ground, not toward black: on a light
                # page darkening moves ink *away* from the background and a
                # dimmed cell comes out more readable than the terminal's.
                fg = tuple(int(f + (b - f) * 0.4) for f, b in zip(fg, bg))
            x0 = pad + x * cw
            y0 = pad + y * ch
            if bg != default_bg:
                draw.rectangle([x0, y0, x0 + cw, y0 + ch], fill=bg)
            if c != " ":
                draw.text((x0, y0), c, fill=fg, font=bold if st.bold else font)
            if st.underline:
                draw.line([x0, y0 + ch - 2, x0 + cw, y0 + ch - 2], fill=fg)
    img.save(out)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--font", default="/usr/share/fonts/TTF/JetBrainsMonoNerdFontMono-Regular.ttf")
    ap.add_argument("--size", type=int, default=14)
    ap.add_argument("--cols", type=int, default=None)
    ap.add_argument("--bg", default=None, help="page background as RRGGBB")
    ap.add_argument("--fg", default=None, help="page foreground as RRGGBB")
    a = ap.parse_args()
    with open(a.input, encoding="utf-8", errors="replace") as f:
        rows = parse(f.read())
    hexed = lambda v: tuple(int(v[i : i + 2], 16) for i in (0, 2, 4))
    fg, bg = page_ground(rows, DEFAULT_FG, DEFAULT_BG)
    if a.bg:
        bg = hexed(a.bg.lstrip("#"))
    if a.fg:
        fg = hexed(a.fg.lstrip("#"))
    render(rows, a.font, a.size, a.output, a.cols, (fg, bg))
    print(a.output, file=sys.stderr)
