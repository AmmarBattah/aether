"""Overhead-diagnosis arms: one change each, launchable only as diagnostics, never countable."""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

import harness_bench as hb
import harness_bench_compare as compare
import harness_bench_report as report
from harness_bench_config import SOLO_SUFFIX
from harness_bench_treatments import (
    AETHER_CODEX_FEATURES_OFF, AETHER_CODEX_POLICY, APPROVAL_NEVER_PATCH, CODEX_BYPASS_FLAG,
    COMPLETE_TASK_REQUEST, DIAGNOSTIC_PREFIX, LUNA_REPEAT, NATIVE_COORDINATION_TOOLS, binary_patches,
    diagnostic_conditions,
    is_diagnostic, is_native_trial,
    loads_user_mcp, lower_tier_conditions, luna_diagnostic_conditions, native_command, native_prompt,
)

ENVELOPE = "diag-native-haiku-low-envelope"
NO_EFFORT = "diag-native-haiku-low-envelope-noeffort"
MINIMAL = "diag-aether-haiku-low-minimal"
NO_COORDINATION = "diag-native-haiku-low-nocoord"
NO_COORDINATION_CONNECTORS = "diag-native-haiku-low-nocoord-connectors"
AETHER_CONNECTORS = "diag-aether-haiku-low-connectors"
COMPLETE_TASK = "diag-aether-haiku-low-complete-task"
LUNA_ON_REQUEST = "diag-native-luna-low-onrequest"
LUNA_NO_MULTI_AGENT = "diag-native-luna-low-nomultiagent"
LUNA_ENVELOPE = "diag-native-luna-low-envelope"
LUNA_APPROVAL_NEVER = "diag-aether-luna-low-approval-never"
LUNA_ARMS = [LUNA_ON_REQUEST, LUNA_NO_MULTI_AGENT, LUNA_ENVELOPE, LUNA_APPROVAL_NEVER, LUNA_REPEAT]
YARDSTICK_BLOCK = ["native-luna-low", LUNA_REPEAT, "aether-single-luna-low"]
ARMS = [ENVELOPE, NO_EFFORT, MINIMAL, NO_COORDINATION, NO_COORDINATION_CONNECTORS, AETHER_CONNECTORS, COMPLETE_TASK,
        *LUNA_ARMS]
ENGINE_SOURCE = Path(__file__).resolve().parent.parent / "crates" / "aether-engine" / "src" / "claude_code.rs"
CODEX_ENGINE_SOURCE = ENGINE_SOURCE.with_name("codex_app.rs")
TASK = "Repair the book." + SOLO_SUFFIX
WORKSPACE = "/trial/workspace"


def _native_trial(key):
    """The record of a native cell's launch, without starting it."""
    def fake(command, cwd, timeout, stdout_path, stderr_path, env):
        Path(stdout_path).write_text("")
        return {"seconds": 1.0, "exit_code": 0, "timed_out": False, "cleanup_error": None}
    result = {}
    with tempfile.TemporaryDirectory() as tmp, \
            patch.object(hb, "run_subprocess_capturing", side_effect=fake), \
            patch.object(hb, "native_configured_defaults", return_value={}), \
            patch.object(hb, "native_resolved_metadata", return_value=[]):
        hb.run_native_trial(hb.CONDITIONS[key], TASK, WORKSPACE, 5, tmp, result)
    return result


def _native_command(key):
    return _native_trial(key)["command"]


