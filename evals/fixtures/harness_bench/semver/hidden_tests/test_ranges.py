import unittest

from semverkit import InvalidRange, InvalidVersion, Range, max_satisfying, min_satisfying, parse, satisfies


class RangeCase(unittest.TestCase):
    def check(self, range_text, inside, outside):
        for version in inside:
            with self.subTest(range=range_text, version=version):
                self.assertTrue(satisfies(version, range_text))
        for version in outside:
            with self.subTest(range=range_text, version=version):
                self.assertFalse(satisfies(version, range_text))


class CaretTests(RangeCase):
    def test_caret_major(self):
        self.check("^1.2.3", ["1.2.3", "1.9.9", "1.2.3+b"], ["1.2.2", "2.0.0", "0.9.0"])

    def test_caret_zero_minor(self):
        self.check("^0.2.3", ["0.2.3", "0.2.9"], ["0.3.0", "0.2.2", "1.0.0"])

    def test_caret_zero_zero_patch(self):
        self.check("^0.0.3", ["0.0.3"], ["0.0.4", "0.0.2", "0.1.0"])
        self.check("^0.0.0", ["0.0.0"], ["0.0.1"])

    def test_caret_partials(self):
        self.check("^1.2", ["1.2.0", "1.99.0"], ["1.1.9", "2.0.0"])
        self.check("^0.2", ["0.2.0", "0.2.7"], ["0.3.0", "0.1.9"])
        self.check("^0.0", ["0.0.0", "0.0.9"], ["0.1.0"])
        self.check("^0", ["0.0.0", "0.9.9"], ["1.0.0"])
        self.check("^1.x", ["1.0.0", "1.5.0"], ["2.0.0", "0.9.9"])

    def test_caret_prerelease_lower_bound(self):
        self.check("^1.2.3-beta.2", ["1.2.3-beta.2", "1.2.3-beta.10", "1.2.3", "1.9.0"],
                   ["1.2.3-beta.1", "1.2.4-beta", "2.0.0"])
        self.check("^0.0.3-beta", ["0.0.3-beta", "0.0.3-pr.2", "0.0.3"], ["0.0.4", "0.0.4-alpha"])


class TildeTests(RangeCase):
    def test_tilde_full(self):
        self.check("~1.2.3", ["1.2.3", "1.2.9"], ["1.3.0", "1.2.2"])
        self.check("~0.2.3", ["0.2.3", "0.2.4"], ["0.3.0"])

    def test_tilde_partials(self):
        self.check("~1.2", ["1.2.0", "1.2.9"], ["1.3.0", "1.1.0"])
        self.check("~1", ["1.0.0", "1.9.9"], ["2.0.0", "0.9.0"])

    def test_tilde_prerelease(self):
        self.check("~1.2.3-beta.2", ["1.2.3-beta.4", "1.2.3", "1.2.5"], ["1.2.3-beta.1", "1.2.4-beta.5", "1.3.0"])


class XRangeTests(RangeCase):
    def test_wildcards_match_every_release_only(self):
        for text in ("*", "x", "X", "", "   ", ">=*", "<=*", "=*", "~*", "^*"):
            self.check(text, ["0.0.0", "9.9.9"], ["1.0.0-rc.1"])

    def test_impossible_wildcards(self):
        for text in ("<*", ">*"):
            self.check(text, [], ["0.0.0", "1.0.0"])

    def test_partial_versions(self):
        self.check("1", ["1.0.0", "1.9.9"], ["2.0.0", "0.9.9"])
        self.check("1.x", ["1.0.0", "1.5.5"], ["2.0.0"])
        self.check("1.2.*", ["1.2.0", "1.2.7"], ["1.3.0"])
        self.check("=1.2", ["1.2.0"], ["1.3.0"])

    def test_exact_versions(self):
        self.check("1.2.3", ["1.2.3", "1.2.3+build"], ["1.2.4", "1.2.3-rc.1"])
        self.check("=1.2.3-rc.1", ["1.2.3-rc.1", "1.2.3-rc.1+b"], ["1.2.3", "1.2.3-rc.2"])


class OperatorTests(RangeCase):
    def test_greater_than_partials(self):
        self.check(">1.2", ["1.3.0"], ["1.2.9", "1.2.0"])
        self.check(">1", ["2.0.0"], ["1.9.9"])
        self.check(">1.2.3", ["1.2.4"], ["1.2.3"])

    def test_greater_or_equal_partials(self):
        self.check(">=1.2", ["1.2.0"], ["1.1.9"])
        self.check(">=1", ["1.0.0"], ["0.9.9"])

    def test_less_than_partials(self):
        self.check("<1.2", ["1.1.9"], ["1.2.0"])
        self.check("<1", ["0.9.9"], ["1.0.0"])

    def test_less_or_equal_partials(self):
        self.check("<=1.2", ["1.2.9"], ["1.3.0"])
        self.check("<=1", ["1.9.9"], ["2.0.0"])
        self.check("<=1.2.3", ["1.2.3"], ["1.2.4"])

    def test_intersections_and_whitespace(self):
        self.check(">= 1.2.3   < 2.0.0", ["1.2.3", "1.9.0"], ["2.0.0", "1.2.2"])
        self.check("  ^1.2.3  ", ["1.5.0"], ["2.0.0"])
        self.check("> 1.0.0 <= 1.2.0", ["1.2.0", "1.0.1"], ["1.0.0", "1.2.1"])


