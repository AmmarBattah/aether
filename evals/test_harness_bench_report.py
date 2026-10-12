import unittest

from harness_bench_report import aggregate, render_html, render_markdown, trial_summary


class ReportTests(unittest.TestCase):
    def row(self, **changes):
        row = {"condition": "native-codex", "case": "ledger", "run": 1,
               "trial_dir": "target/hb/example", "success": False, "status": "failed"}
        row.update(changes)
        return row

    def test_a_failed_test_by_test_grade_loads_and_is_not_an_error_of_the_trial(self):
        failed = "grade_by_test() raised OSError: cannot fork"
        row = trial_summary(self.row(grade={"passed": 0, "total": 22, "errors": ["hidden tests timed out after 60s"]},
                                     grade_by_test=None, grade_by_test_error=failed))
        self.assertEqual((row["checks_passed"], row["checks_total"]), (0, 22))
        self.assertEqual((row["checks_by_test"], row["checks_by_test_error"]), (None, failed))
        self.assertIsNone(row["error"])
        older = trial_summary(self.row())
        self.assertEqual((older["checks_by_test"], older["checks_by_test_error"]), (None, None))

    def test_missing_usage_is_not_free_work(self):
        row = trial_summary(self.row())
        self.assertIsNone(row["tokens"])
        self.assertIsNone(row["api_equivalent_usd"])
        group = aggregate([row])[0]
        self.assertIsNone(group["mean_observed_tokens"])
        self.assertIsNone(group["observed_tokens_per_success"])

    def test_failed_work_counts_in_consumption_per_success(self):
        rows = [trial_summary(self.row(success=success, usage_report={"usage": {"total_tokens": tokens}}))
                for success, tokens in [(True, 100), (False, 200)]]
        self.assertEqual(aggregate(rows)[0]["observed_tokens_per_success"], 300)

    def test_receipts_replace_not_add_to_claude_durable(self):
        row = trial_summary(self.row(condition="aether-single-claude",
            usage_report={"usage": {"total_tokens": 100}},
            claude_reconciliation={"units": [{}], "full_process_usage": {"total_tokens": 105}, "coverage": {}}))
        self.assertEqual(row["tokens"], 105)

    def test_mixed_team_does_not_replace_codex_with_claude_only(self):
        row = trial_summary(self.row(condition="aether-coordinator-team",
            usage_report={"usage": {"total_tokens": 300}},
            claude_reconciliation={"units": [{}], "full_process_usage": {"total_tokens": 105}, "coverage": {}}))
        self.assertEqual(row["tokens"], 300)
        self.assertIn("may omit helpers", row["accounting_scope"])

    def test_explicit_sonnet_treatment_keeps_receipt_coverage_visible(self):
        raw = self.row(condition="aether-single-sonnet-low",
                       requested_treatment={"kind": "aether-single", "model": "claude-code/sonnet"},
                       usage_report={"usage": {"total_tokens": 100}})
        self.assertIn("lower bound", trial_summary(raw)["accounting_scope"])
        raw["claude_reconciliation"] = {"full_process_usage": {"total_tokens": 110}, "coverage": {}}
        summary = trial_summary(raw)
        self.assertEqual(summary["tokens"], 110)
        self.assertEqual(summary["accounting_scope"], "Claude all-model process receipts")

    def test_missing_receipt_retains_labelled_durable_lower_bound(self):
        row = trial_summary(self.row(condition="aether-single-claude",
            usage_report={"usage": {"total_tokens": 100}},
            claude_reconciliation={"units": [{}], "full_process_usage": {"total_tokens": None}, "coverage": {}}))
        self.assertEqual(row["tokens"], 100)
        self.assertIn("lower bound", row["accounting_scope"])

    def test_amended_trials_are_not_pooled_with_baseline(self):
        rows = [trial_summary(self.row(batch=batch)) for batch in ("pilot", "followup")]
        self.assertEqual(len(aggregate(rows)), 2)

    def test_html_escapes_failure_text(self):
        row = trial_summary(self.row(harness_error="<script>alert(1)</script>"))
        rendered = render_html([row], aggregate([row]))
        self.assertNotIn("<script>", rendered)
        self.assertIn("&lt;script&gt;", rendered)

    def test_written_findings_are_escaped(self):
        self.assertIn("&lt;script&gt;", render_html([], [], "<script>unsafe</script>"))

    def test_columns_lead_with_uncached_input_output_and_cost_not_total_tokens(self):
        row = trial_summary(self.row(usage_report={"usage": {
            "uncached_input_tokens": 10, "output_tokens": 20, "cache_read_tokens": 5000,
            "total_tokens": 5030, "api_equivalent_cost_usd": 0.03}}))
        columns = list(row)
        self.assertLess(columns.index("uncached_input_tokens"), columns.index("cached_input_tokens"))
        self.assertLess(columns.index("output_tokens"), columns.index("cached_input_tokens"))
        self.assertLess(columns.index("api_equivalent_usd"), columns.index("cached_input_tokens"))
        self.assertLess(columns.index("cached_input_tokens"), columns.index("tokens"))

    def test_blocked_trials_are_excluded_from_pass_rate_and_cost_means_by_default(self):
        rows = [
            trial_summary(self.row(status="blocked", success=False,
                usage_report={"usage": {"uncached_input_tokens": 99999, "api_equivalent_cost_usd": 999}})),
            trial_summary(self.row(status="completed", success=True,
                usage_report={"usage": {"uncached_input_tokens": 10, "api_equivalent_cost_usd": 0.01}})),
        ]
        group = aggregate(rows)[0]
        self.assertEqual(group["trials"], 1)
        self.assertEqual(group["successes"], 1)
        self.assertEqual(group["blocked_trials"], 1)
        self.assertEqual(group["mean_uncached_input_tokens"], 10)
        self.assertEqual(group["mean_api_equivalent_usd"], 0.01)

    def test_include_blocked_folds_blocked_trials_back_into_the_means(self):
        rows = [
            trial_summary(self.row(status="blocked", success=False,
                usage_report={"usage": {"uncached_input_tokens": 100}})),
            trial_summary(self.row(status="completed", success=True,
                usage_report={"usage": {"uncached_input_tokens": 0}})),
        ]
        group = aggregate(rows, include_blocked=True)[0]
        self.assertEqual(group["trials"], 2)
        self.assertEqual(group["mean_uncached_input_tokens"], 50)

    def test_rendered_table_leads_headers_with_cost_first_columns(self):
        row = trial_summary(self.row())
        rendered = render_html([row], aggregate([row]))
        self.assertLess(rendered.index("Mean uncached input"), rendered.index("Mean cache reads"))
        self.assertLess(rendered.index("Uncached input"), rendered.index("Cache reads"))
        self.assertLess(rendered.index("API-equivalent USD"), rendered.index("Cache reads"))

    def test_rendered_table_reports_blocked_count_separately_from_finished_fraction(self):
        rows = [trial_summary(self.row(status="blocked", success=False)),
                trial_summary(self.row(status="completed", success=True))]
        groups = aggregate(rows)
        rendered = render_html(rows, groups)
        self.assertIn("<td>1/1</td>", rendered)  # 1 finished correctly out of 1 countable trial
        self.assertIn("<td>1</td>", rendered)  # the blocked trial is reported, not silently dropped
        self.assertIn("blocked", rendered)

    def test_accuracy_averages_graded_check_fractions_only(self):
        rows = [trial_summary(self.row(grade={"passed": passed, "total": 10}))
                for passed in (10, 5)] + [trial_summary(self.row())]
        group = aggregate(rows)[0]
        self.assertEqual(group["mean_accuracy"], 0.75)
        self.assertEqual((group["graded_trials"], group["full_passes"], group["trials"]), (2, 1, 3))

    def test_by_case_splits_each_condition(self):
        rows = [trial_summary(self.row(case=case, grade={"passed": passed, "total": 4}))
                for case, passed in (("ledger", 4), ("semver", 1))]
        groups = aggregate(rows, by_case=True)
        self.assertEqual([(g["case"], g["mean_accuracy"]) for g in groups], [("ledger", 1.0), ("semver", 0.25)])
        self.assertNotIn("case", aggregate(rows)[0])

    def test_markdown_keeps_unknowns_and_escapes_cells(self):
        rows = [trial_summary(self.row(condition="native|x"))]
        rendered = render_markdown(aggregate(rows), aggregate(rows, by_case=True))
        self.assertIn("native\\|x", rendered)
        self.assertIn("| unknown | 0/0 |", rendered)


if __name__ == "__main__":
    unittest.main()
