import functools
import re

from .errors import InvalidVersion

NUMBER = r"(?:0|[1-9][0-9]*)"
IDENTIFIERS = r"[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*"
_VERSION = re.compile(rf"({NUMBER})\.({NUMBER})\.({NUMBER})(?:-({IDENTIFIERS}))?(?:\+({IDENTIFIERS}))?")


def prerelease_identifiers(text):
    identifiers = []
    for identifier in text.split("."):
        if identifier.isdigit():
            if len(identifier) > 1 and identifier[0] == "0":
                raise InvalidVersion(f"numeric prerelease identifier has a leading zero: {identifier!r}")
            identifiers.append(int(identifier))
        else:
            identifiers.append(identifier)
    return tuple(identifiers)


@functools.total_ordering
class Version:
    __slots__ = ("major", "minor", "patch", "prerelease", "build")

    def __init__(self, major, minor, patch, prerelease=(), build=()):
        self.major, self.minor, self.patch = major, minor, patch
        self.prerelease, self.build = tuple(prerelease), tuple(build)

    @property
    def core(self):
        return (self.major, self.minor, self.patch)

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
        if not isinstance(other, Version):
            return NotImplemented
        return _compare(self, other) == 0

    def __lt__(self, other):
        if not isinstance(other, Version):
            return NotImplemented
        return _compare(self, other) < 0

    def __hash__(self):
        return hash((self.core, self.prerelease))


def parse(text):
    if not isinstance(text, str):
        raise InvalidVersion(f"version must be a string, got {type(text).__name__}")
    match = _VERSION.fullmatch(text)
    if match is None:
        raise InvalidVersion(f"invalid version: {text!r}")
    major, minor, patch, prerelease, build = match.groups()
    return Version(int(major), int(minor), int(patch),
                   prerelease_identifiers(prerelease) if prerelease else (),
                   tuple(build.split(".")) if build else ())


def _sign(value):
    return (value > 0) - (value < 0)


def _compare_identifier(left, right):
    left_numeric, right_numeric = isinstance(left, int), isinstance(right, int)
    if left_numeric and right_numeric:
        return _sign(left - right)
    if left_numeric != right_numeric:
        return -1 if left_numeric else 1
    return (left > right) - (left < right)


def _compare(left, right):
    if left.core != right.core:
        return -1 if left.core < right.core else 1
    if not left.prerelease:
        return 0 if not right.prerelease else 1
    if not right.prerelease:
        return -1
    for a, b in zip(left.prerelease, right.prerelease):
        result = _compare_identifier(a, b)
        if result:
            return result
    return _sign(len(left.prerelease) - len(right.prerelease))


def coerce(value):
    return value if isinstance(value, Version) else parse(value)


def compare(a, b):
    return _compare(coerce(a), coerce(b))
