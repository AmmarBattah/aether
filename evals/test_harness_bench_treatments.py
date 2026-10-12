"""Explicit model and effort must survive both launch paths identically."""
import json
import unittest
from pathlib import Path
from unittest.mock import Mock

import harness_bench as hb
from aether_client import AetherClient
from harness_bench_pricing import resolve_price, validate_price_table

# `list_models` rows for the two benchmark models, saved from the pinned binary's isolated daemon.
PINNED_INVENTORY = Path(__file__).parent / "fixtures" / "harness_bench_compare" / "pinned-inventory-slice.json"


class LowerTierTreatmentTests(unittest.TestCase):
    def test_matched_launches(self):
        for label, model, builder, provider in (
            ("sonnet", "sonnet", hb.native_claude_command, "claude-code"),
            ("luna", "gpt-6-luna", hb.native_codex_command, "codex"),
            ("haiku", "claude-haiku-5-5", hb.native_claude_command, "claude-code"),
        ):
            for effort in ("low", "medium"):
                with self.subTest(model=model, effort=effort):
                    native = hb.CONDITIONS[f"native-{label}-{effort}"]
                    aether = hb.CONDITIONS[f"aether-single-{label}-{effort}"]
                    self.assertEqual(native["model"], model)
                    self.assertEqual(native["effort"], effort)
                    command = builder("task", native["model"], native["effort"])
                    self.assertEqual(command[command.index("--model") + 1], model)
                    if provider == "claude-code":
                        self.assertEqual(command[command.index("--effort") + 1], effort)
                    else:
                        self.assertIn(f'model_reasoning_effort="{effort}"', command)
                    client = Mock()
                    client.create_agent.return_value = {"id": "root"}
                    self.assertEqual(hb._spawn_root(client, aether, "task"), ("root", effort))
                    client.create_agent.assert_called_once_with(
                        role="assistant", objective="task", model=f"{provider}/{model}",
                        effort=effort, long_lived=False)
                    self.assertEqual(native["suffix"], aether["suffix"])

    def test_haiku_cells_leave_the_existing_cells_unchanged(self):
        from harness_bench_config import SOLO_SUFFIX
        self.assertEqual(hb.CONDITIONS["native-sonnet-low"], {
            "kind": "native-claude", "model": "sonnet", "effort": "low", "suffix": SOLO_SUFFIX})
        self.assertEqual(hb.CONDITIONS["aether-single-luna-medium"], {
            "kind": "aether-single", "model": "codex/gpt-6-luna", "effort": "medium", "suffix": SOLO_SUFFIX})
        self.assertEqual(hb.CONDITIONS["native-haiku-low"], {
            "kind": "native-claude", "model": "claude-haiku-5-5", "effort": "low", "suffix": SOLO_SUFFIX})
        self.assertEqual(hb.CONDITIONS["aether-single-haiku-low"], {
            "kind": "aether-single", "model": "claude-code/claude-haiku-5-5", "effort": "low",
            "suffix": SOLO_SUFFIX})
        self.assertNotIn("native-haiku-low", hb.DEFAULT_CONDITIONS)

    def test_lower_tiers_are_opt_in(self):
        self.assertEqual(len(hb.DEFAULT_CONDITIONS), 4)
        self.assertNotIn("--model", hb.native_codex_command("task"))
        self.assertNotIn("--effort", hb.native_claude_command("task"))

    def test_minimal_luna_tool_filter_is_opt_in_and_recorded(self):
        key = "aether-solo-minimal-luna-low"
        baseline = hb.CONDITIONS["aether-single-luna-low"]
        treatment = hb.CONDITIONS[key]
        self.assertNotIn(key, hb.DEFAULT_CONDITIONS)
        self.assertNotIn("tools", baseline)
        self.assertEqual(treatment["model"], baseline["model"])
        self.assertEqual(treatment["effort"], baseline["effort"])
        self.assertEqual(treatment["suffix"], baseline["suffix"])
        self.assertEqual(treatment["tools"], {
            "mode": "only", "names": ["complete_task", "fail_task"],
        })

        client = Mock()
        client.create_agent.return_value = {"id": "root"}
        self.assertEqual(hb._spawn_root(client, treatment, "task"), ("root", "low"))
        client.create_agent.assert_called_once_with(
            role="assistant", objective="task", model="codex/gpt-6-luna",
            effort="low", long_lived=False, tools=treatment["tools"],
        )

        protocol_client = object.__new__(AetherClient)
        protocol_client.request = Mock(return_value={"data": {"id": "root"}})
        protocol_client.create_agent(
            role="assistant", objective="task", model="codex/gpt-6-luna",
            effort="low", long_lived=False, tools=treatment["tools"],
        )
        request = protocol_client.request.call_args.args[0]
        self.assertEqual(request["config"]["tools"], treatment["tools"])

    def test_haiku_and_luna_cells_are_priceable_from_the_pinned_inventory(self):
        table = json.loads(PINNED_INVENTORY.read_text())
        validate_price_table(table)
        for label, native_provider in (("haiku", "claude-code"), ("luna", "codex")):
            native = hb.CONDITIONS[f"native-{label}-low"]
            aether = hb.CONDITIONS[f"aether-single-{label}-low"]
            with self.subTest(label=label):
                native_price = resolve_price(table, native_provider, native["model"])
                aether_price = resolve_price(table, None, aether["model"])
                self.assertIsNotNone(native_price, "the native cell must be priceable")
                self.assertEqual(native_price["model"], aether_price["model"], "both sides must use one price row")
                self.assertGreater(native_price["capabilities"]["cost"]["output_per_mtok"], 0)

    def test_a_model_missing_from_the_inventory_stays_unpriced(self):
        table = json.loads(PINNED_INVENTORY.read_text())
        self.assertIsNone(resolve_price(table, "claude-code", "claude-haiku-9-9"))
        self.assertIsNone(resolve_price(table, None, "codex/gpt-6-nova"))


if __name__ == "__main__":
    unittest.main()
