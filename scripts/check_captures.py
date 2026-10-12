#!/usr/bin/env python3
"""Hold the frames scripts/tui_capture.sh produced to the TUI design (SPEC §8).

The Rust tests in crates/aether-tui/tests/ check what a TestBackend can see.
These checks need the real terminal: what tmux actually painted, after the
real daemon, the real provider and real timing.

Every check here is one a TestBackend cannot make: that the bytes tmux wrote
decode, that no escape reached the pane as content, that no row ran past the
terminal's width, that the screen actually changed between snapshots, that
two themes do not produce the same screen, and that each driven surface
(approval, queue, board, quit) really showed what its frame is named for.

Not checked here: that the frame resets what it sets. `capture-pane -e`
reconstructs the pane and ends on whatever attribute the last cell carries,
so a trailing reset would be a fact about tmux, not about the TUI.

Usage: python3 scripts/check_captures.py DIR [--theme auto|dark|light|mono]
                                             [--lane full|smoke|mode]
                                             [--width N] [--differs-from DIR]
                                             [--model ID] [--agent NAME]
                                             [--check-board]
"""

import argparse
import pathlib
import re
import sys
import unicodedata

SGR = re.compile(r"\x1b\[([0-9;:]*)m")
# What a decode error leaves behind. A glyph the *font* could not resolve is
# indistinguishable from the codepoint in a capture, so it is not checkable
# here; invalid bytes are.
REPLACEMENT = "�"
# Modifiers and their resets: the whole of mono's vocabulary. Anything else
# in a mono frame is colour.
MONO_CODES = {"0", "1", "2", "3", "4", "7", "22", "23", "24", "27", "39", "49"}
THEMES = {"auto": "auto", "dark": "dark", "light": "light", "mono": "mono", "dusk": "dark", "dawn": "light"}
# Frames with no status line by design: the quit frame is the shell after
# exit, and the artifact pager is the one fullscreen view (SPEC §6.1).
QUIT_FRAME = "15-quit"
NO_STATUS_LINE = {QUIT_FRAME, "24-artifact"}
# Below this the board is a full-width overlay that hides the status line.
BOARD_DOCK_COLUMNS = 100
BOARD_FRAME = "03-agents"
BOARD_HEADER = re.compile(r"Agents\s+\d")
SEPARATORS = "│|"
# The quiet gutter glyphs in front of a cell's continuation rows.
GUTTER = set("└│| ")
# Plain ANSI red and bright red, as SGR or as 256-colour indexes.
RED = {"31", "91", "5;1", "5;9"}

# Keep these in step with the paths in tui_capture.sh. A lane must prove
# every driven surface, rather than validate whichever files survived a run.
SMOKE_FRAMES = {
    "01-startup", "02-hello", "02d-draft", "03-agents", "05-shortcuts", QUIT_FRAME,
}
FULL_FRAMES = {
    "01-startup", "02-hello", "02i-composer-printables", "02e-shift-enter",
    "02c-wrapped-draft", "02d-narrow-draft", "02a-markdown", "27-queued",
    "02b-interrupted", "03-tool-call", "03a-failed-tool", "04-delegate",
    "22-details", "23-artifacts", "24-artifact", "05-shortcuts",
    "25-history-search", "21-palette", "05a-doctor", "09b-report", "09a-find",
    "10-models", "11a-usage", "11b-schedules", "11c-schedule-paused",
    "11d-schedule-resumed", "11e-schedule-confirm", "11f-schedule-cancelled",
    "12-settings", "13-effort", "03-agents", "03b-child", "12-board-hidden",
    "26-approval", "26a-child-approval", QUIT_FRAME,
}
MODE_FRAMES = {"01-startup", "02-hello", "03-mode-filtered", "04-mode-details", "05-mode-picker", QUIT_FRAME}
LANE_FRAMES = {"full": FULL_FRAMES, "smoke": SMOKE_FRAMES, "mode": MODE_FRAMES}


def manifest_problems(root, lane):
    """Require the lane's complete evidence and pairs for any extra frames."""
    plain = {path.stem for path in root.glob("*.txt")}
    ansi = {path.stem for path in root.glob("*.ansi")}
    expected = LANE_FRAMES[lane]
    out = []
    for name in sorted(expected | plain | ansi):
        for suffix, present in (("txt", plain), ("ansi", ansi)):
            if name not in present:
                out.append(f"{name}.{suffix} missing ({lane} lane requires paired text and ANSI captures)")
    return out


class Cell:
    __slots__ = ("ch", "fg", "bg", "bold")

    def __init__(self, ch, fg, bg, bold):
        self.ch, self.fg, self.bg, self.bold = ch, fg, bg, bold