class DiagnosticArmTests(unittest.TestCase):
    def test_the_arms_exist_and_only_under_the_prefix(self):
        self.assertEqual(sorted(diagnostic_conditions(SOLO_SUFFIX)), sorted(ARMS))
        self.assertEqual([key for key in hb.CONDITIONS if is_diagnostic(key)], ARMS)
        self.assertEqual(DIAGNOSTIC_PREFIX, "diag-")

    def test_the_no_coordination_arm_differs_from_counted_native_only_in_the_disallowed_tools(self):
        counted, arm = _native_command("native-haiku-low"), _native_command(NO_COORDINATION)
        flag = arm.index("--disallowedTools")
        self.assertEqual(arm[:flag] + arm[flag + 2:], counted)
        self.assertEqual(arm[flag + 1], NATIVE_COORDINATION_TOOLS)
        self.assertTrue(arm[flag + 2].startswith("--"), "another option ends the tool list before the prompt")
        self.assertEqual(arm[-1], TASK)

    def test_the_disallowed_tools_are_the_ones_aether_passes(self):
        source = ENGINE_SOURCE.read_text()
        self.assertEqual(source.count(f'const NATIVE_COORDINATION_TOOLS: &str = "{NATIVE_COORDINATION_TOOLS}";'), 1)
        self.assertEqual(len(set(NATIVE_COORDINATION_TOOLS.split(","))), 10)

    def test_the_connectors_arm_differs_from_the_no_coordination_arm_only_in_the_strict_flag(self):
        strict, loose = _native_trial(NO_COORDINATION), _native_trial(NO_COORDINATION_CONNECTORS)
        self.assertEqual([part for part in strict["command"] if part != "--strict-mcp-config"], loose["command"])
        self.assertEqual(strict["command"].count("--strict-mcp-config"), 1)
        self.assertEqual(strict["permission_overrides"], ["--dangerously-skip-permissions", "--strict-mcp-config"])
        self.assertEqual(loose["permission_overrides"], ["--dangerously-skip-permissions"])
        self.assertEqual(_native_trial("native-haiku-low")["permission_overrides"], hb.CLAUDE_PERMISSION_OVERRIDES)
        self.assertEqual(_native_trial("native-luna-low")["permission_overrides"], hb.CODEX_PERMISSION_OVERRIDES)

    def test_only_the_two_connector_arms_load_the_users_mcp_servers(self):
        loading = [key for key, condition in hb.CONDITIONS.items() if loads_user_mcp(condition)]
        self.assertEqual(loading, [NO_COORDINATION_CONNECTORS, AETHER_CONNECTORS])
        with self.assertRaisesRegex(AssertionError, "only a Claude command"):
            native_command({**hb.CONDITIONS["native-luna-low"], "user_mcp": True}, ["codex", "exec", TASK])
        with self.assertRaisesRegex(AssertionError, "only Claude Code"):
            native_command({**hb.CONDITIONS["native-luna-low"], "disallowed_tools": "Agent"}, ["codex", "exec", TASK])

    def test_the_aether_connectors_arm_differs_from_counted_aether_only_in_the_mcp_set(self):
        counted, arm = hb.CONDITIONS["aether-single-haiku-low"], dict(hb.CONDITIONS[AETHER_CONNECTORS])
        self.assertIs(arm.pop("user_mcp"), True)
        self.assertEqual(arm, counted)
        asked = {}
        for key in ("aether-single-haiku-low", AETHER_CONNECTORS, MINIMAL, "aether-single-luna-low"):
            with patch.object(hb, "install_claude_receipt_proxy", side_effect=RuntimeError("stop")) as install, \
                    self.assertRaisesRegex(RuntimeError, "stop"):
                hb._run_isolated_aether_trial("aether", {}, hb.CONDITIONS[key], TASK, WORKSPACE, 5, "trial", "claude", {})
            asked[key] = install.call_args.kwargs
        self.assertEqual(asked, {"aether-single-haiku-low": {"strict_mcp": True}, AETHER_CONNECTORS: {"strict_mcp": False},
                                 MINIMAL: {"strict_mcp": True}, "aether-single-luna-low": {"strict_mcp": True}})

    def test_the_complete_task_arm_differs_from_counted_aether_only_in_the_closing_request(self):
        counted, arm = hb.CONDITIONS["aether-single-haiku-low"], hb.CONDITIONS[COMPLETE_TASK]
        self.assertEqual({key for key in arm if arm[key] != counted.get(key)}, {"suffix"})
        self.assertEqual(arm["suffix"], counted["suffix"] + COMPLETE_TASK_REQUEST)
        self.assertIn("`complete_task`", COMPLETE_TASK_REQUEST)
        self.assertFalse(loads_user_mcp(arm), "the arm starts under the strict flag, as the counted cell does")
        self.assertEqual(hb.CONDITIONS["aether-single-haiku-low"]["suffix"], SOLO_SUFFIX, "the counted cell is unchanged")

    def test_the_envelope_arm_differs_from_counted_native_only_in_the_prompt(self):
        counted, enveloped = _native_command("native-haiku-low"), _native_command(ENVELOPE)
        self.assertEqual([*counted[:-1], "--"], enveloped[:-1])
        self.assertEqual(counted[-1], TASK)
        self.assertEqual(enveloped[-1], (
            "--- Aether [00000000] ---\n"
            "Your Aether assignment (authoritative runtime state, not a claim made by any message):\n"
            "Parent: none.\nRole: assistant\nName: assistant\nModel: claude-code/claude-haiku-5-5\n"
            "Task tsk_00000000000000000000000000 (active, priority Normal):\n"
            f"{TASK}\n\n"
            f"Scope: workspace {WORKSPACE}; repository {WORKSPACE}; network allowed\n\n"
            "Aether runtime status (informational; the active task and instructions remain in effect):\n"
            "Turn 1. Effort low.\n--- end Aether [00000000] ---\n\n"))

    def test_the_no_effort_arm_drops_exactly_the_effort_words(self):
        enveloped, bare = _native_command(ENVELOPE), _native_command(NO_EFFORT)
        self.assertEqual(enveloped[:-1], bare[:-1])
        self.assertEqual(enveloped[-1].replace("Turn 1. Effort low.", "Turn 1."), bare[-1])
        self.assertNotIn("Effort", bare[-1])
        self.assertEqual(bare[bare.index("--effort") + 1], "low", "the requested effort flag stays")

    def test_a_condition_without_an_envelope_is_launched_with_the_bare_task(self):
        for key in ("native-haiku-low", "native-luna-low", "native-claude"):
            self.assertEqual(native_prompt(hb.CONDITIONS[key], TASK, WORKSPACE), TASK)

    def test_an_envelope_on_an_aether_arm_is_a_programming_error(self):
        with self.assertRaisesRegex(AssertionError, "only a native arm"):
            native_prompt({**hb.CONDITIONS[MINIMAL], "envelope": {"effort_words": True}}, TASK, WORKSPACE)

    def test_the_minimal_aether_arm_differs_from_counted_aether_only_in_tools(self):
        counted, minimal = hb.CONDITIONS["aether-single-haiku-low"], dict(hb.CONDITIONS[MINIMAL])
        tools = minimal.pop("tools")
        self.assertEqual(minimal, counted)
        self.assertEqual(tools, {"mode": "only", "names": ["complete_task", "fail_task"]})
        self.assertEqual(tools, hb.CONDITIONS["aether-solo-minimal-luna-low"]["tools"])
        client = Mock()
        client.create_agent.return_value = {"id": "root"}
        self.assertEqual(hb._spawn_root(client, hb.CONDITIONS[MINIMAL], TASK), ("root", "low"))
        client.create_agent.assert_called_once_with(
            role="assistant", objective=TASK, model="claude-code/claude-haiku-5-5", long_lived=False,
            effort="low", tools=tools)

    def test_a_prompt_that_opens_with_dashes_is_kept_from_the_option_parser(self):
        for key in (ENVELOPE, NO_EFFORT):
            command = _native_command(key)
            self.assertTrue(command[-1].startswith("--- Aether ["))
            self.assertEqual(command[-2], "--")
            self.assertEqual(command.count("--"), 1)
        self.assertNotIn("--", _native_command("native-haiku-low"))
        self.assertNotIn("--", _native_command("native-luna-low"))


