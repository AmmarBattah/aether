"""Offline pricing and re-pricing: injected inventory, no daemon or model calls."""

import contextlib
import copy
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from harness_bench_pricing import resolve_price, validate_price_table
from harness_bench_reprice import reprice_manifest
from harness_bench_report import aggregate, render_html, trial_summary
from harness_bench_usage import normalize_usage, parse_aether_events, parse_codex_jsonl

SHA = "a" * 64


def inventory():
    def row(provider, model, name, cost=None):
        return {"model": {"provider": provider, "model": model}, "name": name,
                "capabilities": {"cost": cost}}
    rates = dict(input_per_mtok=2, output_per_mtok=10, cache_read_per_mtok=0.5, cache_write_per_mtok=3)
    return {"binary_sha256": SHA, "models": [
        row("openai", "gpt-test", "GPT Test", rates),
        row("anthropic", "claude-test", "Claude Test", rates),
        row("claude-code", "test-alias", "Claude Test"),
    ]}


def events(*, provider="codex", model="gpt-test", **cost):
    def event(seq, kind, **fields):
        return {"seq": seq, "agent_id": "root", "kind": {"type": kind, **fields}}
    return [
        event(1, "agent.created", model={"provider": provider, "model": "default"}),
        event(2, "agent.started", turn=1),
        event(3, "custom", name="engine.session", payload={"model": model}),
        event(4, "agent.usage_updated", turn_usage={
            "input_tokens": 100, "output_tokens": 20, "cache_read_tokens": 40,
            "cache_write_tokens": 10, "cost_usd": 0, "cost_complete": False, **cost}),
        event(5, "agent.turn_completed", turn=1),
    ]


def codex_jsonl(**usage):
    counters = {"input_tokens": 150, "output_tokens": 20, "cached_input_tokens": 40,
                "cache_write_input_tokens": 10, **usage}
    return json.dumps({"type": "turn.started"}) + "\n" + json.dumps({"type": "turn.completed", "usage": counters}) + "\n"


