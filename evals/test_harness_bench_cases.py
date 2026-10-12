"""Proves the harness_bench grader is honest: the buggy starter fails it,
the (never-copied-to-a-workspace) reference implementation passes it fully,
the visible starter tests pass on their own, prepare()/grade() behave per
their documented contract, and grading a hanging/missing/import-failing
candidate returns a failure report instead of hanging or crashing. Run with:

    python3 -m unittest discover -s evals -p test_harness_bench_cases.py
"""
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench_assets as assets
import harness_bench_cases as hb

_FIXTURES = Path(__file__).parent / "fixtures" / "harness_bench"
_SHORT_TIMEOUT = 5  # seconds; keeps the hang test fast without being flaky
_MIN_HIDDEN_TESTS = 20


def _case_ids():
    return [case["id"] for case in hb.cases()]


def _package_name(case_id):
    """The case's package directory name, read from its reference tree."""
    reference = _FIXTURES / case_id / "reference"
    packages = [item.name for item in reference.iterdir() if item.is_dir()]
    assert len(packages) == 1, f"{case_id} reference must hold exactly one package, got {packages}"
    return packages[0]


def _prepared_starter(test_case, case_id):
    tmp = tempfile.TemporaryDirectory()
    test_case.addCleanup(tmp.cleanup)
    workspace = Path(tmp.name)
    hb.prepare(case_id, workspace)
    return workspace


class CasesApiTests(unittest.TestCase):
    def test_multiple_failing_subtests_count_as_one_failed_test(self):
        class CandidateChecks(unittest.TestCase):
            def test_failure(self):
                for value in range(3):
                    with self.subTest(value=value):
                        self.fail("injected")

            def test_success(self):
                self.assertTrue(True)

        result = hb._GradingResult()
        unittest.defaultTestLoader.loadTestsFromTestCase(CandidateChecks).run(result)
        self.assertEqual(result.testsRun, 2)
        self.assertEqual(len(result.failures), 3)
        self.assertEqual(result.passed, 1)

    def test_cases_lists_every_fixture_with_a_nonempty_objective(self):
        listed = hb.cases()
        fixture_ids = {item.name for item in _FIXTURES.iterdir() if item.is_dir()}
        self.assertEqual({case["id"] for case in listed}, fixture_ids)
        for case in listed:
            self.assertIsInstance(case["objective"], str)
            self.assertGreater(len(case["objective"]), 40)

    def test_objectives_do_not_leak_fixture_repository_paths(self):
        for case in hb.cases():
            self.assertNotIn("evals/fixtures", case["objective"])
            self.assertNotIn("harness_bench", case["objective"])
            self.assertNotIn("hidden_tests", case["objective"])
            self.assertNotIn("/reference", case["objective"])

    def test_grade_rejects_an_unknown_case_id(self):
        with self.assertRaises(KeyError):
            hb.grade("no-such-case", Path("/nonexistent"))


