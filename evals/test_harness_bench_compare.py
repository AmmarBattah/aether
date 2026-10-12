"""Offline tests for the four-measure native/Aether comparison report."""
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import harness_bench_compare as compare
import harness_bench_report as report
from harness_bench_test_support import TEST_SCRATCH

SAVED = Path(__file__).parent / "fixtures" / "harness_bench_compare"
HAIKU, LUNA = SAVED / "haiku-block.json", SAVED / "luna-block.json"


def table_row(markdown, section, measure):
    body = markdown.split(f"### {section}\n", 1)[1].split("\n### ", 1)[0]
    matches = [line for line in body.splitlines() if line.startswith(f"| {measure} |")]
    assert len(matches) == 1, f"{measure!r} appears {len(matches)} times in {section}"
    return [cell.strip() for cell in matches[0].strip("|").split("|")]


class SavedManifestReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows = compare.load_rows([HAIKU, LUNA])
        cls.markdown = compare.render(cls.rows)

    def test_one_table_per_model_pair_with_both_sides(self):
        self.assertEqual(self.markdown.count("| Measure | Native | Aether | Aether minus native |"), 2)
        self.assertEqual(table_row(self.markdown, "haiku-low", "Trials"), ["Trials", "2", "2", ""])
        self.assertEqual(table_row(self.markdown, "haiku-low", "Workflows finished")[1:3], ["2/2", "1/2"])

    def test_hidden_checks_give_mean_fraction_and_per_run_values(self):
        mean = table_row(self.markdown, "haiku-low", "Hidden checks passed, mean")
        self.assertEqual(mean[1:], ["75.0 % (2/2)", "72.5 % (2/2)", "-2.5 pp (2 pairs)"])
        per_run = table_row(self.markdown, "haiku-low", "Hidden checks per run")
        self.assertEqual(per_run[1:3], ["gridcalc: 60/100; matchbook: 72/80", "gridcalc: 50/100; matchbook: 76/80"])

    def test_token_measures_average_only_the_trials_that_measured_them(self):
        self.assertEqual(table_row(self.markdown, "haiku-low", "Uncached input tokens, mean")[1:],
                         ["150 (2/2)", "120 (1/2)", "+20 (1 pairs)"])
        self.assertEqual(table_row(self.markdown, "haiku-low", "Output tokens, mean")[1:],
                         ["1,500 (2/2)", "1,100 (1/2)", "+100 (1 pairs)"])
        self.assertEqual(table_row(self.markdown, "haiku-low", "Cache read tokens, mean")[3], "+500 (1 pairs)")
        self.assertEqual(table_row(self.markdown, "haiku-low", "Cache write tokens, mean")[3], "+50 (1 pairs)")

    def test_cost_names_its_basis_and_priced_count(self):
        cost = table_row(self.markdown, "haiku-low", "API-equivalent USD, mean")
        self.assertEqual(cost[1:], ["0.0600 (billed; 2/2 priced)", "0.0600 (billed; 1/2 priced)", "+0.0100 (1 pairs)"])

    def test_an_unpriced_cell_is_reported_unpriced_never_zero(self):
        cost = table_row(self.markdown, "luna-low", "API-equivalent USD, mean")
        self.assertEqual(cost[1:], ["0.0100 (estimated; 2/2 priced)", "unpriced (0/1 priced)", "unknown (0 pairs)"])
        aether = [row for row in self.rows if row["condition"] == "aether-single-luna-low"]
        self.assertTrue(all(row["usd"] is None for row in aether))

    def test_wall_seconds_give_mean_and_spread(self):
        wall = table_row(self.markdown, "haiku-low", "Wall seconds, mean (min to max)")
        self.assertEqual(wall[1:], ["250.0 (200.0 to 300.0; 2/2)", "270.0 (230.0 to 310.0; 2/2)", "+20.0 (2 pairs)"])

    def test_an_ungraded_run_is_unknown_and_left_out_of_the_mean(self):
        self.assertEqual(table_row(self.markdown, "luna-low", "Hidden checks per run")[1], "ledger: 22/22, unknown")
        self.assertEqual(table_row(self.markdown, "luna-low", "Hidden checks passed, mean")[1], "100.0 % (1/2)")

    def test_blocked_trials_are_excluded_and_counted(self):
        self.assertEqual(table_row(self.markdown, "luna-low", "Trials")[1:3], ["2", "1"])
        self.assertEqual(table_row(self.markdown, "luna-low", "Blocked trials (excluded above)")[1:3], ["0", "1"])
        self.assertEqual(table_row(self.markdown, "luna-low", "Wall seconds, mean (min to max)")[3], "+6.0 (1 pairs)")

    def test_effort_is_requested_unless_session_metadata_confirms_it(self):
        self.assertIn("- Native: `claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)", self.markdown)
        self.assertIn("- Aether: `claude-code/claude-haiku-5-5` (resolved: claude-haiku-5-5; effort low, requested)",
                      self.markdown)
        self.assertIn("- Aether: `codex/gpt-6-luna` (resolved: gpt-6-luna; effort low, confirmed by session metadata)",
                      self.markdown)

    def test_each_side_names_its_accounting_scope(self):
        haiku = self.markdown.split("### haiku-low\n", 1)[1].split("\n### ", 1)[0]
        self.assertIn("- Native token and cost accounting: native receipt\n", haiku)
        self.assertIn("- Aether token and cost accounting: Aether durable lower bound; native final receipt "
                      "unavailable / Claude all-model process receipts\n", haiku)
        luna = self.markdown.split("### luna-low\n", 1)[1]
        self.assertIn("- Aether token and cost accounting: Aether durable (may omit helpers)\n", luna)

    def test_pairing_within_one_manifest_is_stated(self):
        self.assertIn("trials from different manifests are never paired", self.markdown)

    def test_concurrency_is_stated_per_pair(self):
        haiku = self.markdown.split("### haiku-low\n", 1)[1].split("\n### ", 1)[0]
        self.assertIn("Trials at once (`--jobs`): 4.", haiku)
        self.assertIn("Trials at once (`--jobs`): 1.", self.markdown.split("### luna-low\n", 1)[1])

    def test_unmatched_cells_are_named_not_dropped(self):
        self.assertIn("Cells without a matched partner, not tabulated: native-claude.", self.markdown)

    def test_the_count_line_counts_trials_not_runs(self):
        haiku = self.markdown.split("### haiku-low\n", 1)[1].split("\n### ", 1)[0]
        self.assertIn("2 native and 2 Aether trials. Counts in brackets", haiku)
        self.assertNotIn("Aether runs.", self.markdown)

    def test_no_significance_is_claimed(self):
        self.assertEqual(self.markdown.count("No significance is claimed."), 2)
        self.assertNotRegex(self.markdown.lower(), r"p\s*[<=]|confidence interval|statistically")