def apply_sgr(params, state):
    """Fold one SGR's parameters into `state` (fg, bg, bold). Extended colours
    keep their whole spec ("5;n" or "2;r;g;b") so their numbers are never
    mistaken for codes of their own."""
    parts = []
    for param in params.split(";"):
        parts.extend(param.split(":") if ":" in param else [param])
    i = 0
    while i < len(parts):
        code = parts[i] or "0"
        i += 1
        if code in ("38", "48", "58"):
            kind = parts[i] if i < len(parts) else ""
            take = 2 if kind == "5" else 4 if kind == "2" else 1
            spec = ";".join(p for p in parts[i : i + take] if p)
            i += take
            if code != "58":
                state["fg" if code == "38" else "bg"] = spec
            continue
        n = int(code) if code.isdigit() else -1
        if n == 0:
            state.update(fg=None, bg=None, bold=False)
        elif n == 1:
            state["bold"] = True
        elif n == 22:
            state["bold"] = False
        elif 30 <= n <= 37 or 90 <= n <= 97:
            state["fg"] = code
        elif n == 39:
            state["fg"] = None
        elif 40 <= n <= 47 or 100 <= n <= 107:
            state["bg"] = code
        elif n == 49:
            state["bg"] = None


def parse_screen(text):
    """Rows of cells as the terminal shows them; a wide glyph takes two cells,
    so a cell's index is its column. Attributes carry across rows, as tmux
    writes them."""
    state = {"fg": None, "bg": None, "bold": False}
    rows = [[]]

    def paint(chars):
        for ch in chars:
            if ch == "\n":
                rows.append([])
                continue
            cell = Cell(ch, state["fg"], state["bg"], state["bold"])
            rows[-1].append(cell)
            if unicodedata.east_asian_width(ch) in ("W", "F"):
                rows[-1].append(Cell("", cell.fg, cell.bg, cell.bold))

    at = 0
    for m in SGR.finditer(text):
        paint(text[at : m.start()])
        apply_sgr(m.group(1), state)
        at = m.end()
    paint(text[at:])
    return rows


def row_text(row):
    return "".join(cell.ch for cell in row)


def separator_column(rows):
    """The column of the docked board's `│`, if one runs down the screen."""
    rows = [row for row in rows if row_text(row).strip()]
    if not rows:
        return None
    counts = {}
    for row in rows:
        for col, cell in enumerate(row):
            if cell.ch in SEPARATORS:
                counts[col] = counts.get(col, 0) + 1
    col, hits = max(counts.items(), key=lambda kv: kv[1], default=(None, 0))
    return col if hits >= 0.9 * len(rows) else None


def is_red(fg):
    if fg is None:
        return False
    if fg in RED:
        return True
    parts = fg.split(";")
    if parts[0] == "2" and len(parts) == 4 and all(p.isdigit() for p in parts[1:]):
        r, g, b = (int(p) for p in parts[1:])
        return r >= 128 and r > 2 * g and r > 2 * b
    return False


def text_problems(path, width):
    """Checks over the plain capture: it decoded, it painted, it fit."""
    raw = path.read_bytes()
    out = []
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as e:
        return [f"invalid UTF-8 at byte {e.start}"]
    if REPLACEMENT in text:
        out.append("a replacement character reached the screen")
    if not text.strip():
        out.append("nothing was painted")
        return out
    # The plain capture is what the pane holds, with tmux's escapes stripped.
    # An escape here is one that reached the screen as content.
    if "\x1b" in text:
        out.append("an escape sequence reached the screen as text")
    # A row can be short (plain captures trim blanks), but never long.
    over = [i + 1 for i, row in enumerate(parse_screen(text)) if len(row) > width]
    if over:
        out.append(f"rows past {width} columns: {over[:5]}")
    return out


def status_line_problems(path, width, model, agent):
    """Every frame with a bottom pane ends in `model effort · cwd · branch ·
    agent` (SPEC §1.1); the agent may read `child ‹ parent`."""
    name = path.stem
    if name in NO_STATUS_LINE or (name == BOARD_FRAME and width < BOARD_DOCK_COLUMNS):
        return []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        left = re.split(f"[{SEPARATORS}]", line, maxsplit=1)[0].strip()
        if left.startswith(model) and "·" in left and agent in left.rsplit("·", 1)[1]:
            return []
    return [f"no status line naming the model {model!r} and the viewed agent {agent!r}"]


