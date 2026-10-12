"""Deterministic checks for check_captures.py's frame validators.

Fixtures are built as plain files under a tempdir - no daemon, no tmux, no
real capture - so these exercise the validators' own logic in isolation,
never real terminal output. The real-terminal coverage is the capture lanes
themselves (see scripts/ci.sh); this suite guards the checker they lean on.
"""
import io
import pathlib
import re
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout

import check_captures as cc

STATUS_LINE = "  mock-1 · ~/aether · main · assistant"
HINT_ROW = "  ? shortcuts · ctrl+g agents                 100% context left"
RED, DIM, BOLD, RESET = "\x1b[38;5;1m", "\x1b[2m", "\x1b[1m", "\x1b[0m"


def write(path, text):
    path.write_text(text, encoding="utf-8")


def docked(left, right, width=60):
    """One row of a docked board: transcript, the `│` column, the board."""
    return f"{left:<{width}} │  {right}".rstrip()


class TempDirCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()


class TextProblemsTests(TempDirCase):
    def test_mode_lane_requires_all_evidence_and_rejects_tool_leaks(self):
        self.assertTrue(cc.manifest_problems(self.root, "mode"))
        frame = self.root / "03-mode-filtered.txt"
        write(frame, "assistant · brief\nRan ls\n")
        self.assertIn("filtered mode leaked tool details", cc.frame_problems(frame, "dark"))
        write(frame, "assistant · brief\nTools retained in details\n")
        self.assertEqual(cc.frame_problems(frame, "dark"), [])

    def test_settings_requires_a_separator_before_view_mode_value(self):
        frame = self.root / "12-settings.txt"
        write(frame, "6. View modestandard\n")
        self.assertTrue(cc.frame_problems(frame, "dark"))
        write(frame, "6. View mode standard\n")
        self.assertEqual(cc.frame_problems(frame, "dark"), [])

    def test_clean_frame_has_no_problems(self):
        frame = self.root / "01-startup.txt"
        write(frame, f"hello\n{STATUS_LINE}\n")
        self.assertEqual(cc.text_problems(frame, width=160), [])

    def test_empty_frame_fails(self):
        frame = self.root / "01-startup.txt"
        write(frame, "   \n  \n")
        problems = cc.text_problems(frame, width=160)
        self.assertIn("nothing was painted", problems)

    def test_leaked_escape_fails(self):
        frame = self.root / "01-startup.txt"
        write(frame, "aether\n\x1b[31mleaked\n")
        problems = cc.text_problems(frame, width=160)
        self.assertTrue(any("escape sequence" in p for p in problems))

    def test_overwide_row_fails(self):
        frame = self.root / "01-startup.txt"
        write(frame, "aether\n" + "x" * 200 + "\n")
        problems = cc.text_problems(frame, width=160)
        self.assertTrue(any("rows past 160 columns" in p for p in problems))

    def test_wide_glyphs_count_two_columns(self):
        frame = self.root / "01-startup.txt"
        write(frame, "界" * 6 + "\n")
        self.assertEqual(cc.text_problems(frame, width=12), [])
        self.assertTrue(cc.text_problems(frame, width=11))

    def test_invalid_utf8_fails(self):
        frame = self.root / "01-startup.txt"
        frame.write_bytes(b"aether\n\xff\xfe not valid utf-8\n")
        problems = cc.text_problems(frame, width=160)
        self.assertTrue(any("invalid UTF-8" in p for p in problems))


