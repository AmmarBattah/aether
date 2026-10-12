import re

from .errors import InvalidRange, InvalidVersion
from .version import IDENTIFIERS, Version, coerce, parse, prerelease_identifiers

_OPERATORS = ("<=", ">=", "<", ">", "=", "~", "^")
_PART = r"(?:x|X|\*|0|[1-9][0-9]*)"
_PARTIAL = re.compile(rf"({_PART})(?:\.({_PART})(?:\.({_PART})(?:-({IDENTIFIERS}))?(?:\+({IDENTIFIERS}))?)?)?")
_HYPHEN = re.compile(r"(\S+) +- +(\S+)")
_NEVER = ("never", None)


def _parse_partial(text):
    match = _PARTIAL.fullmatch(text)
    if match is None:
        raise InvalidRange(f"invalid partial version: {text!r}")
    raw_parts, prerelease, build = match.groups()[:3], match.group(4), match.group(5)
    parts, wildcard = [], False
    for raw in raw_parts:
        if raw is None or raw in ("x", "X", "*"):
            wildcard = True
            parts.append(None)
        elif wildcard:
            raise InvalidRange(f"number after a wildcard: {text!r}")
        else:
            parts.append(int(raw))
    if (prerelease or build) and None in parts:
        raise InvalidRange(f"prerelease or build on a partial version: {text!r}")
    try:
        identifiers = prerelease_identifiers(prerelease) if prerelease else ()
    except InvalidVersion as error:
        raise InvalidRange(str(error)) from error
    return parts, identifiers


def _floor(parts, prerelease=()):
    major, minor, patch = parts
    return Version(major, minor or 0, patch or 0, prerelease if patch is not None else ())


def _bump(parts):
    major, minor, _ = parts
    return Version(major + 1, 0, 0) if minor is None else Version(major, minor + 1, 0)


def _desugar(op, parts, prerelease):
    major, minor, patch = parts
    if major is None:
        return [_NEVER] if op in ("<", ">") else []
    exact = patch is not None
    if op in ("", "="):
        return [("=", _floor(parts, prerelease))] if exact else [(">=", _floor(parts)), ("<", _bump(parts))]
    if op == ">":
        return [(">", _floor(parts, prerelease))] if exact else [(">=", _bump(parts))]
    if op == ">=":
        return [(">=", _floor(parts, prerelease))]
    if op == "<":
        return [("<", _floor(parts, prerelease))]
    if op == "<=":
        return [("<=", _floor(parts, prerelease))] if exact else [("<", _bump(parts))]
    if op == "~":
        return [(">=", _floor(parts, prerelease)), ("<", _bump(parts))]
    if major:
        upper = Version(major + 1, 0, 0)
    elif minor is None:
        upper = Version(1, 0, 0)
    elif minor:
        upper = Version(0, minor + 1, 0)
    elif patch is None:
        upper = Version(0, 1, 0)
    else:
        upper = Version(0, 0, patch + 1)
    return [(">=", _floor(parts, prerelease)), ("<", upper)]


def _parse_hyphen(lower_text, upper_text):
    lower, lower_pre = _parse_partial(lower_text)
    upper, upper_pre = _parse_partial(upper_text)
    comparators = []
    if lower[0] is not None:
        comparators.append((">=", _floor(lower, lower_pre)))
    if upper[0] is not None:
        comparators.append(("<=", _floor(upper, upper_pre)) if upper[2] is not None else ("<", _bump(upper)))
    return comparators


def _parse_alternative(text):
    hyphen = _HYPHEN.fullmatch(text)
    if hyphen:
        return _parse_hyphen(*hyphen.groups())
    tokens, comparators, index = text.split(" "), [], 0
    tokens = [token for token in tokens if token]
    while index < len(tokens):
        token = tokens[index]
        if token in _OPERATORS:
            if index + 1 == len(tokens):
                raise InvalidRange(f"operator without a version: {text!r}")
            op, body, index = token, tokens[index + 1], index + 2
        else:
            op = next((candidate for candidate in _OPERATORS if token.startswith(candidate)), "")
            body, index = token[len(op):], index + 1
        comparators.extend(_desugar(op, *_parse_partial(body)))
    return comparators


def _parse_set(text):
    if not isinstance(text, str):
        raise InvalidRange(f"range must be a string, got {type(text).__name__}")
    if text.strip(" ") == "":
        return [[]]
    if any(char != " " and char.isspace() for char in text) or not text.isascii():
        raise InvalidRange(f"unsupported characters in range: {text!r}")
    alternatives = []
    for part in text.split("||"):
        part = part.strip(" ")
        if not part:
            raise InvalidRange(f"empty alternative in range: {text!r}")
        alternatives.append(_parse_alternative(part))
    return alternatives


def _holds(op, version, bound):
    if op == "never":
        return False
    if op == "=":
        return version == bound
    if op == "<":
        return version < bound
    if op == "<=":
        return version <= bound
    if op == ">":
        return version > bound
    return version >= bound


def _alternative_holds(comparators, version):
    if not all(_holds(op, version, bound) for op, bound in comparators):
        return False
    if not version.prerelease:
        return True
    return any(bound is not None and bound.prerelease and bound.core == version.core
               for _, bound in comparators)


class Range:
    def __init__(self, text):
        self._alternatives = _parse_set(text)
        self.text = text

    def test(self, version):
        version = coerce(version)
        return any(_alternative_holds(alternative, version) for alternative in self._alternatives)

    def __repr__(self):
        return f"Range({self.text!r})"


def satisfies(version, range_text):
    return Range(range_text).test(version)


def _select(versions, range_text, better):
    selector = Range(range_text)
    parsed = [(parse(text), text) for text in versions]
    best = None
    for version, text in parsed:
        if selector.test(version) and (best is None or better(version, best[0])):
            best = (version, text)
    return None if best is None else best[1]


def max_satisfying(versions, range_text):
    return _select(versions, range_text, lambda candidate, best: candidate > best)


def min_satisfying(versions, range_text):
    return _select(versions, range_text, lambda candidate, best: candidate < best)