def colour_problems(path, theme):
    """Require ANSI content; mono paints no colour, other TUI themes do.

    The shell's quit capture has no TUI colour requirement (theme=None).
    """
    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError as e:
        return [f"invalid UTF-8 at byte {e.start}"]
    out = []
    if not SGR.sub("", text).strip():
        out.append("nothing was painted in the ANSI capture")
    if theme is None:
        return out
    runs = SGR.findall(text)
    codes = [c for run in runs for c in re.split("[;:]", run) if c]
    if theme == "mono":
        coloured = sorted({c for c in codes if c not in MONO_CODES})
        if coloured:
            out.append(f"mono painted colour: {coloured[:6]}")
    elif not codes:
        out.append(f"{theme} painted nothing at all")
    return out


def background_problems(root):
    """Only the tint ever carries a background (SPEC §5.2), so a session
    paints at most one background colour across all its frames."""
    seen = {}
    for ansi in sorted(root.glob("*.ansi")):
        for row in parse_screen(ansi.read_text(encoding="utf-8", errors="replace")):
            for cell in row:
                if cell.bg is not None:
                    seen.setdefault(cell.bg, ansi.name)
    if len(seen) > 1:
        where = ", ".join(f"{bg} in {name}" for bg, name in sorted(seen.items()))
        return [f"{len(seen)} background colours, but only the tint may paint one: {where}"]
    return []


def failed_tool_problems(ansi, theme):
    """03a: the failed command's `• Ran` bullet and its last output row carry
    the failure style: red, or bold in mono, which has no colour."""
    failure = (lambda c: c.bold) if theme == "mono" else (lambda c: is_red(c.fg))
    style = "bold" if theme == "mono" else "red"
    rows = parse_screen(ansi.read_text(encoding="utf-8", errors="replace"))
    col = separator_column(rows)
    rows = [row[:col] if col is not None else row for row in rows]
    starts = [i for i, row in enumerate(rows) if row_text(row).lstrip().startswith("• Ran")]
    if not starts:
        return ["no `• Ran` cell"]
    start = starts[-1]
    bullet = next(cell for cell in rows[start] if cell.ch == "•")
    out = [] if failure(bullet) else [f"the `• Ran` bullet is not {style}"]
    end = start
    while end + 1 < len(rows) and row_text(rows[end + 1]).strip():
        end += 1
    if end == start:
        return out + ["the failed `• Ran` cell has no output rows"]
    text = [cell for cell in rows[end] if cell.ch and cell.ch not in GUTTER]
    if not all(failure(cell) for cell in text):
        out.append(f"the failed command's last row is not {style}: {row_text(rows[end]).strip()!r}")
    return out


SCHEDULE_STATES = {
    "11b-schedules": "active",
    "11c-schedule-paused": "paused",
    "11d-schedule-resumed": "active",
    "11e-schedule-confirm": "c again to cancel",
    "11f-schedule-cancelled": "cancelled",
}


def frame_problems(frame, theme):
    """What each named frame was driven to show."""
    name = frame.stem
    content = frame.read_text(encoding="utf-8", errors="replace")
    out = []

    def require(*needles):
        out.extend(f"missing {n!r}" for n in needles if n not in content)

    if name == "12-settings":
        require("View mode standard")
    if name == "03-mode-filtered":
        require("brief")
        if "Ran ls" in content or "$ ls" in content:
            out.append("filtered mode leaked tool details")
    if name == "04-mode-details":
        require("Ran ls")
    if name == "05-mode-picker":
        require("View modes", "brief", "built-in")
    if name == "02a-markdown":
        # Only the sample's blockquote may carry the bar (SPEC §3.1), never a message.
        bars = [l for l in content.splitlines() if "▎" in l and "Note: nothing" not in l]
        if bars:
            out.append("markdown painted a `▎` bar outside the blockquote")
    if name in ("02c-wrapped-draft", "02d-narrow-draft"):
        # The composer keeps its cursor, at the draft's end, in view.
        require("DRAFT END")
    if name == "02e-shift-enter":
        lines = content.splitlines()
        first = [i for i, line in enumerate(lines) if "First draft line" in line]
        second = [i for i, line in enumerate(lines) if "Second draft line" in line]
        if len(first) != 1 or len(second) != 1 or second[0] != first[0] + 1:
            out.append("Shift+Enter did not retain adjacent draft lines")
    if name == "03a-failed-tool":
        ansi = frame.with_suffix(".ansi")
        if ansi.exists():
            out.extend(failed_tool_problems(ansi, theme))
        else:
            out.append(f"{ansi.name} missing; cannot check the failure style")
    if name == "04-delegate":
        require("Spawned", "finished")
    if name == "11a-usage" and not re.search(r"^\s*Usage\b", content, re.MULTILINE):
        out.append("/usage did not open its view")
    if name in SCHEDULE_STATES:
        for expected in ("Documentation timer", "History", SCHEDULE_STATES[name]):
            if expected not in content:
                out.append(f"schedule view missing {expected!r}")
    if name == QUIT_FRAME:
        # Whitespace-blind: a narrow pane wraps the message mid-word.
        flat = "".join(content.split())
        for needle in ("Agents keep running in the daemon", "Reattach with: aether"):
            if "".join(needle.split()) not in flat:
                out.append(f"missing {needle!r}")
    if name.startswith("26"):
        require("(y)", "(esc)")
    if name == "27-queued":
        require("queued for after this turn")
    return out


