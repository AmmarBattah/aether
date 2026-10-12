import copy
import unittest

from semverkit import InvalidRange, InvalidVersion, ResolutionError, resolve


class ResolveTests(unittest.TestCase):
    def test_highest_matching_version(self):
        registry = {"a": {"1.0.0": {}, "1.1.0": {}, "2.0.0": {}}}
        self.assertEqual(resolve(registry, {"a": "^1"}), {"a": "1.1.0"})

    def test_transitive_dependencies_skip_prereleases(self):
        registry = {"a": {"1.0.0": {"b": "^2"}}, "b": {"1.5.0": {}, "2.1.0": {}, "2.2.0-beta": {}}}
        self.assertEqual(resolve(registry, {"a": "*"}), {"a": "1.0.0", "b": "2.1.0"})

    def test_only_reached_packages_are_returned(self):
        registry = {"a": {"1.0.0": {}}, "unused": {"1.0.0": {}}}
        self.assertEqual(resolve(registry, {"a": "1.0.0"}), {"a": "1.0.0"})

    def test_backtracks_to_an_older_version(self):
        registry = {
            "a": {"2.0.0": {"c": "^2"}, "1.0.0": {"c": "^1"}},
            "b": {"1.0.0": {"c": "^1"}},
            "c": {"1.0.0": {}, "1.4.0": {}, "2.0.0": {}},
        }
        self.assertEqual(resolve(registry, {"a": "*", "b": "*"}), {"a": "1.0.0", "b": "1.0.0", "c": "1.4.0"})

    def test_intersects_constraints_from_several_dependents(self):
        registry = {
            "a": {"1.0.0": {"c": ">=1.2"}},
            "b": {"1.0.0": {"c": "<1.4"}},
            "c": {"1.1.0": {}, "1.3.0": {}, "1.4.0": {}, "1.5.0": {}},
        }
        self.assertEqual(resolve(registry, {"a": "*", "b": "*"})["c"], "1.3.0")

    def test_new_dependency_must_accept_decided_package(self):
        registry = {"a": {"1.0.0": {}}, "b": {"2.0.0": {"a": "<1.0.0"}, "1.0.0": {}}}
        self.assertEqual(resolve(registry, {"a": "*", "b": "*"}), {"a": "1.0.0", "b": "1.0.0"})

    def test_decisions_follow_name_order_not_dict_order(self):
        registry = {"b": {"2.0.0": {}, "1.0.0": {}}, "a": {"2.0.0": {"b": "<2"}, "1.0.0": {}}}
        self.assertEqual(resolve(registry, {"b": "*", "a": "*"}), {"a": "2.0.0", "b": "1.0.0"})

    def test_deep_backtracking_drops_undone_constraints(self):
        registry = {
            "a": {"2.0.0": {"b": "^2"}, "1.0.0": {"b": "^1"}},
            "b": {"2.0.0": {"c": "^2"}, "1.0.0": {"c": "^1"}},
            "c": {"1.0.0": {}},
        }
        self.assertEqual(resolve(registry, {"a": "*"}), {"a": "1.0.0", "b": "1.0.0", "c": "1.0.0"})

    def test_cycles_and_self_dependencies(self):
        registry = {"a": {"2.0.0": {"a": "^1"}, "1.0.0": {"b": "^1"}}, "b": {"1.0.0": {"a": "^1"}}}
        self.assertEqual(resolve(registry, {"a": "*"}), {"a": "1.0.0", "b": "1.0.0"})

    def test_prerelease_can_be_opted_into(self):
        registry = {"a": {"1.0.0-beta.2": {}, "0.9.0": {}}}
        self.assertEqual(resolve(registry, {"a": ">=1.0.0-beta"}), {"a": "1.0.0-beta.2"})

    def test_returns_registry_spelling(self):
        registry = {"a": {"1.0.0+build.5": {}}}
        self.assertEqual(resolve(registry, {"a": "1.0.0"}), {"a": "1.0.0+build.5"})

    def test_unsatisfiable_requirement(self):
        with self.assertRaises(ResolutionError):
            resolve({"a": {"1.0.0": {}}}, {"a": "^2"})

    def test_missing_package(self):
        with self.assertRaises(ResolutionError):
            resolve({"a": {"1.0.0": {"ghost": "*"}}}, {"a": "*"})

    def test_resolution_error_is_not_value_error(self):
        self.assertFalse(issubclass(ResolutionError, ValueError))

    def test_empty_requirements(self):
        self.assertEqual(resolve({"a": {"1.0.0": {}}}, {}), {})

    def test_validates_unreached_ranges(self):
        registry = {"a": {"1.0.0": {}}, "z": {"1.0.0": {"a": ">>1"}}}
        with self.assertRaises(InvalidRange):
            resolve(registry, {"a": "*"})

    def test_validates_unreached_versions(self):
        with self.assertRaises(InvalidVersion):
            resolve({"a": {"1.0.0": {}}, "z": {"1.0": {}}}, {"a": "*"})

    def test_rejects_equal_precedence_keys(self):
        with self.assertRaises(InvalidVersion):
            resolve({"a": {"1.0.0+a": {}, "1.0.0+b": {}}}, {"a": "*"})

    def test_validates_requirement_ranges(self):
        with self.assertRaises(InvalidRange):
            resolve({"a": {"1.0.0": {}}}, {"a": "^"})

    def test_rejects_empty_names(self):
        with self.assertRaises(ValueError):
            resolve({"": {"1.0.0": {}}}, {})

    def test_inputs_are_not_mutated(self):
        registry = {"a": {"2.0.0": {"b": "^2"}, "1.0.0": {"b": "^1"}}, "b": {"1.0.0": {}}}
        requirements = {"a": "*"}
        before = copy.deepcopy((registry, requirements))
        self.assertEqual(resolve(registry, requirements), {"a": "1.0.0", "b": "1.0.0"})
        self.assertEqual((registry, requirements), before)


if __name__ == "__main__":
    unittest.main()
