"""Check that terminal background cells survive parsing and PNG rendering."""
import io
import pathlib
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

try:
    from PIL import Image, ImageFont
    import ansi2png
except ModuleNotFoundError as error:
    if error.name != "PIL":
        raise
    ansi2png = None


@unittest.skipIf(ansi2png is None, "ANSI screenshots require Pillow")
class BackgroundTests(unittest.TestCase):
    def test_trailing_background_spaces_survive_parse_and_render(self):
        tint = (30, 34, 39)
        ground = (13, 17, 23)
        rows = ansi2png.parse("\x1b[48;2;30;34;39mx    \x1b[49m\n     \n")
        self.assertEqual([len(row) for row in rows], [5, 5])
        self.assertTrue(all(style.bg == tint for _, style in rows[0]))
        self.assertTrue(all(style.bg is None for _, style in rows[1]))

        # A bundled font keeps this check independent of system font installs.
        font = ImageFont.load_default()
        cell_width = font.getbbox("M")[2] - font.getbbox("M")[0]
        cell_height = int(14 * 1.35)
        output = io.BytesIO()
        output.name = "fixture.png"
        with patch.object(ansi2png.ImageFont, "truetype", return_value=font):
            ansi2png.render(
                rows, "fixture-Regular.ttf", 14, output,
                ground=((204, 204, 204), ground),
            )
        output.seek(0)
        with Image.open(output) as image:
            self.assertEqual(image.size, (5 * cell_width + 32, 2 * cell_height + 32))
            for column in range(1, 5):
                self.assertEqual(image.getpixel((16 + column * cell_width + 1, 17)), tint)
            self.assertEqual(image.getpixel((16 + 4 * cell_width + 1, 17 + cell_height)), ground)

    def test_blank_rows_retain_background_carried_across_newlines(self):
        rows = ansi2png.parse("\x1b[48;2;30;34;39mx    \n     \n")
        self.assertEqual([len(row) for row in rows], [5, 5])
        self.assertTrue(all(style.bg == (30, 34, 39) for _, style in rows[1]))

    @unittest.skipUnless(shutil.which("tmux"), "terminal capture requires tmux")
    def test_tmux_preserves_trailing_tinted_spaces_with_N(self):
        with tempfile.TemporaryDirectory(prefix="ansi-", dir="/tmp") as directory:
            socket = str(pathlib.Path(directory) / "tmux.sock")
            command = ["tmux", "-S", socket, "-f", "/dev/null"]
            # A private pane keeps the operator's terminal and tmux untouched.
            paint = (
                "import sys,time; "
                "sys.stdout.write('\\x1b[48;2;30;34;39mx    \\x1b[49m\\r\\nREADY\\r\\n'); "
                "sys.stdout.flush(); time.sleep(10)"
            )
            try:
                subprocess.run(
                    command + ["new-session", "-d", "-s", "background", "-x", "80", "-y", "24",
                               shlex.join([sys.executable, "-c", paint])],
                    check=True, timeout=5,
                )
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    capture = subprocess.run(
                        command + ["capture-pane", "-t", "background", "-p", "-e", "-N"],
                        check=True, capture_output=True, text=True, timeout=5,
                    ).stdout
                    if "READY" in capture:
                        break
                    time.sleep(0.05)
                else:
                    self.fail("the private tmux fixture did not paint READY")
                row = ansi2png.parse(capture)[0]
                self.assertEqual("".join(char for char, _ in row[:5]), "x    ")
                self.assertTrue(all(style.bg == (30, 34, 39) for _, style in row[:5]))
            finally:
                subprocess.run(command + ["kill-server"], check=False, capture_output=True, timeout=5)


if __name__ == "__main__":
    unittest.main()
