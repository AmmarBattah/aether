"""scripts/ci_scope.sh: which pull requests skip the Rust checks.

.github/workflows/ci.yml asks the script for a pull request and names the same
paths in its push trigger; the last test fails when the two lists part ways.
"""
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCOPE = ROOT / "scripts" / "ci_scope.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
DOCUMENTATION = (
    "README.md", "CHANGELOG.md", "LICENSE", "docs/guide.md", "docs/assets/aether_graphic.svg",
    "docs/assets/src/build.mjs", "clients/python/README.md", "evals/README.md",
    ".github/CODEOWNERS", ".github/dependabot.yml", ".github/ISSUE_TEMPLATE/bug_report.yml",
    ".github/PULL_REQUEST_TEMPLATE.md",
)
CODE = (
    "Cargo.toml", "Cargo.lock", "install.sh", "scripts/ci.sh", "crates/aether/src/main.rs",
    "THIRD-PARTY-NOTICES", "rust-toolchain.toml", "Makefile", "clients/python/aether/client.py",
    ".github/workflows/ci.yml", ".github/workflows/release.yml", "LICENSE.txt", "docs.rs",
    # `cargo test` reads these two documents.
    "docs/engines.md", "docs/tui.md",
)
# What the push trigger of ci.yml must say for the same rule; change both together.
PUSH_PATHS = ["**", "!**/*.md", "!docs/**", "!LICENSE", "!.github/**", ".github/workflows/**",
              "docs/engines.md", "docs/tui.md"]


def scope(paths):
    listing = "".join(f"{path}\n" for path in paths)
    result = subprocess.run([str(SCOPE)], input=listing, capture_output=True, text=True,
                            timeout=30, check=False)
    assert result.returncode == 0, result.stderr
    assert result.stdout in ("code=true\n", "code=false\n"), result.stdout
    return result.stdout.strip()


class CiScopeTests(unittest.TestCase):
    def test_documentation_alone_needs_no_rust_checks(self):
        for path in DOCUMENTATION:
            with self.subTest(path=path):
                self.assertEqual(scope([path]), "code=false")
        self.assertEqual(scope(DOCUMENTATION), "code=false")

    def test_any_other_path_needs_them(self):
        for path in CODE:
            with self.subTest(path=path):
                self.assertEqual(scope([path]), "code=true")
                self.assertEqual(scope([*DOCUMENTATION, path]), "code=true")
                self.assertEqual(scope([path, *DOCUMENTATION]), "code=true")

    def test_no_changed_path_runs_everything(self):
        self.assertEqual(scope([]), "code=true")
        self.assertEqual(scope([""]), "code=true")

    def test_every_documentation_path_exists_in_this_tree(self):
        for path in (*DOCUMENTATION, *CODE):
            if path not in ("LICENSE.txt", "docs.rs"):
                with self.subTest(path=path):
                    self.assertTrue((ROOT / path).exists(), f"{path} is a stale example")

    def test_the_push_trigger_names_the_same_paths(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        block = re.search(r"^  push:\n(?:    .*\n|\n)*?    paths:\n((?:      .*\n)+)", workflow, re.M)
        self.assertIsNotNone(block, "ci.yml has no paths filter on its push trigger")
        paths = re.findall(r'^      - "([^"]+)"$', block.group(1), re.M)
        self.assertEqual(paths, PUSH_PATHS)
        self.assertNotIn("paths", re.search(r"^  pull_request:\n((?:    .*\n)*)", workflow, re.M).group(1),
                         "a path filter on pull_request leaves a required check waiting")


if __name__ == "__main__":
    unittest.main()
