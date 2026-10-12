#!/usr/bin/env python3
"""Writes THIRD-PARTY-NOTICES: the crates in a released `aether` and their licences.

  python3 scripts/third_party_notices.py           rewrite it from Cargo.lock
  python3 scripts/third_party_notices.py --check   fail when it is out of date

The crates are those `cargo tree -e normal` lists for the `aether` package on
the four release targets: what is linked in, and the procedural macros whose
output is compiled in. Dependencies of build scripts and tests are not listed.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
NOTICES = ROOT / "THIRD-PARTY-NOTICES"
BINARY_PACKAGE = "aether"
# The archives scripts/release.sh builds; keep the two in step.
TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
)
LICENCE_FILE = re.compile(r"^(licen[sc]e|copying|notice|unlicense|copyright)", re.IGNORECASE)
TREE_LINE = re.compile(r"^(?P<name>[A-Za-z0-9_-]+) v(?P<version>\S+)(?P<rest>( \([^)]*\))*)$")
RULE = "-" * 78
HEADER = """\
Third-party notices for Aether
==============================

Aether is released under the MIT licence (see LICENSE). A released `aether`
binary also contains code from the crates listed here, each under its own
licence. The list covers the four release targets and includes procedural
macros, whose output is compiled in. The licence texts those crates ship
follow the list; a text shared by several crates is printed once.

Generated from Cargo.lock by scripts/third_party_notices.py. Do not edit it
by hand: after a dependency changes, run

    python3 scripts/third_party_notices.py
"""


def cargo(*arguments):
    """Run cargo in the workspace and return its stdout, or exit with its error."""
    # CI sets CARGO_TERM_COLOR=always, which colours `cargo tree` even into a pipe.
    plain = {**os.environ, "CARGO_TERM_COLOR": "never"}
    result = subprocess.run(["cargo", *arguments], cwd=ROOT, capture_output=True,
                            text=True, check=False, env=plain)
    if result.returncode != 0:
        sys.exit(f"cargo {' '.join(arguments)} failed:\n{result.stderr.strip()}")
    return result.stdout


def parse_tree(listing):
    """(name, version) of each third-party crate in `cargo tree --prefix none` output."""
    crates = set()
    for line in filter(None, (line.strip() for line in listing.splitlines())):
        matched = TREE_LINE.match(line)
        assert matched, f"unexpected `cargo tree` line: {line!r}"
        # A crate of this workspace is printed with its path.
        if " (/" not in matched["rest"]:
            crates.add((matched["name"], matched["version"]))
    return crates


def binary_crates():
    """Every third-party crate the binary is built from, on any release target."""
    crates = set()
    for target in TARGETS:
        listing = cargo("tree", "--locked", "-p", BINARY_PACKAGE, "-e", "normal",
                        "--target", target, "--prefix", "none", "--format", "{p}")
        found = parse_tree(listing)
        assert found, f"cargo tree lists no dependency of {BINARY_PACKAGE} for {target}"
        crates |= found
    return crates


def registry_packages():
    """Cargo's record of each third-party package, by (name, version)."""
    packages = {}
    for target in TARGETS:
        metadata = json.loads(cargo("metadata", "--locked", "--format-version", "1",
                                    "--filter-platform", target))
        for package in metadata["packages"]:
            if package["source"] is None:
                continue
            key = (package["name"], package["version"])
            known = packages.setdefault(key, package)
            assert known["id"] == package["id"], f"{key} comes from two sources"
    return packages


def normalised(text):
    """`text` with Unix line ends and no trailing blanks or surrounding blank lines."""
    lines = [line.rstrip() for line in text.replace("\r\n", "\n").split("\n")]
    return "\n".join(lines).strip("\n")


def licence_texts(package):
    """(file name, text) of each licence file at the root of the package."""
    directory = Path(package["manifest_path"]).parent
    assert directory.is_dir(), f"{directory} is not unpacked; run `cargo fetch --locked`"
    texts = []
    for path in sorted(directory.iterdir(), key=lambda path: path.name):
        if path.is_file() and LICENCE_FILE.match(path.name):
            text = normalised(path.read_text(encoding="utf-8", errors="replace"))
            if text:
                texts.append((path.name, text))
    return texts


def describe(package):
    """One crate as the notices list it, with the licence texts it ships."""
    licence = package["license"] or f"see {package['license_file']}"
    assert licence, f"{package['name']} declares no licence"
    return {
        "name": package["name"],
        "version": package["version"],
        "licence": licence,
        "repository": package["repository"] or "",
        "texts": licence_texts(package),
    }


def crate_list(crates):
    lines = ["Crates", "------", ""]
    for crate in crates:
        line = f"{crate['name']} {crate['version']}: {crate['licence']}"
        if crate["repository"]:
            line += f" ({crate['repository']})"
        lines.append(line)
        if not crate["texts"]:
            lines.append("    its package ships no licence file; the licence named above applies")
    return lines


def text_sections(crates):
    """Each distinct licence text once, under the crates and files that carry it."""
    carriers, texts = {}, {}
    for crate in crates:
        for file_name, text in crate["texts"]:
            key = " ".join(text.split())
            texts.setdefault(key, text)
            carriers.setdefault(key, []).append(
                f"{crate['name']} {crate['version']} ({file_name})")
    lines = ["Licence texts", "-------------"]
    for key, text in texts.items():
        lines += ["", RULE, *carriers[key], RULE, "", text]
    return lines


def render(crates):
    """The whole notices file for `crates`, which must not be empty."""
    assert crates, "a release binary has third-party crates"
    crates = sorted(crates, key=lambda crate: (crate["name"], crate["version"]))
    names = [(crate["name"], crate["version"]) for crate in crates]
    assert len(set(names)) == len(names), "a crate is listed twice"
    lines = [HEADER, *crate_list(crates), "", *text_sections(crates)]
    return "\n".join(lines) + "\n"


def generate():
    packages = registry_packages()
    wanted = binary_crates()
    missing = sorted(wanted - packages.keys())
    assert not missing, f"cargo metadata does not know {missing}"
    return render([describe(packages[key]) for key in wanted])


def out_of_date(expected, path):
    """Why `path` is not `expected`, or None when it is."""
    if not path.is_file():
        return f"{path.name} does not exist"
    if path.read_text(encoding="utf-8") != expected:
        return f"{path.name} does not match Cargo.lock"
    return None


def main(arguments):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="change nothing; exit 1 when the file is out of date")
    options = parser.parse_args(arguments)
    expected = generate()
    if not options.check:
        NOTICES.write_text(expected, encoding="utf-8")
        print(f"wrote {NOTICES.relative_to(ROOT)}")
        return 0
    problem = out_of_date(expected, NOTICES)
    if problem:
        print(f"{problem}; regenerate it with: python3 scripts/third_party_notices.py",
              file=sys.stderr)
        return 1
    print(f"{NOTICES.relative_to(ROOT)} matches Cargo.lock")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