def board_problems(root, width):
    """SPEC §1.2: below 100 columns the board is a full-width overlay with no
    transcript beside it; from 100 it docks behind a `│` column. The hint row
    always names ctrl+g."""
    out = []
    startup = root / "01-startup.txt"
    if startup.exists():
        lines = [line for line in startup.read_text(encoding="utf-8").splitlines() if line.strip()]
        if not lines or "ctrl+g agents" not in lines[-1]:
            out.append(f"01-startup's hint row does not name ctrl+g agents: {lines[-1] if lines else ''!r}")
    else:
        out.append("01-startup.txt missing; cannot check the hint row")
    board = root / f"{BOARD_FRAME}.txt"
    if not board.exists():
        return out + [f"{board.name} missing; cannot check the board"]
    rows = parse_screen(board.read_text(encoding="utf-8"))
    header = next((row for row in rows if BOARD_HEADER.search(row_text(row))), None)
    if header is None:
        return out + [f"{board.name} shows no `Agents` board header"]
    if width < BOARD_DOCK_COLUMNS:
        beside = BOARD_HEADER.split(row_text(header), maxsplit=1)[0].strip()
        if beside:
            out.append(f"width {width}: the overlay board has text beside it: {beside!r}")
        return out
    col = separator_column(rows)
    if col is None:
        out.append(f"width {width}: the docked board has no `│` separator column")
    elif not BOARD_HEADER.search(row_text(header[col + 1 :])):
        out.append(f"width {width}: `Agents` is not right of the separator column {col}")
    return out


def read_frames(root):
    """The coloured captures: two themes paint the same text, not the same
    screen, so only the .ansi files can tell them apart."""
    return {f.name: f.read_bytes() for f in sorted(root.glob("*.ansi"))}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--lane", default="full", choices=sorted(LANE_FRAMES))
    ap.add_argument("--theme", default="auto", choices=sorted(THEMES))
    ap.add_argument("--width", type=int, default=160)
    ap.add_argument(
        "--differs-from",
        default=None,
        help="another theme's capture directory; the two must not be identical",
    )
    ap.add_argument("--model", default="mock-1", help="the model the status line must name")
    ap.add_argument("--agent", default="assistant", help="the agent the status line must name")
    ap.add_argument(
        "--check-board",
        action="store_true",
        help="hold 03-agents to the board's overlay/docked layout at --width",
    )
    args = ap.parse_args()
    theme = THEMES[args.theme]

    root = pathlib.Path(args.dir)
    frames = sorted(root.glob("*.txt"))
    if not frames:
        print(f"no frames in {root}", file=sys.stderr)
        return 1

    failed = False
    def fail(name, what):
        nonlocal failed
        failed = True
        print(f"{name}: {what}", file=sys.stderr)

    for what in manifest_problems(root, args.lane):
        fail(root.name, what)

    for frame in frames:
        for what in frame_problems(frame, theme):
            fail(frame.name, what)
        for what in text_problems(frame, args.width):
            fail(frame.name, what)
        for what in status_line_problems(frame, args.width, args.model, args.agent):
            fail(frame.name, what)
        ansi = frame.with_suffix(".ansi")
        # The quit frame is the shell after exit: no colour of the TUI's own.
        if ansi.exists():
            for what in colour_problems(ansi, None if frame.stem == QUIT_FRAME else theme):
                fail(ansi.name, what)

    if theme != "mono":
        for what in background_problems(root):
            fail(root.name, what)

    # The capture script drives the app through a session. If every frame is
    # the same, it captured a stuck screen, not a session.
    distinct = {f.read_bytes() for f in frames}
    if len(frames) > 1 and len(distinct) == 1:
        fail(root.name, "every frame is identical - the screen never changed")

    if args.differs_from:
        other = pathlib.Path(args.differs_from)
        for what in manifest_problems(other, args.lane):
            fail(str(other), what)
        mine, theirs = read_frames(root), read_frames(other)
        shared = sorted(set(mine) & set(theirs))
        if not shared:
            fail(root.name, f"no frames in common with {other}")
        same = [n for n in shared if mine[n] == theirs[n]]
        if len(same) == len(shared):
            fail(root.name, f"identical to {other} in all {len(shared)} frames")

    if args.check_board:
        for what in board_problems(root, args.width):
            fail(root.name, what)

    print(f"checked {len(frames)} frames in {root} ({theme})")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