class PrepareCopiesOnlyStarterTests(unittest.TestCase):
    def test_prepare_gives_every_case_a_contract_package_and_visible_tests(self):
        for case_id in _case_ids():
            workspace = _prepared_starter(self, case_id)
            package = _package_name(case_id)
            self.assertTrue((workspace / package).is_dir(), case_id)
            self.assertTrue((workspace / "CONTRACT.md").is_file(), case_id)
            self.assertTrue(list((workspace / "tests").glob("test_*.py")), case_id)
            self.assertFalse((workspace / "hidden_tests").exists(), case_id)
            self.assertFalse((workspace / "reference").exists(), case_id)

    def test_prepare_never_hands_out_the_reference_implementation(self):
        for case_id in _case_ids():
            workspace = _prepared_starter(self, case_id)
            package = _package_name(case_id)
            reference_modules = sorted((_FIXTURES / case_id / "reference" / package).glob("*.py"))
            self.assertTrue(reference_modules, case_id)
            # A reference may be split into more modules than the starter; an absent one was not handed out.
            differences = [module.name for module in reference_modules
                           if not (workspace / package / module.name).is_file()
                           or (workspace / package / module.name).read_text() != module.read_text()]
            self.assertTrue(differences, f"{case_id} starter is identical to its reference")

    def test_prepare_copies_starter_but_not_reference_or_hidden_tests(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            hb.prepare("ledger", workspace)
            self.assertTrue((workspace / "ledger" / "engine.py").is_file())
            self.assertTrue((workspace / "CONTRACT.md").is_file())
            self.assertTrue((workspace / "tests" / "test_ledger_basic.py").is_file())
            self.assertFalse((workspace / "hidden_tests").exists())
            reference_marker = (_FIXTURES / "ledger" / "reference" / "ledger" / "engine.py").read_text()
            self.assertNotEqual((workspace / "ledger" / "engine.py").read_text(), reference_marker)

    def test_prepare_does_not_copy_pycache_directories(self):
        starter = _FIXTURES / "ledger" / "starter"
        stray = starter / "ledger" / "__pycache__"
        stray.mkdir(exist_ok=True)
        (stray / "engine.cpython-000.pyc").write_bytes(b"not a real pyc")
        self.addCleanup(shutil.rmtree, stray, ignore_errors=True)

        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            hb.prepare("ledger", workspace)
            self.assertFalse((workspace / "ledger" / "__pycache__").exists())


class StarterFailsHiddenGraderTests(unittest.TestCase):
    """A buggy starter must score fewer than the full hidden-test total."""

    def test_every_starter_fails_some_hidden_tests(self):
        for case_id in _case_ids():
            report = hb.grade(case_id, _prepared_starter(self, case_id))
            self.assertGreaterEqual(report["total"], _MIN_HIDDEN_TESTS, case_id)
            self.assertLess(report["passed"], report["total"],
                            f"{case_id} starter passes its whole hidden suite")
            self.assertTrue(report["errors"], case_id)

    def test_every_starter_fails_a_substantial_share_of_hidden_tests(self):
        for case_id in _case_ids():
            report = hb.grade(case_id, _prepared_starter(self, case_id))
            failed = report["total"] - report["passed"]
            self.assertGreaterEqual(failed * 5, report["total"] * 2,
                                    f"{case_id} starter fails only {failed}/{report['total']}, "
                                    "which is too easy a benchmark case")

    def test_visible_tests_pass_on_every_starter(self):
        for case_id in _case_ids():
            workspace = _prepared_starter(self, case_id)
            result = subprocess.run(
                [sys.executable, "-m", "unittest", "discover", "-s", "tests"],
                cwd=workspace, capture_output=True, text=True, timeout=60,
            )
            self.assertEqual(result.returncode, 0, f"{case_id}: {result.stderr}")


class ReferencePassesHiddenGraderTests(unittest.TestCase):
    """The held-out reference (never copied into a solver workspace) must
    score a perfect, error-free total against its own hidden grader."""

    def _reference_workspace(self, case_id, package_name):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        workspace = Path(tmp.name)
        reference_src = _FIXTURES / case_id / "reference" / package_name
        shutil.copytree(reference_src, workspace / package_name)
        return workspace

    def test_every_reference_passes_every_hidden_test(self):
        for case_id in _case_ids():
            workspace = self._reference_workspace(case_id, _package_name(case_id))
            report = hb.grade(case_id, workspace)
            self.assertEqual(report["errors"], [], case_id)
            self.assertEqual(report["passed"], report["total"], case_id)
            self.assertGreaterEqual(report["total"], _MIN_HIDDEN_TESTS, case_id)

    def test_every_declared_hidden_test_count_is_exact(self):
        """The declared count is what a failure report claims as the total, so
        it must equal the number of tests the oracle actually runs."""
        for case_id in _case_ids():
            actual = hb.grade(case_id, self._reference_workspace(case_id, _package_name(case_id)))
            empty = tempfile.TemporaryDirectory()
            self.addCleanup(empty.cleanup)
            declared = hb.grade(case_id, Path(empty.name))  # no package: total from the declaration
            self.assertEqual(declared["passed"], 0, case_id)
            self.assertEqual(declared["total"], actual["total"], case_id)


class GradeIsolationTests(unittest.TestCase):
    """grade() must not leak state between successive calls (needed since a
    single harness process may grade many solver workspaces in sequence),
    and must survive candidates that hang, are missing, or fail to import -
    each of those runs in its own bounded subprocess."""

    def test_grading_reference_after_starter_still_sees_the_bug_free_result(self):
        starter_tmp = tempfile.TemporaryDirectory()
        self.addCleanup(starter_tmp.cleanup)
        starter_ws = Path(starter_tmp.name)
        hb.prepare("ledger", starter_ws)
        starter_report = hb.grade("ledger", starter_ws)
        self.assertLess(starter_report["passed"], starter_report["total"])

        ref_tmp = tempfile.TemporaryDirectory()
        self.addCleanup(ref_tmp.cleanup)
        ref_ws = Path(ref_tmp.name)
        shutil.copytree(_FIXTURES / "ledger" / "reference" / "ledger", ref_ws / "ledger")
        ref_report = hb.grade("ledger", ref_ws)
        self.assertEqual(ref_report, {"passed": 22, "total": 22, "errors": []})

    def test_grading_does_not_touch_this_processs_sys_path(self):
        before = list(sys.path)
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            hb.prepare("scheduler", workspace)
            hb.grade("scheduler", workspace)
        self.assertEqual(sys.path, before)

    def test_a_missing_package_grades_as_a_failure_report_not_a_crash(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)  # empty: no ledger/ package at all
            report = hb.grade("ledger", workspace)
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["total"], 22)
        self.assertTrue(report["errors"])

    def test_an_import_error_candidate_grades_as_a_failure_report_not_a_crash(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            package = workspace / "ledger"
            package.mkdir()
            (package / "__init__.py").write_text("raise RuntimeError('boom at import time')\n")
            report = hb.grade("ledger", workspace)
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["total"], 22)
        self.assertIn("boom at import time", report["errors"][0])

    def test_a_hanging_candidate_times_out_into_a_failure_report_not_a_hang(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            package = workspace / "scheduler"
            package.mkdir()
            (package / "__init__.py").write_text("while True:\n    pass\n")
            report = hb.grade("scheduler", workspace, timeout_seconds=_SHORT_TIMEOUT)
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["total"], 22)
        self.assertIn("timed out", report["errors"][0])

    def test_a_candidate_that_exits_the_process_grades_as_a_failure_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            package = workspace / "ledger"
            package.mkdir()
            (package / "__init__.py").write_text("import sys\nsys.exit(1)\n")
            report = hb.grade("ledger", workspace)
        self.assertEqual(report["passed"], 0)
        self.assertEqual(report["total"], 22)
        self.assertTrue(report["errors"])