class PairingTests(unittest.TestCase):
    def test_pairs_never_cross_manifests_cases_or_runs(self):
        rows = compare.load_rows([HAIKU, LUNA])
        native = [row for row in rows if row["condition"] == "native-luna-low"]
        aether = [row for row in rows if row["condition"] == "aether-single-haiku-low"]
        differences = compare.paired_differences(native, aether)
        self.assertTrue(all(value == {"mean": None, "pairs": 0} for value in differences.values()))

    def test_a_side_missing_its_partner_is_not_tabulated(self):
        rows = [row for row in compare.load_rows([HAIKU]) if row["side"] == "native"]
        markdown = compare.render(rows)
        self.assertNotIn("| Measure |", markdown)
        self.assertIn("not tabulated: native-haiku-low.", markdown)

    def test_unknown_measures_stay_unknown_on_both_sides(self):
        rows = compare.load_rows([HAIKU])
        for row in rows:
            row.update(solve_seconds=None, check_fraction=None, checks_passed=None, checks_total=None)
        markdown = compare.render(rows)
        self.assertEqual(table_row(markdown, "haiku-low", "Wall seconds, mean (min to max)")[1:],
                         ["unknown (0/2)", "unknown (0/2)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", "Hidden checks passed, mean")[1], "unknown (0/2)")


def incomplete(coverage, fields, terminal=1, units=3):
    """The coverage a timed-out trial leaves: some units never reached a terminal receipt."""
    return {**coverage, **{field: {**coverage[field], "terminal": terminal, "units": units, "complete": False}
                           for field in fields}}