class PricingTests(unittest.TestCase):
    def test_runtime_estimate_replaces_unknown_and_counts_as_priced(self):
        raw = events(estimated_cost_usd=1.25)[3]["kind"]["turn_usage"]
        self.assertEqual(normalize_usage(raw, "aether")["api_equivalent_cost_usd"], 1.25)
        report = parse_aether_events(events(estimated_cost_usd=1.25), "root")
        self.assertEqual(report["units"][0]["cost_basis"], "estimated")
        self.assertEqual(report["coverage"]["api_equivalent_cost_usd"], {
            "reported": 1, "terminal": 1, "units": 1, "complete": True,
            "billed": 0, "estimated": 1, "unpriced": 0})
        self.assertNotIn("unpriced work", " ".join(report["warnings"]))

    def test_runtime_mixed_invocations_keep_billed_plus_estimated_dollars(self):
        report = parse_aether_events(events(cost_usd=2, estimated_cost_usd=1), "root")
        self.assertEqual(report["usage"]["api_equivalent_cost_usd"], 3)
        self.assertEqual(report["units"][0]["cost_basis"], "estimated")

    def test_billed_zero_and_estimated_zero_are_known_not_unpriced(self):
        for cost, basis in [({"cost_complete": True}, "billed"), ({"estimated_cost_usd": 0}, "estimated")]:
            with self.subTest(basis=basis):
                report = parse_aether_events(events(**cost), "root")
                self.assertEqual(report["usage"]["api_equivalent_cost_usd"], 0)
                self.assertEqual(report["coverage"]["api_equivalent_cost_usd"][basis], 1)

    def test_session_model_prices_all_four_counters_and_rollups(self):
        report = parse_aether_events(events(), "root", price_table=inventory())
        self.assertAlmostEqual(report["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertEqual(report["root_usage"], report["usage"])
        self.assertEqual(report["units"][0]["priced_model"], {"provider": "openai", "model": "gpt-test"})
        self.assertTrue(report["coverage"]["api_equivalent_cost_usd"]["complete"])

    def test_claude_engine_alias_joins_unique_api_display_name(self):
        table = inventory()
        report = parse_aether_events(events(provider="claude-code", model="test-alias"), "root", price_table=table)
        self.assertAlmostEqual(report["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertEqual(resolve_price(table, "claude-code", "claude-test")["model"]["provider"], "anthropic")
        table["models"][1]["aliases"] = ["explicit-alias"]
        self.assertIsNotNone(resolve_price(table, "claude-code", "explicit-alias"))

    def test_ambiguous_alias_and_default_remain_unknown(self):
        table = inventory()
        duplicate = copy.deepcopy(table["models"][1])
        duplicate["model"]["model"] = "different-generation"
        table["models"].append(duplicate)
        self.assertIsNone(resolve_price(table, "claude-code", "test-alias"))
        for model in ("default", "not-listed"):
            report = parse_aether_events(events(model=model), "root", price_table=table)
            self.assertIsNone(report["usage"]["api_equivalent_cost_usd"])
            self.assertEqual(report["coverage"]["api_equivalent_cost_usd"]["unpriced"], 1)

    def test_runtime_price_is_not_overwritten_by_posthoc_table(self):
        for cost in ({"cost_usd": 4, "cost_complete": True}, {"estimated_cost_usd": 4}):
            report = parse_aether_events(events(**cost), "root", price_table=inventory())
            self.assertEqual(report["usage"]["api_equivalent_cost_usd"], 4)
            self.assertNotIn("priced_model", report["units"][0])

    def test_multiple_session_models_in_one_turn_remain_unpriced(self):
        capture = events()
        capture.insert(3, {"seq": 10, "agent_id": "root", "kind": {
            "type": "custom", "name": "engine.session", "payload": {"model": "different"}}})
        for seq, event in enumerate(capture):
            event["seq"] = seq
        report = parse_aether_events(capture, "root", price_table=inventory())
        self.assertIsNone(report["usage"]["api_equivalent_cost_usd"])
        self.assertEqual(report["units"][0]["session_models"], ["gpt-test", "different"])

    def test_session_models_are_scoped_to_turn_not_final_agent_identity(self):
        capture = events()
        second = events(model="not-listed")[1:]
        for item in second:
            item["seq"] += 10
            if "turn" in item["kind"]:
                item["kind"]["turn"] = 2
        report = parse_aether_events(capture + second, "root", price_table=inventory())
        self.assertAlmostEqual(report["units"][0]["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertIsNone(report["units"][1]["usage"]["api_equivalent_cost_usd"])
        self.assertFalse(report["coverage"]["api_equivalent_cost_usd"]["complete"])

    def test_native_codex_prices_inclusive_input_without_double_counting_writes(self):
        report = parse_codex_jsonl(codex_jsonl(), price_table=inventory(), model="gpt-test")
        self.assertEqual(report["usage"]["uncached_input_tokens"], 100)
        self.assertEqual(report["usage"]["cache_write_tokens"], 10)
        self.assertEqual(report["usage"]["total_tokens"], 170)
        self.assertAlmostEqual(report["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertEqual(report["units"][0]["cost_basis"], "estimated (configured default model)")

    def test_native_codex_cache_writes_are_priced_as_disjoint_from_cache_reads(self):
        """Documents the assumption that reads and writes do not overlap in input."""
        report = parse_codex_jsonl(codex_jsonl(cache_write_input_tokens=50),
                                   price_table=inventory(), model="gpt-test")
        usage = report["usage"]
        self.assertEqual(usage["uncached_input_tokens"], 150 - 40 - 50)
        self.assertEqual(usage["cache_write_tokens"], 50)
        self.assertEqual(usage["input_tokens"], 150)
        # Overlapping writes would instead leave 110 uncached tokens at input rate.
        self.assertAlmostEqual(usage["api_equivalent_cost_usd"],
                               60 * 2e-6 + 20 * 10e-6 + 40 * 0.5e-6 + 50 * 3e-6)

    def test_native_codex_records_its_model_basis_without_changing_pricing(self):
        for basis in ("resolved model", "configured default model"):
            with self.subTest(basis=basis):
                report = parse_codex_jsonl(codex_jsonl(), price_table=inventory(),
                                           model="gpt-test", model_basis=basis)
                self.assertEqual(report["units"][0]["cost_basis"], f"estimated ({basis})")
                self.assertAlmostEqual(report["usage"]["api_equivalent_cost_usd"], 0.00045)
        # The inclusive-input requirement is keyed off the keyword, not the label:
        # a free cache write would otherwise price unknown writes as uncached input.
        free_writes = inventory()
        free_writes["models"][0]["capabilities"]["cost"]["cache_write_per_mtok"] = 0
        blind = parse_codex_jsonl(codex_jsonl(cache_write_input_tokens=None),
                                  price_table=free_writes, model="gpt-test", model_basis="resolved model")
        self.assertIsNone(blind["usage"]["api_equivalent_cost_usd"])
        with self.assertRaisesRegex(ValueError, "basis must be nonempty"):
            parse_codex_jsonl(codex_jsonl(), price_table=inventory(), model="gpt-test", model_basis="")

    def test_partial_billed_floor_is_retained_when_list_price_replaces_it(self):
        report = parse_aether_events(events(cost_usd=0.1), "root", price_table=inventory())
        unit = report["units"][0]
        self.assertEqual(unit["billed_floor_usd"], 0.1)
        self.assertAlmostEqual(unit["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertEqual(unit["cost_basis"], "estimated")
        self.assertEqual(report["coverage"]["api_equivalent_cost_usd"]["estimated"], 1)
        without_floor = parse_aether_events(events(), "root", price_table=inventory())["units"][0]
        self.assertNotIn("billed_floor_usd", without_floor)

    def test_native_codex_missing_model_or_required_counter_stays_unknown(self):
        for model, text in [(None, codex_jsonl()), ("gpt-test", codex_jsonl(cache_write_input_tokens=None))]:
            report = parse_codex_jsonl(text, price_table=inventory(), model=model)
            self.assertIsNone(report["usage"]["api_equivalent_cost_usd"])
            self.assertEqual(report["coverage"]["api_equivalent_cost_usd"]["unpriced"], 1)
        with self.assertRaisesRegex(ValueError, "exceeds inclusive input"):
            parse_codex_jsonl(codex_jsonl(cache_write_input_tokens=111))

    def test_zero_cache_rate_needs_no_counter_but_missing_work_is_not_free(self):
        table = inventory()
        table["models"][0]["capabilities"]["cost"]["cache_write_per_mtok"] = 0
        capture = events()
        del capture[3]["kind"]["turn_usage"]["cache_write_tokens"]
        report = parse_aether_events(capture, "root", price_table=table)
        self.assertAlmostEqual(report["usage"]["api_equivalent_cost_usd"], 0.00042)
        self.assertIsNone(report["usage"]["cache_write_tokens"])
        table["models"][0]["capabilities"]["cost"] = dict.fromkeys(
            table["models"][0]["capabilities"]["cost"], 0)
        report = parse_codex_jsonl('', price_table=table, model="gpt-test")
        self.assertIsNone(report["usage"]["api_equivalent_cost_usd"])

    def test_invalid_prices_and_runtime_estimates_fail_visibly(self):
        for value in (-1, True, float("nan"), float("inf"), "2"):
            table = inventory()
            table["models"][0]["capabilities"]["cost"]["input_per_mtok"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_price_table(table)
            with self.subTest(estimate=value), self.assertRaises(ValueError):
                parse_aether_events(events(estimated_cost_usd=value), "root")

    def test_report_excludes_partial_trials_from_cost_mean_and_shows_basis(self):
        mixed = events(estimated_cost_usd=1)
        child = events(provider="claude-code", cost_usd=2, cost_complete=True)
        for e in child:
            e["seq"] += 10
            e["agent_id"] = "child"
        child[0]["kind"]["parent"] = "root"
        report = parse_aether_events(mixed + child, "root")
        row = {"condition": "aether-coordinator-team", "case": "test", "run": 1, "trial_dir": "."}
        priced = trial_summary({**row, "usage_report": report})
        partial = trial_summary({**row, "usage_report": parse_aether_events(events(cost_usd=99), "root")})
        groups = aggregate([priced, partial])
        self.assertEqual(priced["cost_basis"], "mixed")
        self.assertEqual(groups[0]["mean_api_equivalent_usd"], 3)
        self.assertEqual(groups[0]["api_equivalent_usd_trials"], 1)
        html = render_html([priced, partial], groups)
        self.assertIn("1/2", html)
        self.assertIn("billed / estimated / unpriced", html)


class RepriceTests(unittest.TestCase):
    def setUp(self):
        scratch = Path(__file__).parent.parent / "target" / "pricing-tests"
        scratch.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def manifest(self):
        (self.root / "events.json").write_text(json.dumps(events()))
        (self.root / "native-stdout.jsonl").write_text(codex_jsonl())
        rows = [{"condition": condition, "root_agent_id": "root", "trial_dir": str(self.root),
                 "usage_report": {"usage": {"api_equivalent_cost_usd": 999}}}
                for condition in ("aether-single-codex", "native-codex")]
        manifest = {"binary": {"sha256": SHA}, "price_table": inventory(), "trials": rows,
                    "native": {"codex": {"configured_defaults": {"model": "gpt-test"}}}}
        path = self.root / "manifest.json"
        path.write_text(json.dumps(manifest))
        return path

    def test_reprice_rebuilds_raw_evidence_and_never_touches_original_or_trials(self):
        path = self.manifest()
        before = {p: p.read_bytes() for p in self.root.iterdir()}
        output = reprice_manifest(path)
        result = json.loads(output.read_text())
        self.assertEqual(output.name, "manifest.repriced.json")
        for row in result["trials"]:
            self.assertAlmostEqual(row["usage_report"]["usage"]["api_equivalent_cost_usd"], 0.00045)
        self.assertEqual(before, {p: p.read_bytes() for p in before})
        with self.assertRaises(FileExistsError):
            reprice_manifest(path)

    def test_missing_capture_replaces_stale_usage_with_visible_error(self):
        path = self.manifest()
        (self.root / "events.json").unlink()
        result = json.loads(reprice_manifest(path).read_text())
        self.assertNotIn("usage_report", result["trials"][0])
        self.assertIn("re-pricing failed", result["trials"][0]["usage_error"])
        self.assertIn("usage_report", result["trials"][1])

    def test_injected_table_and_output_directory(self):
        path = self.manifest()
        manifest = json.loads(path.read_text())
        del manifest["price_table"]
        del manifest["native"]
        path.write_text(json.dumps(manifest))
        output_dir = self.root / "out"
        output_dir.mkdir()
        output = reprice_manifest(path, price_table=inventory(), output_dir=output_dir)
        result = json.loads(output.read_text())
        self.assertEqual(output.parent, output_dir)
        self.assertIsNone(result["trials"][1]["usage_report"]["usage"]["api_equivalent_cost_usd"])
        self.assertEqual(result["price_table"]["binary_sha256"], SHA)

    def test_price_capture_never_inherits_live_routing_and_stops_owned_daemon(self):
        import harness_bench as hb
        binary = self.root / "aether"
        binary.write_bytes(b"pinned fixture")
        digest = hb.sha256_file(binary)
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.root),
                                     "AETHER_SOCKET": "/operator/live.sock", "AETHER_ADDRESS": "live:123"}), \
                patch.object(hb.tempfile, "TemporaryDirectory") as sockets, \
                patch.object(hb, "start_daemon") as start, patch.object(hb, "stop_daemon", return_value=None) as stop, \
                patch.object(hb.subprocess, "run") as run:
            sockets.return_value.__enter__.return_value = "/fake-hbp"
            run.return_value.stdout = json.dumps(inventory()["models"])
            table = hb.fetch_price_table(str(binary), digest, self.root)
        env = start.call_args.args[1]
        self.assertEqual(env["AETHER_SOCKET"], "/fake-hbp/aetherd.sock")
        self.assertNotIn("AETHER_ADDRESS", env)
        self.assertEqual(run.call_args.args[0][-2:], ["--socket", env["AETHER_SOCKET"]])
        self.assertTrue(Path(env["AETHER_CONFIG_DIR"]).resolve().is_relative_to(self.root.resolve()))
        self.assertEqual(table["binary_sha256"], digest)
        stop.assert_called_once_with(str(binary), env, start.return_value)
        self.assertEqual(list(self.root.glob("prices-*")), [], "successful capture must leave no scratch dir")

    def test_price_capture_cleanup_keeps_original_failure(self):
        import harness_bench as hb
        binary = self.root / "aether"
        binary.write_bytes(b"pinned fixture")
        with patch.dict(os.environ, {"XDG_RUNTIME_DIR": str(self.root)}), \
                patch.object(hb.tempfile, "TemporaryDirectory") as sockets, \
                patch.object(hb, "start_daemon"), patch.object(hb, "stop_daemon", return_value="stop failed") as stop, \
                patch.object(hb.subprocess, "run", side_effect=RuntimeError("capture failed")):
            sockets.return_value.__enter__.return_value = "/fake-hbp"
            with self.assertRaisesRegex(RuntimeError, "capture failed") as raised:
                hb.fetch_price_table(str(binary), hb.sha256_file(binary), self.root)
        stop.assert_called_once()
        self.assertIn("stop failed", str(raised.exception.__notes__))
        self.assertTrue(list(self.root.glob("prices-*")), "failed capture must keep its scratch dir")

    def test_native_codex_prefers_the_trial_resolved_model_over_the_config_default(self):
        path = self.manifest()
        manifest = json.loads(path.read_text())
        row = manifest["trials"][1]
        row["resolved_metadata"] = [{"source": "native rollout turn_context", "model": "gpt-test"}]
        manifest["native"]["codex"]["configured_defaults"]["model"] = "not-listed"
        path.write_text(json.dumps(manifest))
        result = json.loads(reprice_manifest(path).read_text())["trials"][1]
        self.assertEqual(result["usage_report"]["units"][0]["cost_basis"], "estimated (resolved model)")
        self.assertAlmostEqual(result["usage_report"]["usage"]["api_equivalent_cost_usd"], 0.00045)

    def test_explicit_native_treatments_rebuild_from_raw_receipts(self):
        from harness_bench_reprice import rebuild_trial_usage
        path = self.manifest()
        manifest = json.loads(path.read_text())
        row = manifest["trials"][1]
        row.update(condition="native-terra-low",
                   requested_treatment={"kind": "native-codex", "model": "gpt-test", "effort": "low"},
                   resolved_metadata=[{"model": "gpt-test", "effort": "low"}])
        rebuild_trial_usage(row, manifest)
        self.assertNotIn("usage_error", row)
        self.assertAlmostEqual(row["usage_report"]["usage"]["api_equivalent_cost_usd"], 0.00045)
        row.pop("resolved_metadata")
        rebuild_trial_usage(row, manifest)
        self.assertIsNone(row["usage_report"]["usage"]["api_equivalent_cost_usd"])
        (self.root / "native-stdout.jsonl").write_text(json.dumps({
            "type": "result", "subtype": "success", "is_error": False,
            "total_cost_usd": 0.1,
            "usage": {"input_tokens": 10, "output_tokens": 20}}))
        row.update(condition="native-sonnet-low",
                   requested_treatment={"kind": "native-claude", "model": "sonnet", "effort": "low"})
        rebuild_trial_usage(row, manifest)
        self.assertNotIn("usage_error", row)
        self.assertEqual(row["usage_report"]["usage"]["output_tokens"], 20)

    def test_native_codex_falls_back_to_config_default_and_never_guesses_between_models(self):
        cases = [(None, "estimated (configured default model)", 0.00045),
                 ([{"model": "gpt-test"}, {"model": "other-model"}], None, None)]
        for metadata, basis, cost in cases:
            with self.subTest(metadata=metadata):
                path = self.manifest()
                manifest = json.loads(path.read_text())
                if metadata is not None:
                    manifest["trials"][1]["resolved_metadata"] = metadata
                path.write_text(json.dumps(manifest))
                unit = json.loads(reprice_manifest(path).read_text())["trials"][1]["usage_report"]["units"][0]
                self.assertEqual(unit["cost_basis"], basis or "unpriced")
                if cost is None:
                    self.assertIsNone(unit["usage"]["api_equivalent_cost_usd"])
                else:
                    self.assertAlmostEqual(unit["usage"]["api_equivalent_cost_usd"], cost)
                path.with_suffix(".repriced.json").unlink()

    def test_reconciliation_failure_is_separate_and_clears_stale_keys(self):
        path = self.manifest()
        manifest = json.loads(path.read_text())
        (self.root / "claude-stdout.jsonl").write_text("")
        good = {"condition": "native-claude", "trial_dir": str(self.root),
                "raw_logs": {"stdout": str(self.root / "claude-stdout.jsonl")},
                "claude_receipts": [{"capture_id": "p1", "receipt": None}],
                "claude_reconciliation_error": "stale error from the source manifest"}
        bad = {**good, "claude_receipts": [{"receipt": None}]}
        manifest["trials"] = [good, bad]
        path.write_text(json.dumps(manifest))
        repriced, failed = json.loads(reprice_manifest(path).read_text())["trials"]
        self.assertNotIn("claude_reconciliation_error", repriced)
        self.assertEqual(repriced["claude_reconciliation"]["unattributed_processes"], 1)
        self.assertIn("re-reconciliation failed", failed["claude_reconciliation_error"])
        self.assertNotIn("usage_error", failed)
        self.assertIn("usage_report", failed)

    def test_scratch_dir_requires_binary(self):
        from harness_bench_reprice import main
        stderr = io.StringIO()
        with self.assertRaises(SystemExit) as raised, contextlib.redirect_stderr(stderr):
            main([str(self.root / "manifest.json"), "--scratch-dir", str(self.root)])
        self.assertEqual(raised.exception.code, 2)
        self.assertIn("--scratch-dir is only used with --binary", stderr.getvalue())

    def test_wrong_binary_sha_rejected_before_output(self):
        path = self.manifest()
        table = inventory()
        table["binary_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "does not match"):
            reprice_manifest(path, price_table=table)
        self.assertFalse(path.with_suffix(".repriced.json").exists())


if __name__ == "__main__":
    unittest.main()