# Each starter's counted grade, as the one-process grader gave it before the test-by-test grade existed.
_STARTER_COUNTED = {
    "ledger": (8, 22), "scheduler": (12, 22), "large": (22, 40), "recovery": (23, 41),
    "transactions": (11, 40), "leases": (3, 28), "concurrent": (11, 49), "eventlog": (20, 49),
    "concurrent_v2": (18, 67), "matchbook": (23, 119), "gridcalc": (5, 154),
    "billing": (24, 79), "semver": (34, 80), "minilang": (45, 102),
}
_HANGING_TEST = "test_deposit_and_withdraw_within_balance_succeed"
_HANG_IN_ONE_TEST = """
import sys as _sys

_real_apply = LedgerEngine.apply


def _apply(self, *args, **kwargs):
    frame = _sys._getframe(1)
    while frame is not None:
        if frame.f_code.co_name == {name!r}:
            while True:
                pass
        frame = frame.f_back
    return _real_apply(self, *args, **kwargs)


LedgerEngine.apply = _apply
"""
_HANG_IN_EVERY_APPLY = """

def _apply(self, *args, **kwargs):
    while True:
        pass


LedgerEngine.apply = _apply
"""


def _reference_ledger(test_case, appended=""):
    tmp = tempfile.TemporaryDirectory()
    test_case.addCleanup(tmp.cleanup)
    workspace = Path(tmp.name)
    shutil.copytree(_FIXTURES / "ledger" / "reference" / "ledger", workspace / "ledger")
    with (workspace / "ledger" / "__init__.py").open("a") as init:
        init.write(appended)
    return workspace


