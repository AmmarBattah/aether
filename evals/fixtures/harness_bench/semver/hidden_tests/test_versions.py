import unittest

from semverkit import InvalidVersion, Version, compare, parse

SPEC_ORDER = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.beta", "1.0.0-beta", "1.0.0-beta.2",
              "1.0.0-beta.11", "1.0.0-rc.1", "1.0.0"]


class ParseTests(unittest.TestCase):
    def test_core_fields(self):
        version = parse("1.22.333")
        self.assertEqual((version.major, version.minor, version.patch), (1, 22, 333))
        self.assertEqual((version.prerelease, version.build), ((), ()))

    def test_prerelease_identifier_types(self):
        self.assertEqual(parse("1.0.0-alpha.1.0a.-1").prerelease, ("alpha", 1, "0a", "-1"))

    def test_build_identifiers_are_strings(self):
        self.assertEqual(parse("1.0.0+001.sha.5").build, ("001", "sha", "5"))

    def test_canonical_text_round_trips(self):
        for text in ("0.0.0", "1.2.3-rc.1+build.7", "10.20.30+meta", "1.0.0-0.3.7"):
            with self.subTest(text=text):
                self.assertEqual(str(parse(text)), text)

    def test_huge_numbers(self):
        big = parse("12345678901234567890.0.0")
        self.assertEqual(big.major, 12345678901234567890)
        self.assertEqual(compare(big, "12345678901234567889.99.99"), 1)

    def test_rejects_leading_zeros(self):
        for text in ("01.0.0", "1.01.0", "1.0.01", "1.0.0-01", "1.0.0-alpha.007"):
            with self.subTest(text=text), self.assertRaises(InvalidVersion):
                parse(text)

    def test_allows_zero_like_identifiers_where_valid(self):
        self.assertEqual(parse("1.0.0-0").prerelease, (0,))
        self.assertEqual(parse("1.0.0-0a").prerelease, ("0a",))
        self.assertEqual(parse("1.0.0+01").build, ("01",))

    def test_rejects_malformed_text(self):
        for text in ("1.0", "1.0.0.0", "v1.0.0", " 1.0.0", "1.0.0 ", "1.0.0-", "1.0.0+", "1.0.0-a..b",
                     "1.0.0+a..b", "1.0.0-a_b", "", "1.0.0-+b", "-1.0.0", "1.0.0-α"):
            with self.subTest(text=text), self.assertRaises(InvalidVersion):
                parse(text)

    def test_rejects_non_ascii_digits(self):
        for text in ("١.0.0", "1.０.0", "1.0.0-١"):
            with self.subTest(text=text), self.assertRaises(InvalidVersion):
                parse(text)

    def test_rejects_non_strings(self):
        for value in (None, 1, b"1.0.0"):
            with self.subTest(value=value), self.assertRaises(InvalidVersion):
                parse(value)

    def test_invalid_version_is_value_error(self):
        self.assertTrue(issubclass(InvalidVersion, ValueError))


class PrecedenceTests(unittest.TestCase):
    def test_semver_specification_order(self):
        for lower, higher in zip(SPEC_ORDER, SPEC_ORDER[1:]):
            with self.subTest(lower=lower, higher=higher):
                self.assertEqual(compare(lower, higher), -1)
                self.assertEqual(compare(higher, lower), 1)

    def test_sorting_versions(self):
        shuffled = [SPEC_ORDER[i] for i in (5, 2, 7, 0, 3, 6, 1, 4)]
        self.assertEqual([str(v) for v in sorted(parse(text) for text in shuffled)], SPEC_ORDER)

    def test_numeric_core_comparison(self):
        self.assertEqual(compare("1.10.0", "1.9.0"), 1)
        self.assertEqual(compare("2.0.0", "1.99.99"), 1)
        self.assertEqual(compare("0.0.9", "0.0.10"), -1)

    def test_numeric_prerelease_identifiers_compare_numerically(self):
        self.assertEqual(compare("1.0.0-2", "1.0.0-10"), -1)
        self.assertEqual(compare("1.0.0-alpha.9", "1.0.0-alpha.10"), -1)

    def test_numeric_identifiers_are_lower_than_alphanumeric(self):
        self.assertEqual(compare("1.0.0-999", "1.0.0-a"), -1)
        self.assertEqual(compare("1.0.0-alpha.1", "1.0.0-alpha.a"), -1)
        self.assertEqual(compare("1.0.0-1", "1.0.0-0a"), -1)

    def test_alphanumeric_identifiers_use_ascii_order(self):
        self.assertEqual(compare("1.0.0-Beta", "1.0.0-alpha"), -1)
        self.assertEqual(compare("1.0.0-alpha-2", "1.0.0-alpha2"), -1)

    def test_compare_returns_exact_signs(self):
        self.assertEqual(compare("1.0.0", "3.0.0"), -1)
        self.assertEqual(compare("3.0.0", "1.0.0"), 1)
        self.assertEqual(compare("1.0.0", "1.0.0"), 0)

    def test_build_metadata_is_ignored(self):
        self.assertEqual(compare("1.0.0+a", "1.0.0+b"), 0)
        self.assertEqual(parse("1.0.0+a"), parse("1.0.0+b"))
        self.assertEqual(len({parse("1.0.0+a"), parse("1.0.0+b"), parse("1.0.0")}), 1)
        self.assertEqual(compare("1.0.0-rc.1+x", "1.0.0-rc.1"), 0)

    def test_rich_comparisons(self):
        low, high = parse("1.0.0-rc.1"), parse("1.0.0")
        self.assertTrue(low < high and low <= high and high > low and high >= low and low != high)
        self.assertFalse(low == high)

    def test_mixed_type_comparisons(self):
        version = parse("1.0.0")
        self.assertFalse(version == "1.0.0")
        self.assertTrue(version != "1.0.0")
        with self.assertRaises(TypeError):
            version < "2.0.0"

    def test_compare_accepts_version_objects(self):
        self.assertEqual(compare(parse("1.0.0"), "1.0.1"), -1)
        self.assertIsInstance(parse("1.0.0"), Version)

    def test_compare_rejects_invalid_text(self):
        with self.assertRaises(InvalidVersion):
            compare("1.0", "1.0.0")


if __name__ == "__main__":
    unittest.main()