class StatusLineTests(TempDirCase):
    def check(self, name, text, width=160):
        frame = self.root / f"{name}.txt"
        write(frame, text)
        return cc.status_line_problems(frame, width, "mock-1", "assistant")

    def test_status_line_passes(self):
        self.assertEqual(self.check("02-hello", f"› hello\n{STATUS_LINE}\n{HINT_ROW}\n"), [])

    def test_status_line_beside_a_docked_board_passes(self):
        text = docked(STATUS_LINE, "Workspace  …/aether") + "\n"
        self.assertEqual(self.check("02-hello", text), [])

    def test_child_breadcrumb_names_the_viewed_agent(self):
        text = "  mock-1 low · ~/aether · worker ‹ assistant\n"
        self.assertEqual(self.check("03b-child", text), [])

    def test_missing_status_line_fails(self):
        problems = self.check("02-hello", f"› hello\n{HINT_ROW}\n")
        self.assertTrue(any("no status line" in p for p in problems))

    def test_model_alone_is_not_enough(self):
        problems = self.check("02-hello", "  mock-1 · ~/aether · main\n")
        self.assertTrue(problems)

    def test_agent_must_be_the_last_item(self):
        problems = self.check("02-hello", "  mock-1 · assistant · main\n")
        self.assertTrue(problems)

    def test_frames_without_a_bottom_pane_are_exempt(self):
        self.assertEqual(self.check("15-quit", "Agents keep running in the daemon\n"), [])
        self.assertEqual(self.check("24-artifact", "aether artifact line\n"), [])

    def test_only_the_narrow_board_overlay_is_exempt(self):
        overlay = "  Agents     1 · —\n  ◌ assistant\n"
        self.assertEqual(self.check("03-agents", overlay, width=99), [])
        self.assertTrue(self.check("03-agents", overlay, width=100))


class ColourProblemsTests(TempDirCase):
    def test_mono_rejects_colour_codes(self):
        ansi = self.root / "01-startup.ansi"
        write(ansi, "\x1b[38;5;196mred text\x1b[0m")
        problems = cc.colour_problems(ansi, "mono")
        self.assertTrue(any("mono painted colour" in p for p in problems))

    def test_mono_accepts_weight_only_codes(self):
        ansi = self.root / "01-startup.ansi"
        write(ansi, "\x1b[1mbold\x1b[22m\x1b[2mdim\x1b[0m")
        self.assertEqual(cc.colour_problems(ansi, "mono"), [])

    def test_coloured_theme_requires_some_codes(self):
        ansi = self.root / "01-startup.ansi"
        write(ansi, "plain text, no escapes at all")
        problems = cc.colour_problems(ansi, "dark")
        self.assertTrue(any("painted nothing at all" in p for p in problems))

    def test_empty_ansi_fails_even_without_a_colour_requirement(self):
        ansi = self.root / "15-quit.ansi"
        write(ansi, RESET + "   \n")
        for theme in ("mono", "dark", None):
            with self.subTest(theme=theme):
                self.assertIn("nothing was painted in the ANSI capture", cc.colour_problems(ansi, theme))

    def test_invalid_ansi_utf8_fails(self):
        ansi = self.root / "01-startup.ansi"
        ansi.write_bytes(b"\x1b[31m\xff")
        self.assertIn("invalid UTF-8 at byte 5", cc.colour_problems(ansi, "dark"))


class BackgroundProblemsTests(TempDirCase):
    def test_one_tint_across_frames_passes(self):
        tint = "\x1b[48;2;30;34;39m"
        write(self.root / "01-startup.ansi", f"{tint}› Message assistant\x1b[49m\n")
        write(self.root / "02-hello.ansi", f"{tint}› hello\x1b[0m\n")
        self.assertEqual(cc.background_problems(self.root), [])

    def test_a_second_background_fails(self):
        write(self.root / "01-startup.ansi", "\x1b[48;2;30;34;39m› Message\x1b[0m\n")
        write(self.root / "02-hello.ansi", "\x1b[41mbanner\x1b[0m\n")
        problems = cc.background_problems(self.root)
        self.assertTrue(any("2 background colours" in p and "02-hello.ansi" in p for p in problems))

    def test_truecolour_foreground_numbers_are_not_backgrounds(self):
        # 40/45/100 here are channels of a foreground colour, not SGR codes.
        write(self.root / "01-startup.ansi", "\x1b[38;2;40;45;100mtext\x1b[0m \x1b[38:2::41:46:101mmore\n")
        self.assertEqual(cc.background_problems(self.root), [])

    def test_unpainted_reset_is_not_a_background(self):
        write(self.root / "01-startup.ansi", "\x1b[44m\x1b[49mtext\n")
        self.assertEqual(cc.background_problems(self.root), [])