class TestByTestGradeTests(unittest.TestCase):
    """The second grade stands beside the counted one and never replaces it."""

    def test_a_hang_in_one_hidden_test_counts_zero_and_all_but_one_test_by_test(self):
        workspace = _reference_ledger(self, _HANG_IN_ONE_TEST.format(name=_HANGING_TEST))
        counted, by_test = hb.grade_with_tests("ledger", workspace, timeout_seconds=_SHORT_TIMEOUT,
                                               per_test_seconds=2, by_test_total_seconds=60)
        self.assertEqual(counted, {"passed": 0, "total": 22,
                                   "errors": [f"hidden tests timed out after {_SHORT_TIMEOUT}s"]})
        self.assertEqual({key: by_test[key] for key in ("passed", "failed", "hung", "not_run", "total")},
                         {"passed": 21, "failed": 0, "hung": 1, "not_run": 0, "total": 22})
        self.assertEqual(by_test["hung_tests"], [f"test_ledger_hidden.LedgerHiddenTests.{_HANGING_TEST}"])
        self.assertEqual(by_test["source"], hb.BY_TEST_FROM_PROCESSES)

    def test_the_counted_grade_of_that_solution_is_what_grade_returns(self):
        workspace = _reference_ledger(self, _HANG_IN_ONE_TEST.format(name=_HANGING_TEST))
        self.assertEqual(hb.grade("ledger", workspace, timeout_seconds=_SHORT_TIMEOUT),
                         {"passed": 0, "total": 22, "errors": [f"hidden tests timed out after {_SHORT_TIMEOUT}s"]})

    def test_a_solution_without_a_hang_gives_equal_fields(self):
        for workspace, passed in ((_reference_ledger(self), 22), (_prepared_starter(self, "ledger"), 8)):
            counted, by_test = hb.grade_with_tests("ledger", workspace)
            self.assertEqual((counted["passed"], counted["total"]), (passed, 22))
            self.assertEqual(by_test, {"passed": passed, "failed": 22 - passed, "hung": 0, "not_run": 0,
                                       "total": 22, "hung_tests": [], "errors": [],
                                       "source": hb.BY_TEST_FROM_COUNTED})

    def test_every_starter_keeps_its_counted_grade_and_the_second_field_repeats_it(self):
        self.assertEqual(sorted(_STARTER_COUNTED), sorted(_case_ids()))
        for case_id, expected in _STARTER_COUNTED.items():
            workspace = _prepared_starter(self, case_id)
            alone = hb.grade(case_id, workspace)
            counted, by_test = hb.grade_with_tests(case_id, workspace)
            self.assertEqual((alone["passed"], alone["total"]), expected, case_id)
            self.assertEqual((counted["passed"], counted["total"]), expected, case_id)
            self.assertEqual((by_test["passed"], by_test["hung"], by_test["total"]), (expected[0], 0, expected[1]))

    def test_the_total_limit_bounds_a_solution_that_hangs_in_every_test(self):
        workspace = _reference_ledger(self, _HANG_IN_EVERY_APPLY)
        started = time.monotonic()
        counted, by_test = hb.grade_with_tests("ledger", workspace, timeout_seconds=_SHORT_TIMEOUT,
                                               per_test_seconds=1, by_test_total_seconds=3)
        self.assertLess(time.monotonic() - started, _SHORT_TIMEOUT + 3 + 5)
        self.assertEqual(counted["passed"], 0)
        self.assertGreater(by_test["not_run"], 0)
        self.assertLessEqual(by_test["hung"], 3)
        self.assertEqual(by_test["passed"] + by_test["failed"] + by_test["hung"] + by_test["not_run"], 22)

    def test_a_solution_that_hangs_at_import_is_reported_as_not_run_not_as_failed(self):
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp) / "scheduler"
            package.mkdir()
            (package / "__init__.py").write_text("while True:\n    pass\n")
            counted, by_test = hb.grade_with_tests("scheduler", Path(tmp), timeout_seconds=_SHORT_TIMEOUT,
                                                   per_test_seconds=1, by_test_total_seconds=2)
        self.assertEqual((counted["passed"], counted["total"]), (0, 22))
        self.assertEqual((by_test["passed"], by_test["failed"], by_test["hung"], by_test["not_run"]), (0, 0, 0, 22))
        self.assertIn("could not be listed", by_test["errors"][0])


