import re

from .errors import InvalidRange
from .version import Version, coerce, parse, prerelease_identifiers

_COMPARATOR = re.compile(r"(<=|>=|<|>|=|~|\^)?(x|X|\*|\d+)(?:\.(x|X|\*|\d+))?(?:\.(x|X|\*|\d+)(?:-([0-9A-Za-z.-]+))?)?(?:\+[0-9A-Za-z.-]+)?")


def _number(raw):
    return None if raw is None or raw in ("x", "X", "*") else int(raw)


def _parse_comparator(token):
    match = _COMPARATOR.fullmatch(token)
    if match is None:
        raise InvalidRange(f"invalid comparator: {token!r}")
    op, major, minor, patch, prerelease = match.groups()
    parts = [_number(major), _number(minor), _number(patch)]
    if parts[1] is None:
        parts[2] = None
    return op or "", parts, prerelease_identifiers(prerelease) if prerelease else ()


def _floor(parts, prerelease=()):
    major, minor, patch = parts
    return Version(major, minor or 0, patch or 0, prerelease)


def _desugar(op, parts, prerelease):
    major, minor, patch = parts
    if major is None:
        return []
    if op in ("", "="):
        if patch is not None:
            return [("=", _floor(parts, prerelease))]
        upper = Version(major + 1, 0, 0) if minor is None else Version(major, minor + 1, 0)
        return [(">=", _floor(parts)), ("<", upper)]
    if op in ("<", "<=", ">", ">="):
        return [(op, _floor(parts, prerelease))]
    if op == "~":
        return [(">=", _floor(parts, prerelease)), ("<", Version(major, (minor or 0) + 1, 0))]
    return [(">=", _floor(parts, prerelease)), ("<", Version(major + 1, 0, 0))]


def _parse_alternative(text):
    tokens = text.split()
    if len(tokens) == 3 and tokens[1] == "-":
        _, lower, lower_pre = _parse_comparator(tokens[0])
        _, upper, upper_pre = _parse_comparator(tokens[2])
        comparators = []
        if lower[0] is not None:
            comparators.append((">=", _floor(lower, lower_pre)))
        if upper[0] is not None:
            comparators.append(("<=", _floor(upper, upper_pre)))
        return comparators
    comparators = []
    for token in tokens:
        comparators.extend(_desugar(*_parse_comparator(token)))
    return comparators


def _parse_set(text):
    return [_parse_alternative(part) for part in text.split("||")]


def _holds(op, version, bound):
    if op == "=":
        return version == bound
    if op == "<":
        return version < bound
    if op == "<=":
        return version <= bound
    if op == ">":
        return version > bound
    return version >= bound


class Range:
    def __init__(self, text):
        self._alternatives = _parse_set(text)
        self.text = text

    def test(self, version):
        version = coerce(version)
        return any(all(_holds(op, version, bound) for op, bound in alternative)
                   for alternative in self._alternatives)

    def __repr__(self):
        return f"Range({self.text!r})"


def satisfies(version, range_text):
    return Range(range_text).test(version)


def max_satisfying(versions, range_text):
    selector, best = Range(range_text), None
    for text in versions:
        try:
            version = parse(text)
        except ValueError:
            continue
        if selector.test(version) and (best is None or version >= best[0]):
            best = (version, text)
    return None if best is None else best[1]


def min_satisfying(versions, range_text):
    selector, best = Range(range_text), None
    for text in versions:
        try:
            version = parse(text)
        except ValueError:
            continue
        if selector.test(version) and (best is None or version <= best[0]):
            best = (version, text)
    return None if best is None else best[1]