ALL_RECEIPT_FIELDS = ("uncached_input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens",
                      "total_tokens", "api_equivalent_cost_usd")
TOKEN_ROWS = ("Uncached input tokens, mean", "Output tokens, mean", "Cache read tokens, mean",
              "Cache write tokens, mean")
PARTIAL_ROW = "Partial-receipt trials (in no token or cost mean)"


class PartialReceiptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-partial-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def _render(self, change):
        manifest = json.loads(HAIKU.read_text())
        change({trial["short_id"]: trial for trial in manifest["trials"]})
        path = self.tmp / "manifest.json"
        path.write_text(json.dumps(manifest))
        return compare.render(compare.load_rows([path]))

    def test_an_unreconciled_incomplete_aether_trial_is_not_a_measured_mean_or_pair(self):
        def change(trials):
            del trials["h2"]["claude_reconciliation"]
            report = trials["h2"]["usage_report"]
            report["coverage"] = incomplete(report["coverage"], ALL_RECEIPT_FIELDS)
        markdown = self._render(change)
        for measure in TOKEN_ROWS:
            self.assertEqual(table_row(markdown, "haiku-low", measure)[2:], ["unknown (0/2)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", "API-equivalent USD, mean")[2:],
                         ["unknown, partial receipts only (0/2 priced)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", PARTIAL_ROW)[1:3], ["0", "1"])
        self.assertIn("Partial receipts are lower bounds and enter no mean or paired difference above: "
                      "Aether matchbook run 1 (Uncached input tokens at least 90, Output tokens at least 900, "
                      "Cache read tokens at least 4,000, Cache write tokens at least 100, "
                      "API-equivalent USD at least 0.0400).", markdown)
        self.assertIn("- Aether token and cost accounting: "
                      "Aether durable lower bound; native final receipt unavailable", markdown)
        self.assertEqual(table_row(markdown, "haiku-low", "Uncached input tokens, mean")[1], "150 (2/2)")

    def test_a_durable_only_claude_trial_is_a_lower_bound_even_with_complete_coverage(self):
        markdown = self._render(lambda trials: trials["h2"].pop("claude_reconciliation"))
        self.assertEqual(table_row(markdown, "haiku-low", "Output tokens, mean")[2:],
                         ["unknown (0/2)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", PARTIAL_ROW)[2], "1")

    def test_incomplete_process_receipts_are_withheld_too(self):
        def change(trials):
            reconciliation = trials["h2"]["claude_reconciliation"]
            reconciliation["coverage"] = incomplete(reconciliation["coverage"], ALL_RECEIPT_FIELDS, terminal=2)
        markdown = self._render(change)
        self.assertEqual(table_row(markdown, "haiku-low", "Cache read tokens, mean")[2:],
                         ["unknown (0/2)", "unknown (0 pairs)"])
        self.assertIn("Aether matchbook run 1 (Uncached input tokens at least 120,", markdown)

    def test_only_the_field_with_incomplete_coverage_is_withheld(self):
        def change(trials):
            reconciliation = trials["h2"]["claude_reconciliation"]
            reconciliation["coverage"] = incomplete(reconciliation["coverage"], ["cache_write_tokens"])
        markdown = self._render(change)
        self.assertEqual(table_row(markdown, "haiku-low", "Cache write tokens, mean")[2:],
                         ["unknown (0/2)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", "Output tokens, mean")[2:],
                         ["1,100 (1/2)", "+100 (1 pairs)"])
        self.assertIn("Aether matchbook run 1 (Cache write tokens at least 250).", markdown)

    def test_a_native_trial_with_incomplete_receipts_is_withheld_the_same_way(self):
        def change(trials):
            report = trials["h1"]["usage_report"]
            report["coverage"] = incomplete(report["coverage"], ALL_RECEIPT_FIELDS)
        markdown = self._render(change)
        self.assertEqual(table_row(markdown, "haiku-low", "Uncached input tokens, mean")[1:],
                         ["200 (1/2)", "120 (1/2)", "unknown (0 pairs)"])
        self.assertEqual(table_row(markdown, "haiku-low", PARTIAL_ROW)[1:3], ["1", "0"])
        self.assertIn("Native matchbook run 1 (Uncached input tokens at least 100,", markdown)

    def test_a_figure_without_a_coverage_entry_is_not_assumed_complete(self):
        def change(trials):
            for field in ("output_tokens", "api_equivalent_cost_usd"):
                del trials["h1"]["usage_report"]["coverage"][field]
        markdown = self._render(change)
        self.assertEqual(table_row(markdown, "haiku-low", "Output tokens, mean")[1], "2,000 (1/2)")
        self.assertEqual(table_row(markdown, "haiku-low", "API-equivalent USD, mean")[1],
                         "0.0700 (billed; 1/2 priced)")
        self.assertEqual(table_row(markdown, "haiku-low", "Uncached input tokens, mean")[1], "150 (2/2)")

    def test_a_case_both_sides_pass_in_full_is_named_as_the_accuracy_ceiling(self):
        def change(trials):
            for short_id in ("h1", "h2"):
                trials[short_id]["grade"].update(passed=80, total=80)
        markdown = self._render(change)
        self.assertIn("Accuracy is at the ceiling on matchbook: every run of both sides passed every hidden "
                      "check, so this pair can show a loss there but not a gain.", markdown)

    def test_one_side_short_of_full_marks_or_ungraded_is_not_a_ceiling(self):
        markdown = self._render(lambda trials: trials["h1"]["grade"].update(passed=80, total=80))
        self.assertNotIn("at the ceiling", markdown)
        self.assertNotIn("at the ceiling", compare.render(compare.load_rows([LUNA])),
                         "an ungraded native run cannot make ledger a ceiling case")

    def test_fully_measured_manifests_carry_no_lower_bound_note(self):
        markdown = compare.render(compare.load_rows([HAIKU, LUNA]))
        self.assertNotIn("Partial receipts are lower bounds", markdown)
        self.assertEqual(table_row(markdown, "haiku-low", PARTIAL_ROW)[1:3], ["0", "0"])
        self.assertEqual(table_row(markdown, "luna-low", PARTIAL_ROW)[1:3], ["0", "0"])