def _reference_workspace(test_case, case_id):
    tmp = tempfile.TemporaryDirectory()
    test_case.addCleanup(tmp.cleanup)
    package = _package_name(case_id)
    shutil.copytree(_FIXTURES / case_id / "reference" / package, Path(tmp.name) / package)
    return Path(tmp.name)


class GradeWrapperTests(unittest.TestCase):
    """The wrapper the runner calls: a failure of the test-by-test half never touches the counted grade."""

    def _counted_under_a_short_limit(self):
        real = hb.grade_counted
        return patch.object(hb, "grade_counted",
                            side_effect=lambda case_id, workspace: real(case_id, workspace, _SHORT_TIMEOUT))

    def test_a_test_by_test_half_that_raises_after_a_timeout_leaves_the_counted_zero(self):
        workspace = _reference_ledger(self, _HANG_IN_ONE_TEST.format(name=_HANGING_TEST))
        with self._counted_under_a_short_limit(), \
             patch.object(hb, "_grade_test_by_test", side_effect=OSError("cannot fork")) as by_test_half:
            counted, by_test, error, by_test_error = assets.run_grade_with_tests("ledger", workspace)
        self.assertEqual(by_test_half.call_count, 1)
        self.assertEqual(counted, {"passed": 0, "total": 22,
                                   "errors": [f"hidden tests timed out after {_SHORT_TIMEOUT}s"]})
        self.assertIsNone(error)
        self.assertIsNone(by_test)
        self.assertEqual(by_test_error, "grade_by_test() raised OSError: cannot fork")

    def test_a_test_by_test_half_that_raises_without_a_timeout_leaves_the_counted_grade(self):
        workspace = _prepared_starter(self, "ledger")
        with patch.object(hb, "grade_by_test", side_effect=RuntimeError("boom")):
            counted, by_test, error, by_test_error = assets.run_grade_with_tests("ledger", workspace)
        self.assertEqual((counted["passed"], counted["total"]), _STARTER_COUNTED["ledger"])
        self.assertEqual((by_test, error), (None, None))
        self.assertEqual(by_test_error, "grade_by_test() raised RuntimeError: boom")

    def test_a_test_by_test_grade_that_does_not_add_up_is_unknown_and_not_a_grade(self):
        workspace = _prepared_starter(self, "ledger")
        short = {"passed": 8, "failed": 3, "hung": 0, "not_run": 0, "total": 22}
        for wrong, reason in ((short, "ValueError: the test-by-test grade does not add up to its total"),
                              ({"passed": 8}, "KeyError: 'failed'"), (None, "TypeError: ")):
            with patch.object(hb, "grade_by_test", return_value=wrong):
                counted, by_test, error, by_test_error = assets.run_grade_with_tests("ledger", workspace)
            self.assertEqual((counted["passed"], counted["total"]), _STARTER_COUNTED["ledger"], reason)
            self.assertEqual((by_test, error), (None, None), reason)
            self.assertIn(f"grade_by_test() raised {reason}", by_test_error)

    def test_a_test_by_test_count_that_is_no_integer_within_the_total_is_unknown(self):
        workspace = _prepared_starter(self, "ledger")
        right = {"passed": 8, "failed": 14, "hung": 0, "not_run": 0, "total": 22}
        not_an_integer = "ValueError: the test-by-test grade holds a count that is not an integer"
        out_of_range = "ValueError: the test-by-test grade holds a count outside 0 to its total"
        for change, reason in (({"passed": 8.0}, not_an_integer), ({"hung": True, "failed": 13}, not_an_integer),
                               ({"not_run": None}, not_an_integer), ({"total": 22.0}, not_an_integer),
                               ({"passed": 23, "failed": -1}, out_of_range),
                               ({"passed": 30, "failed": -8}, out_of_range)):
            with patch.object(hb, "grade_by_test", return_value={**right, **change}):
                counted, by_test, error, by_test_error = assets.run_grade_with_tests("ledger", workspace)
            self.assertEqual((counted["passed"], counted["total"]), _STARTER_COUNTED["ledger"], change)
            self.assertEqual((by_test, error), (None, None), change)
            self.assertEqual(by_test_error, f"grade_by_test() raised {reason}", change)
        with patch.object(hb, "grade_by_test", return_value=dict(right)):
            self.assertEqual(assets.run_grade_with_tests("ledger", workspace)[1:], (right, None, None))

    def test_a_failed_counted_grade_is_reported_as_before_and_is_not_graded_test_by_test(self):
        failures = (({"side_effect": RuntimeError("boom")}, "grade() raised RuntimeError: boom"),
                    ({"return_value": ({"passed": 1}, False)}, "grade() raised ValueError: grade() contract violated"),
                    ({"return_value": ({"passed": 3, "total": 2, "errors": []}, False)},
                     "grade() raised ValueError: grade() returned impossible counters"))
        for behaviour, expected in failures:
            with patch.object(hb, "grade_counted", **behaviour), patch.object(hb, "grade_by_test") as by_test_half:
                graded = assets.run_grade_with_tests("ledger", Path("unused"))
                alone = assets.run_grade("ledger", Path("unused"))
            self.assertEqual(graded, (None, None, expected, assets.BY_TEST_NEEDS_COUNTED))
            self.assertEqual(alone, (None, expected))
            self.assertEqual(by_test_half.call_count, 0)

    def test_every_fixture_solution_keeps_its_counted_grade_through_the_wrapper(self):
        self.assertEqual(sorted(_STARTER_COUNTED), sorted(_case_ids()))
        for case_id, (starter_passed, total) in _STARTER_COUNTED.items():
            solutions = ((_prepared_starter(self, case_id), starter_passed),
                         (_reference_workspace(self, case_id), total))
            for workspace, passed in solutions:
                counted, by_test, error, by_test_error = assets.run_grade_with_tests(case_id, workspace)
                self.assertEqual((error, by_test_error), (None, None), case_id)
                self.assertEqual((counted["passed"], counted["total"]), (passed, total), case_id)
                self.assertEqual(sorted(counted), ["errors", "passed", "total"], case_id)
                self.assertEqual((by_test["passed"], by_test["failed"], by_test["hung"], by_test["not_run"]),
                                 (passed, total - passed, 0, 0), case_id)

    def test_the_counted_half_alone_gives_the_same_grade(self):
        workspace = _prepared_starter(self, "ledger")
        counted, error = assets.run_grade("ledger", workspace)
        self.assertIsNone(error)
        self.assertEqual((counted["passed"], counted["total"]), _STARTER_COUNTED["ledger"])
        self.assertEqual(counted, hb.grade("ledger", workspace))


if __name__ == "__main__":
    unittest.main()
