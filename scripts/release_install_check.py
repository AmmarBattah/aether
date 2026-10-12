#!/usr/bin/env python3
"""Installs a packed release archive the way a user does, and checks the result.

  python3 scripts/release_install_check.py <target> <directory> <version>

`<directory>` holds aether-<target>.tar.gz and its .sha256, as
`scripts/release.sh package` leaves them. They are served from this machine in
the layout of a GitHub release. The real install.sh installs from there into
temporary directories, and the installed binary is asked for its version and
for an update. Nothing here reaches github.com or the user's home.
"""
import functools
import http.server
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import threading

ROOT = Path(__file__).resolve().parent.parent
INSTALLER = ROOT / "install.sh"
RECEIPT = ".aether-receipt"
ARCHIVE_MEMBERS = ["LICENSE", "THIRD-PARTY-NOTICES", "aether"]
VERSION = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")
PROXY_VARIABLES = ("http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY",
                   "all_proxy", "ALL_PROXY")
# Settings of the caller's own Aether that must not reach the one under test.
AETHER_VARIABLES = ("AETHER_VERSION", "AETHER_INSTALL_DIR", "AETHER_INSTALL_BASE_URL",
                    "AETHER_SOCKET", "AETHER_CONFIG_DIR", "AETHER_DATA_DIR")
TIMEOUT_SECONDS = 120


def require(condition, message):
    if not condition:
        sys.exit(f"release install check failed: {message}")


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    """Serves the fake release and records each path asked for."""
    requested = None

    def log_message(self, format, *args):
        pass

    def do_GET(self):
        assert self.requested is not None, "bind `requested` in a subclass per server"
        self.requested.append(self.path)
        super().do_GET()


def check_archive(archive):
    """The archive holds the binary, the licence and the notices, and nothing else."""
    with tarfile.open(archive, "r:gz") as tar:
        members = tar.getmembers()
    names = sorted(member.name for member in members)
    require(names == ARCHIVE_MEMBERS, f"{archive.name} holds {names}, not {ARCHIVE_MEMBERS}")
    for member in members:
        require(member.isfile() and member.size > 0,
                f"{member.name} in {archive.name} is not a regular, non-empty file")


def publish(www, archive, tag):
    """Lay the assets out as GitHub does: download/<tag>/ and latest/download/."""
    checksum = archive.with_name(f"{archive.name}.sha256")
    require(checksum.is_file(), f"{checksum} does not exist")
    for release in (www / "download" / tag, www / "latest" / "download"):
        release.mkdir(parents=True)
        shutil.copy(archive, release / archive.name)
        shutil.copy(checksum, release / checksum.name)
        (release / "VERSION").write_text(f"{tag}\n")


def serve(www):
    """Start a server for `www`; returns it, its base URL and the paths requested."""
    requested = []
    recording = type("Handler", (QuietHandler,), {"requested": requested})
    handler = functools.partial(recording, directory=str(www))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, f"http://127.0.0.1:{server.server_address[1]}", requested


def environment(root, base_url, **settings):
    """The caller's environment, with every Aether and home path under `root`."""
    env = {name: value for name, value in os.environ.items()
           if name not in PROXY_VARIABLES + AETHER_VARIABLES}
    env.update({
        "HOME": str(root / "home"),
        "XDG_CONFIG_HOME": str(root / "home" / "config"),
        "XDG_DATA_HOME": str(root / "home" / "data"),
        "XDG_RUNTIME_DIR": str(root / "home" / "run"),
        "AETHER_CONFIG_DIR": str(root / "home" / "aether-config"),
        "AETHER_DATA_DIR": str(root / "home" / "aether-data"),
        "AETHER_INSTALL_BASE_URL": base_url,
        "NO_PROXY": "127.0.0.1",
        "no_proxy": "127.0.0.1",
    })
    env.update(settings)
    return env


def run(command, cwd, env):
    return subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True,
                          timeout=TIMEOUT_SECONDS, check=False)


def install(root, base_url, install_dir, **settings):
    """Run install.sh from an empty directory and require exactly its two files."""
    empty = Path(tempfile.mkdtemp(prefix="cwd-", dir=root))
    env = environment(root, base_url, AETHER_INSTALL_DIR=str(install_dir), **settings)
    result = run(["sh", str(INSTALLER)], empty, env)
    require(result.returncode == 0,
            f"install.sh exited {result.returncode}:\n{result.stdout}{result.stderr}")
    installed = sorted(path.name for path in install_dir.iterdir())
    require(installed == [RECEIPT, "aether"],
            f"{install_dir} holds {installed}, not the binary and its receipt")
    require(not any(empty.iterdir()), "install.sh wrote into the directory it was run from")
    return result.stdout


def check_installed(root, base_url, install_dir, version):
    """The installed binary is this build, and says it is the latest release."""
    binary = install_dir / "aether"
    env = environment(root, base_url)
    reported = run([str(binary), "--version"], root, env)
    require(reported.returncode == 0 and reported.stdout.strip() == f"aether {version}",
            f"`aether --version` printed {reported.stdout!r} (exit {reported.returncode})")
    check = run([str(binary), "update", "--check"], root, env)
    require(check.returncode == 0,
            f"`aether update --check` exited {check.returncode}, not 0 for up to date:\n"
            f"{check.stdout}{check.stderr}")
    require(check.stdout.strip() == f"aether {version} is up to date.",
            f"`aether update --check` printed {check.stdout!r}")
    installed = sorted(path.name for path in install_dir.iterdir())
    require(installed == [RECEIPT, "aether"], f"`aether update --check` left {installed}")


def check_release(target, directory, version):
    archive = directory / f"aether-{target}.tar.gz"
    require(archive.is_file(), f"{archive} does not exist")
    check_archive(archive)
    tag = f"v{version}"
    with tempfile.TemporaryDirectory(prefix="aether-install-check-") as temporary:
        root = Path(temporary).resolve()
        (root / "home").mkdir()
        publish(root / "www", archive, tag)
        server, base_url, requested = serve(root / "www")
        try:
            latest, pinned = root / "latest-bin", root / "pinned-bin"
            install(root, base_url, latest)
            install(root, base_url, pinned, AETHER_VERSION=tag)
            require(not any((root / "home").iterdir()), "install.sh wrote into the home")
            for install_dir in (latest, pinned):
                check_installed(root, base_url, install_dir, version)
        finally:
            server.shutdown()
            server.server_close()
        for layout in (f"/latest/download/{archive.name}", f"/download/{tag}/{archive.name}",
                       "/latest/download/VERSION"):
            require(layout in requested, f"nothing asked for {layout}: {requested}")


def main(arguments):
    if len(arguments) != 3:
        sys.exit("usage: release_install_check.py <target> <directory> <version>")
    target, directory, version = arguments[0], Path(arguments[1]).resolve(), arguments[2]
    require(VERSION.match(version), f"'{version}' is not MAJOR.MINOR.PATCH")
    require(directory.is_dir(), f"{directory} is not a directory")
    check_release(target, directory, version)
    print(f"install check passed for {target}: install.sh installed aether {version} "
          "(latest and pinned), and `aether update --check` reports it up to date")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
