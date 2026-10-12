# Semantic versions, ranges, and dependency resolution

Implement the stdlib-only `semverkit` package. Functions are deterministic and
never mutate their arguments. `InvalidVersion` and `InvalidRange` subclass
`ValueError`. `ResolutionError` subclasses `Exception` but **not** `ValueError`.

## Versions

`parse(text) -> Version` accepts exactly SemVer 2.0.0
`MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]` and raises `InvalidVersion` otherwise
(including for non-strings).

- Core numbers are ASCII digits `0-9` without leading zeros (`0` is valid, `01`
  is not) and may be arbitrarily large.
- `PRERELEASE` and `BUILD` are one or more dot-separated, nonempty identifiers
  of ASCII `[0-9A-Za-z-]`. A purely numeric prerelease identifier must not have
  a leading zero and is numeric; any other prerelease identifier (for example
  `0a` or `-1`) is alphanumeric. Build identifiers may have leading zeros and
  are always strings.
- No surrounding whitespace, `v` prefix, or non-ASCII characters.

A `Version` has `major`, `minor`, `patch` (`int`), `prerelease` (tuple of `int`
for numeric and `str` for alphanumeric identifiers, empty when absent) and
`build` (tuple of `str`). `str(version)` is its canonical text.

Precedence follows SemVer 2.0.0: compare major, minor, and patch numerically;
a version with a prerelease is lower than the same core without one; compare
prerelease identifiers left to right, where numeric identifiers compare
numerically and are lower than alphanumeric identifiers, alphanumeric
identifiers compare by ASCII order (case-sensitive), and a shorter prerelease
list is lower when all preceding identifiers are equal. Build metadata never
affects precedence.

- `compare(a, b) -> int` accepts `Version` objects or version strings and
  returns exactly `-1`, `0`, or `1`.
- `Version` supports `==`, `!=`, `<`, `<=`, `>`, `>=` consistently with
  `compare`. Equality ignores build metadata, and `hash` agrees with equality.
  Comparing a `Version` with a non-`Version` is unequal for `==`/`!=` and
  raises `TypeError` for ordering.

## Ranges

`Range(text)` parses a range or raises `InvalidRange` (including for
non-strings). `Range.test(version)` accepts a `Version` or version string.
`satisfies(version, range_text) -> bool` is a convenience wrapper.

```text
range-set   := alternative ( "||" alternative )*
alternative := hyphen | simple ( whitespace simple )*
hyphen      := partial whitespace "-" whitespace partial
simple      := [ op ] [ whitespace ] partial
op          := "<" | "<=" | ">" | ">=" | "=" | "~" | "^"
partial     := xr [ "." xr [ "." xr [ "-" prerelease ] [ "+" build ] ] ]
xr          := "x" | "X" | "*" | number
```

- Whitespace is any run of ASCII spaces. It is optional around `||`, ignored
  at both ends of the text and of each alternative, and allowed between an
  operator and its partial (`>= 1.2.3`). An alternative never contains a bare
  operator without a partial.
- A text that is empty or only whitespace is equivalent to `*`. An empty
  alternative inside a nonempty text (`"1.2.3 ||"`, `"|| 1.2.3"`,
  `"1 |||| 2"`) is invalid.
- Numbers have no leading zeros. Once a wildcard or missing part appears,
  every later part must also be a wildcard or missing (`1.x.3` is invalid).
  Prerelease and build are only allowed when all three parts are numbers.
  Build metadata in a range is ignored.

Each simple comparator desugars as follows (`X` is a wildcard or missing part):