class LunaDiagnosticArmTests(unittest.TestCase):
    def test_the_luna_arms_exist(self):
        self.assertEqual(list(luna_diagnostic_conditions(SOLO_SUFFIX)), LUNA_ARMS)
        self.assertTrue(all(is_diagnostic(key) for key in LUNA_ARMS))
        self.assertEqual(hb.CONDITIONS["native-luna-low"], {
            "kind": "native-codex", "model": "gpt-6-luna", "effort": "low", "suffix": SOLO_SUFFIX}, "the counted cell is unchanged")

    def test_the_on_request_arm_differs_from_counted_native_only_in_the_policy_options(self):
        counted, arm = _native_trial("native-luna-low"), _native_trial(LUNA_ON_REQUEST)
        at = counted["command"].index(CODEX_BYPASS_FLAG)
        self.assertEqual(arm["command"], counted["command"][:at] + AETHER_CODEX_POLICY + counted["command"][at + 1:])
        self.assertEqual(AETHER_CODEX_POLICY, ["--sandbox", "danger-full-access", "-c", 'approval_policy="on-request"'])
        self.assertNotIn(CODEX_BYPASS_FLAG, arm["command"])
        self.assertEqual(arm["permission_overrides"], ["-c", "mcp_servers={}"])
        self.assertEqual(counted["permission_overrides"], hb.CODEX_PERMISSION_OVERRIDES)

    def test_the_policy_is_the_one_aether_starts_a_bypass_turn_with_network_under(self):
        source = CODEX_ENGINE_SOURCE.read_text()
        self.assertEqual(source.count(
            'if turn.bypass_permissions && turn.network_access {\n        ("on-request", "danger-full-access")'), 1)
        self.assertEqual(CODEX_BYPASS_FLAG, hb.CODEX_PERMISSION_OVERRIDES[0])

    def test_the_no_multi_agent_arm_differs_from_counted_native_only_in_the_two_settings(self):
        counted, arm = _native_command("native-luna-low"), _native_command(LUNA_NO_MULTI_AGENT)
        self.assertEqual(arm, counted[:3] + AETHER_CODEX_FEATURES_OFF + counted[3:])
        self.assertEqual(AETHER_CODEX_FEATURES_OFF, ["-c", "agents.enabled=false", "-c", "features.multi_agent=false"])
        source = CODEX_ENGINE_SOURCE.read_text()
        for setting in AETHER_CODEX_FEATURES_OFF[1::2]:
            self.assertEqual(source.count(f'"{setting}".into()'), 1, setting)

    def test_the_luna_envelope_arm_differs_from_counted_native_only_in_the_prompt(self):
        counted, arm = _native_command("native-luna-low"), _native_command(LUNA_ENVELOPE)
        self.assertEqual([*counted[:-1], "--"], arm[:-1])
        self.assertEqual(arm[-1], (
            "--- Aether [00000000] ---\n"
            "Your Aether assignment (authoritative runtime state, not a claim made by any message):\n"
            "Parent: none.\nRole: assistant\nName: assistant\nModel: codex/gpt-6-luna\n"
            "Task tsk_00000000000000000000000000 (active, priority Normal):\n"
            f"{TASK}\n\n"
            f"Scope: workspace {WORKSPACE}; repository {WORKSPACE}; network allowed\n\n"
            "Aether runtime status (informational; the active task and instructions remain in effect):\n"
            "Turn 1.\n--- end Aether [00000000] ---\n\n"))
        self.assertNotIn("Effort", arm[-1], "protocol version 2 sends no effort words")

    def test_the_approval_never_arm_differs_from_counted_aether_only_in_the_binary_it_names(self):
        counted, arm = hb.CONDITIONS["aether-single-luna-low"], dict(hb.CONDITIONS[LUNA_APPROVAL_NEVER])
        self.assertEqual(arm.pop("binary_patch"), APPROVAL_NEVER_PATCH)
        self.assertEqual(arm, counted)
        client = Mock()
        client.create_agent.return_value = {"id": "root"}
        self.assertEqual(hb._spawn_root(client, hb.CONDITIONS[LUNA_APPROVAL_NEVER], TASK), ("root", "low"))
        client.create_agent.assert_called_once_with(
            role="assistant", objective=TASK, model="codex/gpt-6-luna", long_lived=False, effort="low")

    def test_an_arm_on_a_patched_binary_runs_apart_from_other_aether_arms(self):
        natives = f"native-luna-low,{LUNA_ON_REQUEST},{LUNA_NO_MULTI_AGENT},{LUNA_ENVELOPE}"
        with patch.object(sys, "stderr"), self.assertRaises(SystemExit) as refused:
            hb.parse_args(["--conditions", f"aether-single-luna-low,{LUNA_APPROVAL_NEVER}", "--diagnostic"])
        self.assertEqual(refused.exception.code, 2)
        self.assertTrue(hb.parse_args(["--conditions", f"{natives},aether-single-luna-low", "--diagnostic"]).diagnostic)
        self.assertTrue(hb.parse_args(["--conditions", f"{natives},{LUNA_APPROVAL_NEVER}", "--diagnostic"]).diagnostic)
        self.assertEqual(binary_patches([hb.CONDITIONS[key] for key in natives.split(",")]), [])
        self.assertEqual(binary_patches([hb.CONDITIONS["aether-single-luna-low"]]), [""])
        self.assertEqual(binary_patches([hb.CONDITIONS[LUNA_APPROVAL_NEVER]]), [APPROVAL_NEVER_PATCH])
        self.assertEqual([key for key, condition in hb.CONDITIONS.items() if "binary_patch" in condition],
                         [LUNA_APPROVAL_NEVER])

    def test_the_repeat_arm_is_the_counted_native_cell_a_second_time(self):
        self.assertEqual(LUNA_REPEAT, "diag-native-luna-low-repeat")
        self.assertEqual(hb.CONDITIONS[LUNA_REPEAT], hb.CONDITIONS["native-luna-low"])
        counted, repeat = _native_trial("native-luna-low"), _native_trial(LUNA_REPEAT)
        self.assertEqual(repeat["command"], counted["command"])
        self.assertEqual(set(repeat), set(counted))
        self.assertEqual([key for key in counted if repeat[key] != counted[key]], ["raw_logs"],
                         "the launch record differs only in where its own logs lie")
        self.assertEqual(repeat["command"][:2], ["codex", "exec"])

    def test_the_yardstick_block_starts_its_three_arms_together_and_only_as_a_diagnosis(self):
        order = hb.trial_order(["matchbook"], YARDSTICK_BLOCK, 1, 31)
        blocks = hb.paired_blocks(order, 3, 31)
        self.assertEqual([sorted(condition for _case, condition, _run in block) for block in blocks],
                         [sorted(YARDSTICK_BLOCK)])
        with patch.object(sys, "stderr"), self.assertRaises(SystemExit) as refused:
            hb.parse_args(["--conditions", ",".join(YARDSTICK_BLOCK)])
        self.assertEqual(refused.exception.code, 2)
        self.assertTrue(hb.parse_args(["--conditions", ",".join(YARDSTICK_BLOCK), "--diagnostic"]).diagnostic)

    def test_codex_options_on_a_claude_command_are_a_programming_error(self):
        claude = ["claude", "-p", "--output-format", "stream-json", TASK]
        with self.assertRaisesRegex(AssertionError, "only a Codex command carries the bypass flag"):
            native_command({**hb.CONDITIONS["native-haiku-low"], "codex_policy": AETHER_CODEX_POLICY}, claude)
        with self.assertRaisesRegex(AssertionError, "only a Codex command takes Codex settings"):
            native_command({**hb.CONDITIONS["native-haiku-low"], "codex_config": AETHER_CODEX_FEATURES_OFF}, claude)


