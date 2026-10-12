import unittest

from semverkit import compare, max_satisfying, parse, resolve, satisfies


class SmokeTests(unittest.TestCase):
    def test_parse_core(self):
        self.assertEqual(parse("1.2.3").minor, 2)

    def test_compare_cores(self):
        self.assertEqual(compare("1.2.3", "1.10.0"), -1)

    def test_caret_major(self):
        self.assertTrue(satisfies("1.4.0", "^1.2.3"))
        self.assertFalse(satisfies("2.0.0", "^1.2.3"))

    def test_max_satisfying(self):
        self.assertEqual(max_satisfying(["1.0.0", "1.2.0", "2.0.0"], "~1.0"), "1.0.0")

    def test_simple_resolution(self):
        registry = {"app": {"1.0.0": {"lib": "^1"}}, "lib": {"1.0.0": {}, "1.1.0": {}}}
        self.assertEqual(resolve(registry, {"app": "*"}), {"app": "1.0.0", "lib": "1.1.0"})