def failed_cell(bullet=RED, last=RED, board=""):
    """A 03a frame: a failed `• Ran` cell, optionally beside a docked board."""
    rows = [
        f"{DIM}•{RESET} Running a command that fails.",
        "",
        f"{BOLD}{bullet}•{RESET} {BOLD}Ran{RESET} cat missing.txt {DIM}(exit 1){RESET}",
        f"{DIM}  └ {RESET}{last}cat: missing.txt: No such file or directory{RESET}",
        "",
        f"{DIM}─── Worked for 0s ───{RESET}",
    ]
    if board:
        pad = lambda row: " " * (50 - len(cc.SGR.sub("", row)))
        rows = [f"{row}{pad(row)}{DIM}│{RESET}  {board}" for row in rows]
    return "\n".join(rows) + "\n"


class FailedToolTests(TempDirCase):
    def check(self, text, theme="dark"):
        ansi = self.root / "03a-failed-tool.ansi"
        write(ansi, text)
        return cc.failed_tool_problems(ansi, theme)

    def test_red_bullet_and_red_last_row_pass(self):
        self.assertEqual(self.check(failed_cell()), [])

    def test_neutral_bullet_fails(self):
        problems = self.check(failed_cell(bullet=DIM))
        self.assertTrue(any("bullet is not red" in p for p in problems))

    def test_dim_last_row_fails(self):
        problems = self.check(failed_cell(last=DIM))
        self.assertTrue(any("last row is not red" in p for p in problems))

    def test_truecolour_red_counts(self):
        red = "\x1b[38;2;220;50;47m"
        self.assertEqual(self.check(failed_cell(bullet=red, last=red)), [])

    def test_board_text_beside_the_cell_is_ignored(self):
        # The docked board's rows are not part of the transcript cell.
        self.assertEqual(self.check(failed_cell(board="Model      mock-1")), [])

    def test_mono_uses_bold_for_failure(self):
        self.assertEqual(self.check(failed_cell(bullet="", last=BOLD), theme="mono"), [])
        problems = self.check(failed_cell(bullet="", last=""), theme="mono")
        self.assertTrue(any("not bold" in p for p in problems))

    def test_missing_cell_fails(self):
        self.assertEqual(self.check("• Hello\n"), ["no `• Ran` cell"])


class FrameProblemsTests(TempDirCase):
    def check(self, name, text):
        frame = self.root / f"{name}.txt"
        write(frame, text)
        return cc.frame_problems(frame, "dark")

    def test_markdown_without_bars_passes(self):
        self.assertEqual(self.check("02a-markdown", "  ▎ Note: nothing is written\n"), [])
        self.assertTrue(self.check("02a-markdown", "▎ hello\n"))

    def test_delegate_needs_spawned_and_finished(self):
        self.assertEqual(self.check("04-delegate", "• Spawned worker\n• worker finished\n"), [])
        problems = self.check("04-delegate", "• Spawned worker\n")
        self.assertEqual(problems, ["missing 'finished'"])

    def test_approval_frames_need_both_hotkeys(self):
        self.assertEqual(self.check("26-approval", "1. Yes (y)\n4. No (esc)\n"), [])
        self.assertEqual(self.check("26a-child-approval", "1. Yes (y)\n"), ["missing '(esc)'"])

    def test_queued_frame_needs_the_queue_line(self):
        self.assertEqual(self.check("27-queued", "↪ hi\n  queued for after this turn\n"), [])
        self.assertTrue(self.check("27-queued", "↪ hi\n  steering\n"))

    def test_usage_needs_its_view_title(self):
        self.assertEqual(self.check("11a-usage", "\n  Usage  refreshing…\n"), [])
        # The slash popup's row is not the view.
        self.assertTrue(self.check("11a-usage", "  /usage  providers, sign-in\n"))

    def test_quit_needs_the_detach_message(self):
        message = "Agents keep running in the daemon (0 running). Reattach with: aether\n"
        self.assertEqual(self.check("15-quit", message), [])
        self.assertEqual(len(self.check("15-quit", "Pane is dead\n")), 2)
        # A 59-column pane wraps the message mid-word.
        wrapped = "Agents keep running in the daemon (0 running · 0 needs you). Re\nattach with: aether\n"
        self.assertEqual(self.check("15-quit", wrapped), [])

    def test_schedule_frames_need_the_timer_history_and_state(self):
        good = "› 1. Documentation timer  paused\n\n  History\n"
        self.assertEqual(self.check("11c-schedule-paused", good), [])
        problems = self.check("11b-schedules", "› 1. Documentation timer  active\n")
        self.assertEqual(problems, ["schedule view missing 'History'"])

    def test_draft_frames_keep_the_draft_end_in_view(self):
        self.assertEqual(self.check("02d-narrow-draft", "review the findings. DRAFT END\n"), [])
        self.assertTrue(self.check("02d-narrow-draft", "  Agents  1 · —\n"))

    def test_shift_enter_needs_adjacent_lines(self):
        self.assertEqual(self.check("02e-shift-enter", "› First draft line\n  Second draft line\n"), [])
        self.assertTrue(self.check("02e-shift-enter", "› First draft line\n\n  Second draft line\n"))

    def test_failed_tool_without_ansi_fails(self):
        problems = self.check("03a-failed-tool", "• Ran cat missing.txt\n")
        self.assertTrue(any("03a-failed-tool.ansi missing" in p for p in problems))