class DiagnosticAccountingTests(unittest.TestCase):
    def test_a_trial_is_native_by_its_treatment_not_its_name(self):
        for key, condition in hb.CONDITIONS.items():
            row = {"condition": key, "requested_treatment": dict(condition)}
            self.assertEqual(is_native_trial(row), condition["kind"].startswith("native-"), key)
        self.assertTrue(is_native_trial({"condition": "native-claude"}))
        self.assertFalse(is_native_trial({"condition": "aether-single-claude"}))

    def test_a_native_diagnostic_arm_is_accounted_from_its_own_cli_receipt(self):
        from harness_bench_reprice import rebuild_trial_usage
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "native-stdout.jsonl").write_text(json.dumps({
                "type": "result", "subtype": "success", "is_error": False, "total_cost_usd": 0.1,
                "usage": {"input_tokens": 10, "output_tokens": 20}}))
            row = {"condition": ENVELOPE, "requested_treatment": dict(hb.CONDITIONS[ENVELOPE]),
                   "trial_dir": tmp, "short_id": "a", "case": "matchbook", "run": 1}
            rebuild_trial_usage(row, {"price_table": {}})
        self.assertNotIn("usage_error", row)
        self.assertEqual(row["usage_report"]["source"], "claude_print")
        self.assertEqual(row["usage_report"]["usage"]["output_tokens"], 20)
        self.assertEqual(report.accounted_usage(row)["scope"], "native receipt")
        self.assertEqual(report.trial_summary(row)["descendants"], 0)


class DiagnosticsAreNeverCountedTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.tmp = Path(temporary.name)

    def _saved(self, name, **manifest):
        path = self.tmp / name
        path.write_text(json.dumps(manifest))
        return path

    def test_no_diagnostic_arm_is_a_default_or_counted_condition(self):
        self.assertFalse([key for key in hb.DEFAULT_CONDITIONS if is_diagnostic(key)])
        self.assertFalse([key for key in lower_tier_conditions(SOLO_SUFFIX) if is_diagnostic(key)])
        self.assertFalse(any(is_diagnostic(key) for key in hb.parse_args([]).conditions.split(",")))

    def test_a_diagnostic_arm_runs_only_with_the_flag(self):
        with patch.object(sys, "stderr"), self.assertRaises(SystemExit):
            hb.parse_args(["--conditions", f"native-haiku-low,{ENVELOPE}"])
        with patch.object(sys, "stderr"), self.assertRaises(SystemExit) as refused:
            hb.parse_args(["--conditions", f"native-haiku-low, {ENVELOPE} "])
        self.assertEqual(refused.exception.code, 2, "a padded name is the same condition and is refused the same way")
        args = hb.parse_args(["--conditions", f"native-haiku-low, {ENVELOPE}", "--diagnostic"])
        self.assertTrue(args.diagnostic)
        self.assertFalse(hb.parse_args(["--conditions", "native-haiku-low"]).diagnostic)

    def test_the_manifest_records_the_flag(self):
        args = hb.parse_args(["--conditions", "native-luna-low", "--diagnostic"])
        with patch.object(hb, "git_provenance", return_value={}), \
                patch.object(hb, "native_cli_provenance", return_value={}), \
                patch.object(hb, "benchmark_hashes", return_value={}):
            manifest = hb.build_manifest([], "aether", "0" * 64, args)
        self.assertIs(manifest["diagnostic"], True)
        self.assertIs(manifest["pilot"], False)
        self.assertEqual(manifest["protocol_version"], 2)
        self.assertEqual(report.protocol_version(manifest), 2)
        self.assertEqual(report.protocol_version({"trials": []}), 1, "a manifest from before versions is version 1")

    def test_manifests_of_two_protocol_versions_are_never_merged(self):
        trial = {"condition": "native-haiku-low", "case": "matchbook"}
        earlier = self._saved("earlier.json", trials=[{**trial, "short_id": "a"}])
        later = self._saved("later.json", protocol_version=2, trials=[{**trial, "short_id": "b"}])
        with self.assertRaisesRegex(ValueError, "different protocol versions are never merged"):
            compare.load_rows([earlier, later])
        argv = ["harness_bench_report.py", str(later), str(earlier), "--output-dir", str(self.tmp / "report")]
        with patch.object(sys, "argv", argv), self.assertRaisesRegex(ValueError, "earlier.json: version 1"):
            report.main()
        self.assertFalse((self.tmp / "report").exists(), "a refused mix must not leave a report behind")
        same = [(earlier, {"trials": []}), (later, {"trials": []})]
        self.assertEqual(report.refuse_mixed_protocols(same), 1)
        self.assertEqual(report.refuse_mixed_protocols([(later, {"protocol_version": 2})]), 2)

    def test_the_comparison_refuses_a_diagnostic_manifest(self):
        counted_cells = [{"condition": "native-haiku-low", "case": "matchbook", "short_id": "a"}]
        flagged = self._saved("flagged.json", diagnostic=True, trials=counted_cells)
        with self.assertRaisesRegex(ValueError, "diagnostic manifests locate a cause and are never counted"):
            compare.load_rows([flagged])

    def test_a_diag_trial_is_refused_even_without_the_flag(self):
        unflagged = self._saved("unflagged.json", trials=[
            {"condition": MINIMAL, "case": "matchbook", "short_id": "b"}])
        with self.assertRaisesRegex(ValueError, "never counted"):
            compare.load_rows([unflagged])
        self.assertTrue(report.is_diagnostic_manifest(json.loads(unflagged.read_text())))
        self.assertFalse(report.is_diagnostic_manifest({"trials": [{"condition": "native-haiku-low"}]}))

    def test_the_html_report_refuses_a_diagnostic_manifest_too(self):
        flagged = self._saved("flagged.json", diagnostic=True, trials=[])
        argv = ["harness_bench_report.py", str(flagged), "--output-dir", str(self.tmp / "report")]
        with patch.object(sys, "argv", argv), self.assertRaisesRegex(ValueError, "never counted"):
            report.main()
        self.assertFalse((self.tmp / "report").exists(), "a refused diagnostic must not leave a report behind")


if __name__ == "__main__":
    unittest.main()
