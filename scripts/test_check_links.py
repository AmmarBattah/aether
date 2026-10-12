"""scripts/check_links.py against small trees: what it must accept and refuse."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECK = Path(__file__).resolve().parent / "check_links.py"
GUIDE = """# Guide

## Set up
<a id="by-hand"></a>
## `aether update`: what it does
## Set up
"""


def run(files):
    """Check a new repository holding `files`; returns (exit code, stderr)."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for name, text in files.items():
            (root / name).parent.mkdir(parents=True, exist_ok=True)
            (root / name).write_text(text, encoding="utf-8")
        subprocess.run(["git", "init", "-q", str(root)], check=True)
        subprocess.run(["git", "-C", str(root), "add", "-A"], check=True)
        result = subprocess.run([sys.executable, str(CHECK), str(root)], capture_output=True,
                                text=True, timeout=30, check=False)
    assert result.returncode in (0, 1), result.stderr
    return result.returncode, result.stderr


class CheckLinksTests(unittest.TestCase):
    def test_links_that_resolve_pass(self):
        readme = ("[a](docs/guide.md) [b](docs/guide.md#set-up) [c](docs/guide.md#set-up-1)\n"
                  "[d](docs/guide.md#aether-update-what-it-does) [e](docs/guide.md#by-hand)\n"
                  "[f](#here) [g](https://example.com/missing#x) [h](docs/) [i](mailto:a@example.com)\n"
                  '<img src="docs/logo.svg" alt=""> [j](docs/logo.svg "title")\n\n[ref]: docs/guide.md#guide\n\n## Here\n')
        code, stderr = run({"README.md": readme, "docs/guide.md": GUIDE + "[up](../README.md#here)\n",
                            "docs/logo.svg": "<svg/>"})
        self.assertEqual((code, stderr), (0, ""))

    def test_a_missing_file_and_a_missing_heading_are_named_with_their_line(self):
        code, stderr = run({"README.md": "# T\n\n[a](docs/nope.md)\n[b](docs/guide.md#set-up-2)\n[c](#nope)\n",
                            "docs/guide.md": GUIDE})
        self.assertEqual(code, 1)
        self.assertIn("README.md:3: docs/nope.md does not exist", stderr)
        self.assertIn("README.md:4: docs/guide.md#set-up-2 names no heading of docs/guide.md", stderr)
        self.assertIn("README.md:5: #nope names no heading of README.md", stderr)
        self.assertIn("3 broken link(s) in 2 documents", stderr)

    def test_reference_definitions_and_html_are_checked_too(self):
        code, stderr = run({"README.md": '# T\n\n[ref]: gone.md\n<a href="also-gone.md">x</a>\n'})
        self.assertEqual(code, 1)
        self.assertIn("README.md:3: gone.md does not exist", stderr)
        self.assertIn("README.md:4: also-gone.md does not exist", stderr)

    def test_a_link_out_of_the_repository_is_refused(self):
        code, stderr = run({"README.md": "# T\n\n[a](../outside.md)\n"})
        self.assertEqual(code, 1)
        self.assertIn("README.md:3: ../outside.md leaves the repository", stderr)

    def test_code_is_not_prose(self):
        text = "# T\n\n`[a](gone.md)`\n\n```md\n[b](gone.md)\n## Not a heading\n```\n\n[c](#not-a-heading)\n"
        code, stderr = run({"README.md": text})
        self.assertEqual(code, 1)
        self.assertEqual(stderr.count("README.md:"), 1, stderr)
        self.assertIn("README.md:10: #not-a-heading names no heading", stderr)


if __name__ == "__main__":
    unittest.main()
