"""Visible smoke tests. These only cover the easy happy path; the graded
oracle (outside this workspace) checks the harder contract requirements."""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ledger import LedgerEngine


class LedgerBasicTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.path = Path(self._tmp.name) / "journal.jsonl"
        self.engine = LedgerEngine(self.path)

    def tearDown(self):
        self.engine.close()
        self._tmp.cleanup()

    def test_open_and_deposit(self):
        self.engine.apply("r1", "open_account", account="alice", opening_balance=10)
        self.engine.apply("r2", "deposit", account="alice", amount=5)
        self.assertEqual(self.engine.balance("alice"), 15)

    def test_withdraw_within_balance(self):
        self.engine.apply("r1", "open_account", account="alice", opening_balance=10)
        result = self.engine.apply("r2", "withdraw", account="alice", amount=4)
        self.assertEqual(result["status"], "applied")
        self.assertEqual(self.engine.balance("alice"), 6)

    def test_transfer_between_existing_accounts(self):
        self.engine.apply("r1", "open_account", account="alice", opening_balance=10)
        self.engine.apply("r2", "open_account", account="bob", opening_balance=0)
        result = self.engine.apply("r3", "transfer", src="alice", dst="bob", amount=4)
        self.assertEqual(result["status"], "applied")
        self.assertEqual(self.engine.balance("alice"), 6)
        self.assertEqual(self.engine.balance("bob"), 4)

    def test_deposit_to_unknown_account_is_rejected(self):
        result = self.engine.apply("r1", "deposit", account="ghost", amount=5)
        self.assertEqual(result["status"], "rejected")


if __name__ == "__main__":
    unittest.main()