class BoardProblemsTests(TempDirCase):
    def startup(self, hint=HINT_ROW):
        write(self.root / "01-startup.txt", f"\n{STATUS_LINE}\n{hint}\n")

    def test_narrow_overlay_passes(self):
        self.startup()
        write(self.root / "03-agents.txt", "\n  Agents          1 · —\n\n  ◌ assistant\n")
        self.assertEqual(cc.board_problems(self.root, 59), [])

    def test_narrow_board_beside_the_transcript_fails(self):
        self.startup()
        write(self.root / "03-agents.txt", "• Hello │  Agents   1 · —\n")
        problems = cc.board_problems(self.root, 80)
        self.assertTrue(any("text beside it" in p for p in problems))

    def test_docked_board_behind_a_separator_passes(self):
        self.startup()
        rows = [docked("", ""), docked("› hello", "Agents   1 · —"), docked("• Hello", "◌ assistant")]
        write(self.root / "03-agents.txt", "\n".join(rows) + "\n")
        self.assertEqual(cc.board_problems(self.root, 100), [])

    def test_docked_board_without_a_separator_fails(self):
        self.startup()
        write(self.root / "03-agents.txt", "› hello      Agents   1 · —\n• Hello      ◌ assistant\n")
        problems = cc.board_problems(self.root, 160)
        self.assertTrue(any("no `│` separator column" in p for p in problems))

    def test_hint_row_must_name_ctrl_g(self):
        self.startup(hint="  ? shortcuts                     100% context left")
        write(self.root / "03-agents.txt", "  Agents   1 · —\n")
        problems = cc.board_problems(self.root, 59)
        self.assertTrue(any("ctrl+g agents" in p for p in problems))

    def test_missing_frames_fail(self):
        problems = cc.board_problems(self.root, 59)
        self.assertTrue(any("01-startup.txt missing" in p for p in problems))
        self.assertTrue(any("03-agents.txt missing" in p for p in problems))

    def test_missing_board_header_fails(self):
        self.startup()
        write(self.root / "03-agents.txt", f"› hello\n{STATUS_LINE}\n")
        problems = cc.board_problems(self.root, 160)
        self.assertTrue(any("no `Agents` board header" in p for p in problems))