def trial_line(markdown, case, cell, run="1"):
    body = markdown.split("## Every trial\n", 1)[1]
    lines = [[cell.strip() for cell in line.strip("|").split("|")] for line in body.splitlines()
             if line.startswith("| ") and not line.startswith("| Manifest")]
    matches = [line for line in lines if line[1:4] == [case, run, cell]]
    assert len(matches) == 1, f"{case} {cell} run {run} appears {len(matches)} times"
    return matches[0]


class TrialTableTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-trials-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_every_trial_has_one_line_with_its_own_figures(self):
        rows = compare.load_rows([HAIKU, LUNA])
        markdown = compare.render_trials(rows)
        self.assertEqual(sum(line.startswith("| harness_bench_compare/") for line in markdown.splitlines()),
                         len(rows))
        self.assertEqual(trial_line(markdown, "matchbook", "native-haiku-low"),
                         ["harness_bench_compare/haiku-block.json", "matchbook", "1", "native-haiku-low",
                          "completed", "72/80", "100", "1,000", "5,000", "200", "0.0500", "200.0"])

    def test_the_manifest_column_never_holds_a_full_path(self):
        markdown = compare.render_trials(compare.load_rows([HAIKU.resolve()]))
        self.assertNotIn(str(HAIKU.resolve().parent.parent), markdown)
        self.assertIn("| harness_bench_compare/haiku-block.json |", markdown)

    def test_a_timed_out_trial_keeps_its_grade_and_unknown_figures(self):
        line = trial_line(compare.render_trials(compare.load_rows([HAIKU])), "gridcalc", "aether-single-haiku-low")
        self.assertEqual(line[4:], ["timeout", "50/100", "unknown", "unknown", "unknown", "unknown", "unknown", "310.0"])

    def test_a_lower_bound_is_named_and_never_printed_as_a_measurement(self):
        manifest = json.loads(HAIKU.read_text())
        trial = next(trial for trial in manifest["trials"] if trial["short_id"] == "h2")
        del trial["claude_reconciliation"]
        path = self.tmp / "manifest.json"
        path.write_text(json.dumps(manifest))
        line = trial_line(compare.render_trials(compare.load_rows([path])), "matchbook", "aether-single-haiku-low")
        self.assertEqual(line[6:11], ["at least 90", "at least 900", "at least 4,000", "at least 100",
                                      "at least 0.0400"])

    def test_unpriced_ungraded_and_blocked_trials_say_so(self):
        markdown = compare.render_trials(compare.load_rows([LUNA]))
        blocked = [line for line in markdown.splitlines() if "| blocked |" in line]
        self.assertEqual(len(blocked), 1)
        self.assertIn("| unpriced |", blocked[0])
        self.assertEqual(trial_line(markdown, "ledger", "native-luna-low", run="2")[5], "unknown")
        self.assertNotIn("| 0.0000 |", markdown)

    def _single_run_manifests(self, grades):
        """One single-run manifest per (file name, native passed, Aether passed), in the order given."""
        paths = []
        for name, native, aether in grades:
            manifest = json.loads(HAIKU.read_text())
            trials = {trial["short_id"]: trial for trial in manifest["trials"]}
            trials["h1"]["grade"].update(passed=native)
            trials["h2"]["grade"].update(passed=aether)
            for trial in manifest["trials"]:
                trial["short_id"] = f"{name}-{trial['short_id']}"
            paths.append(self.tmp / f"{name}.json")
            paths[-1].write_text(json.dumps(manifest))
        return paths

    def test_merged_single_run_manifests_keep_the_order_given_on_both_sides(self):
        rows = compare.load_rows(self._single_run_manifests((("c", 72, 76), ("a", 60, 80), ("b", 80, 40))))
        self.assertEqual({row["run"] for row in rows}, {1})
        per_run = table_row(compare.render(rows), "haiku-low", "Hidden checks per run")
        self.assertEqual(per_run[1], "gridcalc: 60/100, 60/100, 60/100; matchbook: 72/80, 60/80, 80/80")
        self.assertEqual(per_run[2], "gridcalc: 50/100, 50/100, 50/100; matchbook: 76/80, 80/80, 40/80")
        markdown = compare.render_trials(rows)
        listed = [line.split("|")[1].strip() for line in markdown.splitlines() if line.startswith("| hb-trials-")]
        self.assertEqual([name.rsplit("/", 1)[1] for name in listed], ["c.json"] * 4 + ["a.json"] * 4 + ["b.json"] * 4)
        self.assertLess(markdown.index(compare.TRIAL_ORDER_NOTE), markdown.index("| Manifest |"))

    def test_a_manifest_without_the_test_by_test_grade_loads_and_gets_no_column(self):
        rows = compare.load_rows([HAIKU, LUNA])
        self.assertTrue(all(row["checks_by_test"] is None for row in rows))
        markdown = compare.render_trials(rows)
        self.assertNotIn(compare.BY_TEST_COLUMN, markdown)
        self.assertNotIn(compare.BY_TEST_NOTE, markdown)

    def test_the_test_by_test_grade_has_its_own_column_and_leaves_the_counted_one_alone(self):
        manifest = json.loads(HAIKU.read_text())
        trials = {trial["short_id"]: trial for trial in manifest["trials"]}
        trials["h1"]["grade"].update(passed=0)
        trials["h1"]["grade_by_test"] = {"passed": 79, "failed": 0, "hung": 1, "not_run": 0, "total": 80,
                                         "hung_tests": ["test_x.T.test_y"], "errors": [], "source": "s"}
        trials["h2"]["grade_by_test"] = {"passed": 76, "failed": 4, "hung": 0, "not_run": 0, "total": 80,
                                         "hung_tests": [], "errors": [], "source": "s"}
        path = self.tmp / "manifest.json"
        path.write_text(json.dumps(manifest))
        markdown = compare.render_trials(compare.load_rows([path]))
        header = next(line for line in markdown.splitlines() if line.startswith("| Manifest"))
        self.assertEqual([cell.strip() for cell in header.strip("|").split("|")][5:7],
                         ["Hidden checks", compare.BY_TEST_COLUMN])
        self.assertEqual(trial_line(markdown, "matchbook", "native-haiku-low")[5:7], ["0/80", "79/80 (1 hung)"])
        self.assertEqual(trial_line(markdown, "matchbook", "aether-single-haiku-low")[5:7], ["76/80", "76/80"])
        self.assertEqual(trial_line(markdown, "gridcalc", "native-haiku-low")[6], "not recorded")
        self.assertEqual(markdown.split("\n\n")[-1], compare.BY_TEST_NOTE + "\n")
        self.assertIn('"unknown" there means that grade could not be made', markdown)
        self.assertEqual(table_row(compare.render(compare.load_rows([path])), "haiku-low",
                                   "Hidden checks per run")[1].split("matchbook: ")[1], "0/80")

    def test_a_failed_test_by_test_grade_loads_as_unknown_and_leaves_the_counted_one_alone(self):
        manifest = json.loads(HAIKU.read_text())
        trials = {trial["short_id"]: trial for trial in manifest["trials"]}
        trials["h1"].update(grade_by_test=None, grade_by_test_error="grade_by_test() raised OSError: cannot fork")
        path = self.tmp / "manifest.json"
        path.write_text(json.dumps(manifest))
        rows = compare.load_rows([path])
        markdown = compare.render_trials(rows)
        counted = trials["h1"]["grade"]
        self.assertEqual(trial_line(markdown, "matchbook", "native-haiku-low")[5:7],
                         [f"{counted['passed']}/{counted['total']}", "unknown"])
        self.assertEqual(trial_line(markdown, "matchbook", "aether-single-haiku-low")[6], "not recorded")
        unchanged = compare.render(compare.load_rows([HAIKU])).replace(str(HAIKU), str(path))
        self.assertEqual(compare.render(rows), unchanged)

    def test_main_appends_the_trial_table_only_when_asked(self):
        self.assertNotIn("## Every trial", compare.main([str(HAIKU), "--output", str(self.tmp / "plain.md")]))
        markdown = compare.main([str(HAIKU), "--trials", "--output", str(self.tmp / "trials.md")])
        self.assertTrue(markdown.startswith("## Native and Aether, matched model and effort"))
        self.assertEqual(markdown.count("## Every trial"), 1)


class LoadingTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(dir=TEST_SCRATCH, prefix="hb-compare-"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def _saved(self, name, **changes):
        manifest = {**json.loads(HAIKU.read_text()), **changes}
        path = self.tmp / name
        path.write_text(json.dumps(manifest))
        return path

    def test_a_pilot_manifest_is_refused(self):
        with self.assertRaisesRegex(ValueError, "pilot manifests are calibration only and are never counted"):
            compare.load_rows([LUNA, self._saved("pilot.json", pilot=True)])

    def test_the_html_report_refuses_a_pilot_manifest_too(self):
        pilot = self._saved("pilot.json", pilot=True)
        argv = ["harness_bench_report.py", str(pilot), "--output-dir", str(self.tmp / "report")]
        with patch.object(sys, "argv", argv), self.assertRaisesRegex(ValueError, "never counted"):
            report.main()
        self.assertFalse((self.tmp / "report").exists(), "a refused pilot must not leave a report behind")

    def test_the_same_trial_in_two_manifests_is_refused(self):
        with self.assertRaisesRegex(ValueError, "duplicate trial h1"):
            compare.load_rows([HAIKU, self._saved("again.json")])

    def test_a_file_without_trials_is_refused(self):
        with self.assertRaisesRegex(ValueError, "not a harness-bench manifest"):
            compare.load_rows([self._saved("broken.json", trials=None)])
        with self.assertRaisesRegex(ValueError, "no trial results"):
            compare.load_rows([self._saved("empty.json", trials=[])])

    def test_main_writes_the_same_markdown_it_returns(self):
        output = self.tmp / "report.md"
        markdown = compare.main([str(HAIKU), str(LUNA), "--output", str(output)])
        self.assertEqual(output.read_text(), markdown)
        self.assertIn("### haiku-low", markdown)
        self.assertIn("### luna-low", markdown)


if __name__ == "__main__":
    unittest.main()