class HyphenTests(RangeCase):
    def test_full_hyphen(self):
        self.check("1.2.3 - 2.3.4", ["1.2.3", "2.3.4"], ["1.2.2", "2.3.5"])

    def test_partial_lower_bound(self):
        self.check("1.2 - 2.3.4", ["1.2.0"], ["1.1.9", "2.3.5"])

    def test_partial_upper_bounds(self):
        self.check("1.2.3 - 2.3", ["2.3.9"], ["2.4.0"])
        self.check("1.2.3 - 2", ["2.9.9"], ["3.0.0"])

    def test_wildcard_ends(self):
        self.check("* - 2.0.0", ["0.0.0", "2.0.0"], ["2.0.1"])
        self.check("1.0.0 - *", ["1.0.0", "99.0.0"], ["0.9.9"])

    def test_prerelease_hyphen(self):
        self.check("1.2.3-alpha - 1.2.3", ["1.2.3-beta", "1.2.3"], ["1.2.4"])


class AlternativeTests(RangeCase):
    def test_alternatives(self):
        self.check("<1.0.0 || >=2.0.0", ["0.5.0", "2.0.0"], ["1.5.0"])
        self.check("1.2.3||2.0.0", ["1.2.3", "2.0.0"], ["1.2.4"])

    def test_prerelease_gate_is_per_alternative(self):
        self.check("<1.0.0 || >=1.2.3-alpha <1.3.0", ["1.2.3-beta"], ["1.2.4-beta"])
        self.check(">=1.2.3-alpha <1.3.0 || 2.x", ["1.2.3-rc.1"], ["2.0.0-rc.1"])


class PrereleaseGateTests(RangeCase):
    def test_releases_satisfy_prerelease_bounds(self):
        self.check(">=1.2.3-alpha", ["1.2.3", "2.0.0"], [])

    def test_prerelease_needs_matching_tuple(self):
        self.check("<1.2.3", ["1.2.2"], ["1.2.3-beta", "1.2.2-beta"])
        self.check("^1.2.3", [], ["2.0.0-alpha", "1.5.0-rc.1"])
        self.check(">1.2.3-beta.1", ["1.2.3-beta.2", "1.2.4"], ["1.2.4-alpha"])


class InvalidRangeTests(unittest.TestCase):
    def test_rejects_malformed_ranges(self):
        for text in (">=", "1.2.3 -", "- 1.2.3", "1.2.3 ||", "|| 1.2.3", "1 |||| 2", "~>1.2", ">>1",
                     "1.2.3-", "01.2", "1.2.3 - 2.0.0 - 3.0.0", "1.2-beta", "1.x.3", "a.b.c", "1.2.3 | 2",
                     "1.2.3\t<2", "^ ", "1.2.3.4", "> =1"):
            with self.subTest(text=text), self.assertRaises(InvalidRange):
                Range(text)

    def test_rejects_non_strings(self):
        with self.assertRaises(InvalidRange):
            Range(None)

    def test_invalid_range_is_value_error(self):
        self.assertTrue(issubclass(InvalidRange, ValueError))

    def test_satisfies_rejects_invalid_version(self):
        with self.assertRaises(InvalidVersion):
            satisfies("1.2", "*")


class SelectionTests(unittest.TestCase):
    VERSIONS = ["1.2.3", "1.4.0", "2.0.0-rc.1", "1.4.0+meta", "2.0.0", "0.9.0"]

    def test_max_satisfying(self):
        self.assertEqual(max_satisfying(self.VERSIONS, "^1.0.0"), "1.4.0")
        self.assertEqual(max_satisfying(self.VERSIONS, "*"), "2.0.0")
        self.assertEqual(max_satisfying(self.VERSIONS, ">=2.0.0-rc.0 <2.0.0"), "2.0.0-rc.1")

    def test_min_satisfying(self):
        self.assertEqual(min_satisfying(self.VERSIONS, "^1.3"), "1.4.0")
        self.assertEqual(min_satisfying(list(reversed(self.VERSIONS)), "^1.3"), "1.4.0+meta")
        self.assertEqual(min_satisfying(self.VERSIONS, "*"), "0.9.0")

    def test_equal_precedence_keeps_first_entry(self):
        self.assertEqual(max_satisfying(["1.0.0+b", "1.0.0+a"], "1.x"), "1.0.0+b")

    def test_no_match_is_none(self):
        self.assertIsNone(max_satisfying(self.VERSIONS, ">=3"))
        self.assertIsNone(min_satisfying([], "*"))

    def test_every_entry_is_validated(self):
        with self.assertRaises(InvalidVersion):
            max_satisfying(["2.0.0", "1.0"], "*")

    def test_range_test_accepts_versions_and_strings(self):
        selector = Range("^1.0.0")
        self.assertTrue(selector.test(parse("1.5.0")))
        self.assertTrue(selector.test("1.5.0"))


if __name__ == "__main__":
    unittest.main()