| Input | Meaning |
| --- | --- |
| `*`, `x`, `>=*`, `<=*`, `=*`, `~*`, `^*` | any version (no comparator) |
| `<*`, `>*` | nothing |
| `1.2.3`, `=1.2.3` | exactly `1.2.3` (prerelease included, build ignored) |
| `1`, `1.X`, `=1` | `>=1.0.0 <2.0.0` |
| `1.2`, `1.2.X` | `>=1.2.0 <1.3.0` |
| `>1.2.3` / `>1.2` / `>1` | `>1.2.3` / `>=1.3.0` / `>=2.0.0` |
| `>=1.2.3` / `>=1.2` / `>=1` | `>=1.2.3` / `>=1.2.0` / `>=1.0.0` |
| `<1.2.3` / `<1.2` / `<1` | `<1.2.3` / `<1.2.0` / `<1.0.0` |
| `<=1.2.3` / `<=1.2` / `<=1` | `<=1.2.3` / `<1.3.0` / `<2.0.0` |
| `~1.2.3` / `~1.2` | `>=1.2.3` / `>=1.2.0`, each `<1.3.0` |
| `~1` | `>=1.0.0 <2.0.0` |
| `^1.2.3`, `^1.2`, `^1` | `>=1.2.3` / `>=1.2.0` / `>=1.0.0`, each `<2.0.0` |
| `^0.2.3`, `^0.2` | `>=0.2.3` / `>=0.2.0`, each `<0.3.0` |
| `^0.0.3` | `>=0.0.3 <0.0.4` |
| `^0.0`, `^0` | `>=0.0.0 <0.1.0`, `>=0.0.0 <1.0.0` |

A lower bound keeps the partial's prerelease (`~1.2.3-beta.2` is
`>=1.2.3-beta.2 <1.3.0`; `^0.0.3-beta` is `>=0.0.3-beta <0.0.4`). Upper bounds
created by desugaring never have a prerelease.

Hyphen ranges `A - B`: the lower bound is `>=A` with missing parts as zero
(none if `A` is a wildcard). The upper bound is `<=B` when `B` is complete,
`<B.(minor+1).0` when `B` has major and minor only, `<(major+1).0.0` when `B`
has a major only, and none when `B` is a wildcard. `1.2 - 2.3.4` is
`>=1.2.0 <=2.3.4`; `1.2.3 - 2.3` is `>=1.2.3 <2.4.0`.

A version satisfies a range when it satisfies any alternative. It satisfies an
alternative when it satisfies every comparator **and**, if the version has a
prerelease, at least one comparator of that same alternative has a version
with a prerelease and the same `major.minor.patch`. So `*` and the empty range
match only releases, `2.0.0-alpha` does not satisfy `^1.2.3`, `1.2.3-beta`
does not satisfy `<1.2.3`, and `1.2.3-beta.3` satisfies `^1.2.3-beta.2`.

- `max_satisfying(versions, range_text) -> str | None` returns the highest
  satisfying entry of a list of version strings, exactly as given; equal
  precedence keeps the earliest entry. Every entry is parsed first, so any
  invalid entry raises `InvalidVersion`. `None` means no match.
- `min_satisfying(versions, range_text)` is the same for the lowest.

## Resolution

`resolve(registry, requirements) -> dict[str, str]`.

- `registry` maps a package name to a dict that maps version strings to a
  dependency dict (`{dependency_name: range_text}`). `requirements` maps
  package names to range texts.
- Names are nonempty strings. Validate everything before searching: every
  version key and every range, including packages that are never reached,
  raising `InvalidVersion`, `InvalidRange`, or `ValueError` for other
  malformed input. Two version keys of the same package with equal precedence
  (e.g. `1.0.0+a` and `1.0.0+b`) raise `InvalidVersion`.
- Search deterministically. A package is *required* if it appears in
  `requirements` or in the dependencies of a currently decided version. Always
  decide the required, undecided package whose name is smallest in Python
  string order. Its candidates are its registry versions that satisfy every
  current constraint on it (the requirement and every decided version's
  dependency on it), tried from highest to lowest precedence. Deciding a
  candidate also requires that each of its own dependencies on an
  already-decided package, including itself, is satisfied. If no candidate
  leads to a complete solution, undo it and backtrack to the previous decision
  (constraints introduced by undone decisions disappear with them). A required
  package missing from the registry has no candidates.
- Return `{name: version_string}` for exactly the decided packages, using the
  registry's version strings. If the search fails, raise `ResolutionError`.
