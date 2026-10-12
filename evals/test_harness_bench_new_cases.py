"""Validates the cases added for the lower-tier matrix: the stated starter
score, a full reference pass, repeatable grading, and no credit for an empty
or constant implementation."""
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench_cases as hb
from harness_bench_assets import fixture_digest
from harness_bench_test_support import TEST_SCRATCH

_FIXTURES = Path(__file__).parent / "fixtures" / "harness_bench"
# case -> (hidden checks, checks the unmodified starter passes, public class every check goes through)
NEW_CASES = {"matchbook": (119, 23, "Engine"), "gridcalc": (154, 5, "Sheet")}
# Frozen before the counted run: sha256 over each case's starter and hidden-check hashes (fixture_digest).
FROZEN = {"matchbook": "7f719f2d3e0b7c98228f9bdf9a922aea93b605cbec9cb3b46535fe66ab8d630a",
          "gridcalc": "df35116f6492541fbdc8078c26355647483ff321332b91bb7ab3c9c975dd4758"}
_STUB = '''from .errors import *


class {public_class}:
    def __init__(self, *args, **kwargs):
        pass

    def __getattr__(self, name):
        def method(*args, **kwargs):
            {body}
        return method
'''
_CONSTANT_BODIES = {
    "returns None": "return None",
    "returns zero": "return 0",
    "returns an empty list": "return []",
    "returns an empty dict": "return {}",
    "always raises": "raise ValueError('constant implementation')",
    "echoes its first argument": "return args[0] if args else None",
}


class NewCaseValidationTests(unittest.TestCase):
    def _workspace(self, case_id, source="starter"):
        root = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix=f"case-{case_id}-"))
        self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        workspace = root / "workspace"
        hb.prepare(case_id, workspace)
        if source == "reference":
            shutil.rmtree(workspace / case_id)
            shutil.copytree(_FIXTURES / case_id / "reference" / case_id, workspace / case_id)
        return workspace

    def test_new_cases_are_registered_with_enough_independent_checks(self):
        registered = {case["id"] for case in hb.cases()}
        for case_id, (total, _, _) in NEW_CASES.items():
            self.assertIn(case_id, registered)
            self.assertGreaterEqual(total, 40, f"{case_id} needs 40 or more hidden checks")

    def test_starter_fails_the_stated_number_of_checks(self):
        for case_id, (total, starter_passed, _) in NEW_CASES.items():
            report = hb.grade(case_id, self._workspace(case_id))
            self.assertEqual((report["passed"], report["total"]), (starter_passed, total), case_id)
            self.assertGreaterEqual(total - starter_passed, total * 0.4, f"{case_id} starter leaves too little to do")

    def test_reference_passes_every_check(self):
        for case_id, (total, _, _) in NEW_CASES.items():
            report = hb.grade(case_id, self._workspace(case_id, "reference"))
            self.assertEqual(report, {"passed": total, "total": total, "errors": []}, case_id)

    def test_three_repeated_gradings_agree(self):
        for case_id in NEW_CASES:
            for source in ("starter", "reference"):
                workspace = self._workspace(case_id, source)
                reports = [hb.grade(case_id, workspace) for _ in range(3)]
                self.assertEqual(reports[0], reports[1], f"{case_id} {source}")
                self.assertEqual(reports[0], reports[2], f"{case_id} {source}")

    def test_an_empty_or_constant_implementation_passes_nothing(self):
        for case_id, (total, _, public_class) in NEW_CASES.items():
            for label, body in _CONSTANT_BODIES.items():
                workspace = self._workspace(case_id)
                package = workspace / case_id
                for module in package.glob("*.py"):
                    if module.name != "errors.py":
                        module.unlink()
                (package / "__init__.py").write_text(_STUB.format(public_class=public_class, body=body))
                report = hb.grade(case_id, workspace)
                self.assertEqual((report["passed"], report["total"]), (0, total), f"{case_id}: {label}")

    def test_hidden_checks_and_reference_never_reach_the_solver_workspace(self):
        for case_id in NEW_CASES:
            workspace = self._workspace(case_id)
            names = {path.name for path in workspace.rglob("*")}
            self.assertNotIn("hidden_tests", names)
            self.assertNotIn("reference", names)
            hidden = {path.name for path in (_FIXTURES / case_id / "hidden_tests").glob("test_*.py")}
            self.assertTrue(hidden and not hidden & names, f"{case_id} leaked {hidden & names}")

    def test_fixtures_are_frozen(self):
        """A changed contract, starter or hidden check must be a new, re-validated case version."""
        self.assertEqual(set(FROZEN), set(NEW_CASES))
        for case_id, digest in FROZEN.items():
            self.assertEqual(fixture_digest(case_id), digest, f"{case_id} changed after it was frozen")


if __name__ == "__main__":
    unittest.main()
