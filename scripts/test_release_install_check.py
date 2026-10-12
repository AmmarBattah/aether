"""scripts/release_install_check.py against archives of a fake `aether`.

The check installs with the real install.sh from a server it starts itself, so
these tests need no build and reach neither github.com nor the user's home.
"""
import hashlib
import io
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
CHECK = ROOT / "scripts" / "release_install_check.py"
VERSION = "1.2.3"
MACHINES = {"x86_64": "x86_64", "amd64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}
SYSTEMS = {"Linux": "unknown-linux-musl", "Darwin": "apple-darwin"}
# As the real binary learns the newest release: from the server it is pointed at.
ASK_FOR_LATEST = 'curl -fsS "$AETHER_INSTALL_BASE_URL/latest/download/VERSION" >/dev/null || exit 1'
FAKE_AETHER = """#!/bin/sh
case "$*" in
  --version) echo "aether {reported}" ;;
  "update --check")
    {ask_for_latest}
    echo "{update_line}"
    exit {update_status} ;;
  *) echo "unexpected arguments: $*" >&2; exit 2 ;;
esac
"""


def host_target():
    """The archive install.sh asks for on this machine."""
    machine, system = platform.machine().lower(), platform.system()
    assert machine in MACHINES and system in SYSTEMS, f"no release for {system} {machine}"
    return f"{MACHINES[machine]}-{SYSTEMS[system]}"


def fake_aether(reported=VERSION, update_line=f"aether {VERSION} is up to date.",
                update_status=0, ask_for_latest=ASK_FOR_LATEST):
    script = FAKE_AETHER.format(reported=reported, update_line=update_line,
                                update_status=update_status, ask_for_latest=ask_for_latest)
    return script.encode()


def pack(directory, members):
    """Write this host's archive and its .sha256 into `directory`."""
    archive = directory / f"aether-{host_target()}.tar.gz"
    with tarfile.open(archive, "w:gz") as tar:
        for name, mode, data in members:
            member = tarfile.TarInfo(name)
            member.mode = mode
            member.size = len(data)
            tar.addfile(member, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    Path(f"{archive}.sha256").write_text(f"{digest}  {archive.name}\n")
    return archive


def release_members(binary):
    return [("aether", 0o755, binary), ("LICENSE", 0o644, b"MIT\n"),
            ("THIRD-PARTY-NOTICES", 0o644, b"notices\n")]


class InstallCheckTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="aether-install-check-test-")
        self.addCleanup(temporary.cleanup)
        self.dist = Path(temporary.name)

    def check(self, version=VERSION):
        return subprocess.run([sys.executable, str(CHECK), host_target(), str(self.dist), version],
                              capture_output=True, text=True, timeout=120, check=False)

    def assert_refused(self, result, reason):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("release install check failed", result.stderr)
        self.assertIn(reason, result.stderr)

    def test_an_archive_that_installs_and_is_up_to_date_passes(self):
        pack(self.dist, release_members(fake_aether()))
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"install.sh installed aether {VERSION} (latest and pinned)", result.stdout)
        self.assertEqual(sorted(path.name for path in self.dist.iterdir()),
                         [f"aether-{host_target()}.tar.gz", f"aether-{host_target()}.tar.gz.sha256"])

    def test_an_archive_without_the_notices_is_refused(self):
        pack(self.dist, release_members(fake_aether())[:2])
        self.assert_refused(self.check(), "holds ['LICENSE', 'aether']")

    def test_an_archive_with_one_more_member_is_refused(self):
        pack(self.dist, release_members(fake_aether()) + [("README.md", 0o644, b"hello\n")])
        self.assert_refused(self.check(), "README.md")

    def test_a_binary_of_another_version_is_refused(self):
        pack(self.dist, release_members(fake_aether(reported="0.9.0")))
        self.assert_refused(self.check(), "`aether --version` printed 'aether 0.9.0\\n'")

    def test_a_binary_that_sees_an_update_is_refused(self):
        binary = fake_aether(update_line="aether 9.9.9 is available", update_status=10)
        pack(self.dist, release_members(binary))
        self.assert_refused(self.check(), "`aether update --check` exited 10, not 0")

    def test_a_binary_that_never_asks_the_server_is_refused(self):
        pack(self.dist, release_members(fake_aether(ask_for_latest=":")))
        self.assert_refused(self.check(), "nothing asked for /latest/download/VERSION")

    def test_a_checksum_that_does_not_match_is_refused(self):
        archive = pack(self.dist, release_members(fake_aether()))
        Path(f"{archive}.sha256").write_text(f"{'0' * 64}  {archive.name}\n")
        self.assert_refused(self.check(), "install.sh exited")

    def test_a_missing_archive_and_a_malformed_version_are_refused(self):
        self.assert_refused(self.check(), "does not exist")
        self.assert_refused(self.check(version="1.2"), "'1.2' is not MAJOR.MINOR.PATCH")


if __name__ == "__main__":
    unittest.main()
