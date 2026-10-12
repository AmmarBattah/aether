"""Deterministic checks for the soak driver's evidence collection."""
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from soak_mock import ModelReceipts, Receipts, run


def event(seq, kind, **fields):
    return {"seq": seq, "kind": {"type": kind, **fields}}



class ModelReceiptTests(unittest.TestCase):
    def test_only_user_input_satisfies_expected_marker_once(self):
        receipts = ModelReceipts()
        receipts.expect("soak-steer-1-2")
        receipts.observe({"messages": [{"role": "assistant", "content": "soak-steer-1-2"}]})
        self.assertEqual(receipts.remaining(), 1)
        request = {"messages": [{"role": "user", "content": [
            {"type": "text", "text": "hello soak-steer-1-2"}]}]}
        receipts.observe(request)
        receipts.observe(request)
        self.assertEqual(receipts.remaining(), 0)
        self.assertEqual(receipts.seen, 1)


class ReceiptTests(unittest.TestCase):
    def test_delivery_and_consumption_across_pages(self):
        receipts = Receipts()
        receipts.pending.update(("message-a", "message-b"))
        receipts.observe([event(1, "inbox.delivered", item={
            "id": "item-a", "kind": {"kind": "message", "message_id": "message-a"}})])
        self.assertEqual(receipts.consumed, 0)
        receipts.observe([event(2, "inbox.consumed", items=["item-a", "untracked"])])
        receipts.observe([event(3, "inbox.consumed", items=["item-a"])])
        self.assertEqual(receipts.pending, {"message-b"})
        self.assertEqual(receipts.consumed, 1)
        self.assertEqual(receipts.cursor, 3)
        self.assertEqual(receipts.items, {})

    def test_tool_error_is_not_success(self):
        receipts = Receipts()
        with self.assertRaisesRegex(RuntimeError, "tool error"):
            receipts.observe([event(1, "tool.succeeded", outcome={"is_error": True})])
        self.assertEqual(receipts.tools, 0)

    def test_runtime_failures_are_visible(self):
        for kind in ("agent.failed", "task.failed", "tool.failed", "tool.denied", "tool.cancelled"):
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                Receipts().observe([event(1, kind)])

    def test_successful_tools_are_counted(self):
        receipts = Receipts()
        receipts.observe([event(1, "tool.succeeded", outcome={"is_error": False})])
        self.assertEqual(receipts.tools, 1)

    def test_missing_binary_retains_failure_report(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            args = SimpleNamespace(binary=str(output / "missing"), seconds=1, agents=1)
            with self.assertRaises(FileNotFoundError):
                run(args, output)
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(report["status"], "failed")
            self.assertIn("missing", report["error"])


if __name__ == "__main__":
    unittest.main()