class MainMatrixTests(TempDirCase):
    """End-to-end: main() over a fixture directory, the same entry point
    scripts/ci.sh calls, so these guard the checker's own exit-code contract."""

    def run_main(self, argv):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            old_argv = sys.argv
            sys.argv = ["check_captures.py"] + argv
            try:
                code = cc.main()
            finally:
                sys.argv = old_argv
        return code, out.getvalue(), err.getvalue()

    def test_empty_directory_fails_loudly(self):
        code, _out, err = self.run_main([str(self.root)])
        self.assertEqual(code, 1)
        self.assertIn("no frames", err)

    def matrix(self, lane="full", root=None, theme="dark"):
        root = root or self.root
        root.mkdir(exist_ok=True)
        content = {
            "01-startup": f"{STATUS_LINE}\n{HINT_ROW}\n",
            "02c-wrapped-draft": "DRAFT END",
            "02d-narrow-draft": "DRAFT END",
            "02e-shift-enter": "First draft line\nSecond draft line",
            "03-agents": "\n".join([
                docked("", "Agents  2 · —"),
                docked(STATUS_LINE, "◌ assistant"),
                docked(HINT_ROW, "worker"),
            ]),
            "03a-failed-tool": cc.SGR.sub("", failed_cell()),
            "04-delegate": "Spawned worker\nworker finished",
            "11a-usage": "  Usage",
            "12-settings": "6. View mode standard",
            "15-quit": "Agents keep running in the daemon. Reattach with: aether",
            "26-approval": "1. Yes (y)\n4. No (esc)",
            "26a-child-approval": "1. Yes (y)\n4. No (esc)",
            "27-queued": "queued for after this turn",
        }
        for name, state in cc.SCHEDULE_STATES.items():
            content[name] = f"Documentation timer  {state}\nHistory"
        for name in cc.LANE_FRAMES[lane]:
            text = content.get(name, name)
            if name not in ("01-startup", "03-agents", "15-quit"):
                text += f"\n{STATUS_LINE}"
            write(root / f"{name}.txt", text + "\n")
            if name == "03a-failed-tool":
                ansi = failed_cell(bullet="", last=BOLD) if theme == "mono" else failed_cell()
            else:
                style = BOLD if theme == "mono" else RED
                ansi = f"{style}{text}{RESET}\n"
            write(root / f"{name}.ansi", ansi)

    def test_complete_full_and_smoke_matrices_pass(self):
        for lane in ("full", "smoke"):
            for theme in ("dark", "mono"):
                with self.subTest(lane=lane, theme=theme):
                    root = self.root / f"{lane}-{theme}"
                    self.matrix(lane, root, theme)
                    code, out, err = self.run_main([str(root), "--lane", lane, "--theme", theme])
                    self.assertEqual(code, 0, err)
                    self.assertIn(f"checked {len(cc.LANE_FRAMES[lane])} frames", out)

    def test_manifests_match_capture_script_paths(self):
        script = pathlib.Path(__file__).with_name("tui_capture.sh").read_text()
        # The colour lane is a review aid that exits on its own, not a checked lane.
        theme, rest = script.split('if [ "$THEME_LANE" = 1 ]; then', 1)
        theme_lane, rest = rest.split("\nfi\n", 1)
        self.assertIn("\n  exit 0", theme_lane)
        script = theme + rest
        before_mode, mode_and_rest = script.rsplit('if [ "$MODE_LANE" = 1 ]; then', 1)
        mode, rest = mode_and_rest.split("\nfi\n", 1)
        self.assertIn("\n  exit 0", mode)
        script = before_mode + rest
        before, smoke_and_full = script.split('if [ "$SMOKE" = 1 ]; then', 1)
        smoke, full = smoke_and_full.split("\nfi\n", 1)
        def names(text):
            found = set(re.findall(r"^snap ([0-9][\w-]+)", text, re.MULTILINE))
            if re.search(r"^  ?quit_and_capture$|^quit_and_capture$", text, re.MULTILINE):
                found.add(cc.QUIT_FRAME)
            # Smoke's commands are indented within its branch.
            found.update(re.findall(r"^  snap ([0-9][\w-]+)", text, re.MULTILINE))
            return found
        # The prefix also contains function definitions; only snap commands count.
        self.assertEqual(names(before) | names(smoke), cc.SMOKE_FRAMES)
        self.assertEqual(names(before) | names(full), cc.FULL_FRAMES)
        self.assertEqual(names(before) | names(mode), cc.MODE_FRAMES)

    def test_old_theme_names_are_aliases(self):
        self.matrix()
        code, out, err = self.run_main([str(self.root), "--theme", "dusk"])
        self.assertEqual(code, 0, err)
        self.assertIn("(dark)", out)

    def test_text_only_startup_and_board_cannot_pass(self):
        write(self.root / "01-startup.txt", f"{STATUS_LINE}\n{HINT_ROW}\n")
        write(self.root / "03-agents.txt", "  Agents  1 · —\n")
        code, _out, err = self.run_main([str(self.root), "--theme", "dark", "--width", "80", "--check-board"])
        self.assertEqual(code, 1)
        for name in ("01-startup.ansi", "26-approval.txt", "26a-child-approval.txt", "27-queued.txt", "15-quit.txt"):
            self.assertIn(name + " missing", err)

    def test_each_required_frame_and_ansi_peer_is_enforced(self):
        self.matrix()
        for name in sorted(cc.FULL_FRAMES):
            for suffix in ("txt", "ansi"):
                with self.subTest(name=name, suffix=suffix):
                    path = self.root / f"{name}.{suffix}"
                    original = path.read_bytes()
                    path.unlink()
                    try:
                        code, _out, err = self.run_main([str(self.root)])
                        self.assertEqual(code, 1)
                        self.assertIn(path.name + " missing", err)
                    finally:
                        path.write_bytes(original)

    def test_smoke_manifest_cannot_satisfy_default_full_lane(self):
        self.matrix("smoke")
        code, _out, err = self.run_main([str(self.root)])
        self.assertEqual(code, 1)
        self.assertIn("26-approval.txt missing", err)
        self.assertIn("27-queued.txt missing", err)

    def test_smoke_still_requires_quit_and_all_ansi_peers(self):
        self.matrix("smoke")
        (self.root / "15-quit.txt").unlink()
        (self.root / "02d-draft.ansi").unlink()
        code, _out, err = self.run_main([str(self.root), "--lane", "smoke"])
        self.assertEqual(code, 1)
        self.assertIn("15-quit.txt missing", err)
        self.assertIn("02d-draft.ansi missing", err)

    def test_empty_quit_ansi_cannot_satisfy_the_manifest(self):
        self.matrix("smoke")
        write(self.root / "15-quit.ansi", "")
        code, _out, err = self.run_main([str(self.root), "--lane", "smoke"])
        self.assertEqual(code, 1)
        self.assertIn("15-quit.ansi: nothing was painted", err)

    def test_incomplete_theme_comparison_fails(self):
        self.matrix()
        other = self.root / "other"
        self.matrix(root=other)
        write(other / "01-startup.ansi", f"{BOLD}different{RESET}")
        (other / "15-quit.ansi").unlink()
        code, _out, err = self.run_main([str(self.root), "--differs-from", str(other)])
        self.assertEqual(code, 1)
        self.assertIn("15-quit.ansi missing", err)

    def test_identical_complete_theme_comparison_fails(self):
        self.matrix()
        other = self.root / "other"
        self.matrix(root=other)
        code, _out, err = self.run_main([str(self.root), "--differs-from", str(other)])
        self.assertEqual(code, 1)
        self.assertIn("identical to", err)

    def test_missing_status_line_fails(self):
        write(self.root / "01-startup.txt", "› Message assistant\n")
        code, _out, err = self.run_main([str(self.root)])
        self.assertEqual(code, 1)
        self.assertIn("no status line", err)

    def test_stuck_screen_fails(self):
        write(self.root / "01-startup.txt", f"{STATUS_LINE}\n")
        write(self.root / "02-hello.txt", f"{STATUS_LINE}\n")
        code, _out, err = self.run_main([str(self.root), "--theme", "dark", "--width", "160"])
        self.assertEqual(code, 1)
        self.assertIn("every frame is identical", err)

    def test_background_rule_skips_mono_only(self):
        write(self.root / "01-startup.txt", f"{STATUS_LINE}\n")
        write(self.root / "01-startup.ansi", "\x1b[41ma\x1b[42mb\x1b[0m")
        code, _out, err = self.run_main([str(self.root), "--theme", "light"])
        self.assertEqual(code, 1)
        self.assertIn("background colours", err)
        code, _out, err = self.run_main([str(self.root), "--theme", "mono"])
        self.assertEqual(code, 1)
        self.assertNotIn("background colours", err)
        self.assertIn("mono painted colour", err)

    def test_quit_frame_may_paint_no_colour(self):
        self.matrix()
        write(self.root / "15-quit.txt", "Agents keep running in the daemon. Reattach with: aether\n")
        write(self.root / "15-quit.ansi", "Agents keep running in the daemon. Reattach with: aether\n")
        code, _out, err = self.run_main([str(self.root), "--theme", "dark"])
        self.assertEqual(code, 0, err)

    def test_board_flag_enforced(self):
        write(self.root / "01-startup.txt", f"{STATUS_LINE}\n{HINT_ROW}\n")
        write(self.root / "03-agents.txt", f"• Hello   Agents  1 · —\n{STATUS_LINE}\n")
        code, _out, err = self.run_main([str(self.root), "--width", "59", "--check-board"])
        self.assertEqual(code, 1)
        self.assertIn("text beside it", err)


if __name__ == "__main__":
    unittest.main()
