"""scripts/release.sh, for the steps that need no build: the notes and the archive."""
import hashlib
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / "scripts" / "release.sh"
INSTALL_COMMAND = "curl -fsSL https://raw.githubusercontent.com/mengsig/aether/main/install.sh | sh"
CHANGELOG = """# Changelog

## 1.0.10

Tenth patch.

## 1.0.1

First patch.

### Fixed

- One thing.

## 1.0.0

## 0.9.0

Before the first release.
"""


def release(*arguments):
    return subprocess.run([str(RELEASE), *arguments], capture_output=True, text=True,
                          timeout=60, check=False)


class ReleaseNotesTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.changelog = Path(directory.name) / "CHANGELOG.md"
        self.changelog.write_text(CHANGELOG)

    def notes(self, tag):
        return release("notes", tag, str(self.changelog))

    def test_notes_are_the_tags_section_then_the_install_command(self):
        result = self.notes("v1.0.1")
        self.assertEqual(result.returncode, 0, result.stderr)
        body, install = result.stdout.split("\n## Install\n")
        self.assertEqual(body, "First patch.\n\n### Fixed\n\n- One thing.\n")
        self.assertIn(f"```sh\n{INSTALL_COMMAND}\n```", install)
        self.assertIn("AETHER_VERSION=v1.0.1", install)

    def test_a_longer_version_with_the_same_prefix_is_another_section(self):
        result = self.notes("v1.0.10")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("Tenth patch.\n"), result.stdout)
        self.assertNotIn("First patch.", result.stdout)

    def test_the_last_section_runs_to_the_end_of_the_file(self):
        result = self.notes("v0.9.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("Before the first release.\n\n## Install\n"),
                        result.stdout)

    def test_a_missing_or_empty_section_fails_and_prints_no_notes(self):
        for tag, version in (("v2.0.0", "2.0.0"), ("v1.0.0", "1.0.0")):
            with self.subTest(tag=tag):
                result = self.notes(tag)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stdout, "")
                self.assertIn(f"has no section headed exactly '## {version}'", result.stderr)
                self.assertIn(f"before tagging {tag}", result.stderr)

    def test_a_missing_changelog_and_a_malformed_tag_are_refused(self):
        missing = release("notes", "v1.0.1", str(self.changelog.with_name("absent.md")))
        self.assertEqual(missing.returncode, 1)
        self.assertIn("does not exist", missing.stderr)
        for tag in ("1.0.1", "v1.0", "v1.0.1-rc1"):
            with self.subTest(tag=tag):
                malformed = self.notes(tag)
                self.assertEqual(malformed.returncode, 1)
                self.assertIn("is not of the form v1.2.3", malformed.stderr)

    def test_the_changelog_has_notes_for_the_workspace_version(self):
        version = release("version")
        self.assertEqual(version.returncode, 0, version.stderr)
        result = release("notes", f"v{version.stdout.strip()}")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(INSTALL_COMMAND, result.stdout)
        self.assertNotRegex(result.stdout, r"(?m)^## (?!Install$)", "one release's section only")


class ArchiveTests(unittest.TestCase):
    TARGET = "aarch64-apple-darwin"

    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.binary = self.root / "build" / "aether"
        self.binary.parent.mkdir()
        self.binary.write_text("#!/bin/sh\necho built\n")
        self.binary.chmod(0o755)
        self.out = self.root / "dist"

    def test_an_archive_holds_the_binary_the_licence_and_the_notices(self):
        result = release("archive", self.TARGET, str(self.binary), str(self.out))
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.out / f"aether-{self.TARGET}.tar.gz"
        with tarfile.open(archive, "r:gz") as tar:
            members = {member.name: member for member in tar.getmembers()}
            packed = {name: tar.extractfile(member).read() for name, member in members.items()}
        self.assertEqual(sorted(members), ["LICENSE", "THIRD-PARTY-NOTICES", "aether"])
        self.assertEqual(packed["aether"], self.binary.read_bytes())
        self.assertTrue(members["aether"].mode & 0o111, "the binary stays executable")
        for document in ("LICENSE", "THIRD-PARTY-NOTICES"):
            self.assertEqual(packed[document], (ROOT / document).read_bytes())
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        published = Path(f"{archive}.sha256").read_text()
        self.assertEqual(published, f"{digest}  {archive.name}\n")

    def test_what_is_not_a_releasable_binary_is_refused(self):
        misnamed = self.binary.with_name("aether-new")
        misnamed.write_bytes(self.binary.read_bytes())
        misnamed.chmod(0o755)
        plain = self.root / "aether"
        plain.write_text("not executable\n")
        cases = (
            (("x86_64-pc-windows-msvc", self.binary), "is not a release target"),
            ((self.TARGET, misnamed), "is not named aether"),
            ((self.TARGET, plain), "is not an executable file"),
            ((self.TARGET, self.root / "absent" / "aether"), "is not an executable file"),
        )
        for (target, binary), message in cases:
            with self.subTest(message=message, binary=binary.name):
                result = release("archive", target, str(binary), str(self.out))
                self.assertEqual(result.returncode, 1)
                self.assertIn(message, result.stderr)
                self.assertFalse(self.out.exists(), "nothing is packed")


if __name__ == "__main__":
    unittest.main()
