import functools
import re

from .errors import InvalidVersion

_VERSION = re.compile(r"(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+([0-9A-Za-z.-]+))?")


def prerelease_identifiers(text):
    return tuple(int(identifier) if identifier.isdigit() else identifier for identifier in text.split("."))


@functools.total_ordering
class Version:
    __slots__ = ("major", "minor", "patch", "prerelease", "build")

    def __init__(self, major, minor, patch, prerelease=(), build=()):
        self.major, self.minor, self.patch = major, minor, patch
        self.prerelease, self.build = tuple(prerelease), tuple(build)

    @property
    def core(self):
        return (self.major, self.minor, self.patch)

    def _key(self):
        return (self.major, self.minor, self.patch, self.prerelease, self.build)

    def __str__(self):
        text = f"{self.major}.{self.minor}.{self.patch}"
        if self.prerelease:
            text += "-" + ".".join(str(identifier) for identifier in self.prerelease)
        if self.build:
            text += "+" + ".".join(self.build)
        return text

    def __repr__(self):
        return f"Version({str(self)!r})"

    def __eq__(self, other):
        return isinstance(other, Version) and self._key() == other._key()

    def __lt__(self, other):
        return self._key()[:4] < other._key()[:4]

    def __hash__(self):
        return hash(self._key())


def parse(text):
    match = _VERSION.fullmatch(text)
    if match is None:
        raise InvalidVersion(f"invalid version: {text!r}")
    major, minor, patch, prerelease, build = match.groups()
    return Version(int(major), int(minor), int(patch),
                   prerelease_identifiers(prerelease) if prerelease else (),
                   tuple(build.split(".")) if build else ())


def coerce(value):
    return value if isinstance(value, Version) else parse(value)


def compare(a, b):
    a, b = coerce(a), coerce(b)
    if a == b:
        return 0
    return -1 if a < b else 1
