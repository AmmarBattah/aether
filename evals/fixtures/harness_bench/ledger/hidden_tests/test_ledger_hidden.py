"""Held-out oracle for the ledger case. Not copied into solver workspaces;
grade() imports the solver's `ledger` package from their workspace and runs
these tests against it. Independent of whatever tests the solver writes.

tearDown does not call close() itself (some tests close explicitly, e.g. to
force a flush before reading the journal directly) so double-close never
happens by accident here; test_engine_close_is_idempotent below is the
dedicated, deliberate check for that contract requirement.
"""
import heapq
import random
import tempfile
import unittest
from pathlib import Path

from ledger import LedgerEngine, LedgerError
from ledger.journal import Journal


class _FailingJournal:
    """Wraps a real Journal; append() raises exactly once, on demand, to
    exercise the append-failure rollback contract without touching disk."""

    def __init__(self, real):
        self._real = real
        self.fail_next = False

    def append(self, entry):
        if self.fail_next:
            self.fail_next = False
            raise OSError("simulated disk failure")
        self._real.append(entry)

    def read_all(self):
        return self._real.read_all()

    def close(self):
        self._real.close()


class LedgerHiddenTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.path = Path(self._tmp.name) / "journal.jsonl"
        self.engine = LedgerEngine(self.path)

    def _open(self, request_id, account, opening_balance=0):
        return self.engine.apply(request_id, "open_account", account=account,
                                  opening_balance=opening_balance)

    # -- basic happy paths also covered lightly by the visible tests --

    def test_deposit_and_withdraw_within_balance_succeed(self):
        self._open("r0", "a", 10)
        self.engine.apply("r1", "deposit", account="a", amount=5)
        result = self.engine.apply("r2", "withdraw", account="a", amount=3)
        self.assertEqual(result["status"], "applied")
        self.assertEqual(self.engine.balance("a"), 12)

    def test_withdraw_more_than_balance_is_rejected_without_mutating_balance(self):
        self._open("r0", "a", 10)
        result = self.engine.apply("r1", "withdraw", account="a", amount=1000)
        self.assertEqual(result["status"], "rejected")
        self.assertEqual(self.engine.balance("a"), 10)

    def test_transfer_to_unknown_account_does_not_leak_from_source(self):
        self._open("r0", "a", 10)
        result = self.engine.apply("r1", "transfer", src="a", dst="ghost", amount=4)
        self.assertEqual(result["status"], "rejected")
        self.assertEqual(self.engine.balance("a"), 10)

    # -- idempotency --

    def test_duplicate_request_id_deposit_is_idempotent(self):
        self._open("r0", "a", 0)
        first = self.engine.apply("dup", "deposit", account="a", amount=10)
        second = self.engine.apply("dup", "deposit", account="a", amount=10)
        self.assertEqual(first, second)
        self.assertEqual(self.engine.balance("a"), 10)
        self.assertEqual(len(self.engine.history("a")), 2)  # open + one deposit

    def test_duplicate_request_id_open_account_is_idempotent(self):
        first = self._open("dup", "a", 5)
        second = self._open("dup", "a", 999)
        self.assertEqual(first, second)
        self.assertEqual(self.engine.balance("a"), 5)

    # -- durability / replay --

    def test_journal_write_is_visible_to_a_separate_reader_before_close(self):
        self._open("r0", "a", 3)
        self.engine.apply("r1", "deposit", account="a", amount=2)
        reader = Journal(self.path)
        entries = reader.read_all()
        reader.close()
        self.assertEqual(len(entries), 2)

    def test_replay_reconstructs_state_without_growing_journal(self):
        self._open("r0", "a", 10)
        self._open("r1", "b", 0)
        self.engine.apply("r2", "transfer", src="a", dst="b", amount=4)
        self.engine.close()
        lines_before = self.path.read_text().count("\n")

        replayed = LedgerEngine.replay(self.path)
        self.assertEqual(replayed.balance("a"), 6)
        self.assertEqual(replayed.balance("b"), 4)
        lines_after = self.path.read_text().count("\n")
        self.assertEqual(lines_before, lines_after)
        replayed.close()

    def test_replay_preserves_rejected_entries_in_history(self):
        self._open("r0", "a", 0)
        self.engine.apply("r1", "deposit", account="ghost", amount=5)  # rejected
        self.engine.close()
        replayed = LedgerEngine.replay(self.path)
        statuses = [h["status"] for h in replayed.history("a")]
        self.assertIn("applied", statuses)
        replayed.close()

    def test_replay_tolerates_a_truncated_final_journal_line(self):
        self._open("r0", "a", 10)
        self.engine.apply("r1", "deposit", account="a", amount=5)
        self.engine.close()
        with open(self.path, "a", encoding="utf-8") as handle:
            handle.write('{"request_id": "r2", "status": "applie')  # no newline: truncated

        replayed = LedgerEngine.replay(self.path)
        self.assertEqual(replayed.balance("a"), 15)
        replayed.close()

    def test_replay_raises_ledger_error_on_a_corrupt_non_trailing_journal_line(self):
        self._open("r0", "a", 10)
        self.engine.close()
        with open(self.path, "a", encoding="utf-8") as handle:
            handle.write("not json at all\n")
            handle.write('{"request_id": "r1", "status": "applied", "reason": null, '
                          '"op": "deposit", "account": "a", "amount": 5}\n')
        with self.assertRaises(LedgerError):
            LedgerEngine.replay(self.path)

    def test_engine_close_is_idempotent(self):
        self._open("r0", "a", 1)
        self.engine.close()
        self.engine.close()  # must not raise

    # -- history ordering and audit trail --

    def test_history_orders_entries_and_includes_rejections(self):
        self._open("r0", "a", 0)
        self.engine.apply("r1", "deposit", account="a", amount=1)
        self.engine.apply("r2", "deposit", account="ghost", amount=1)  # rejected
        self.engine.apply("r3", "deposit", account="a", amount=2)
        history = self.engine.history("a")
        self.assertEqual([h["request_id"] for h in history], ["r0", "r1", "r3"])

    # -- defensive copies: callers must not be able to corrupt engine state --

    def test_apply_result_is_a_defensive_copy_not_a_live_reference(self):
        result = self.engine.apply("r1", "open_account", account="a", opening_balance=5)
        result["status"] = "tampered"
        result["opening_balance"] = -1
        again = self.engine.apply("r1", "open_account", account="a", opening_balance=999)
        self.assertEqual(again["status"], "applied")
        self.assertEqual(self.engine.balance("a"), 5)

    def test_history_entries_are_defensive_copies(self):
        self._open("r0", "a", 3)
        entries = self.engine.history("a")
        entries[0]["opening_balance"] = -999
        entries_again = self.engine.history("a")
        self.assertEqual(entries_again[0]["opening_balance"], 3)

    # -- same-account transfer --

    def test_transfer_to_the_same_account_is_a_net_no_op_when_funds_suffice(self):
        self._open("r0", "a", 20)
        result = self.engine.apply("r1", "transfer", src="a", dst="a", amount=7)
        self.assertEqual(result["status"], "applied")
        self.assertEqual(self.engine.balance("a"), 20)

    def test_transfer_to_the_same_account_is_rejected_without_mutation_when_insufficient(self):
        self._open("r0", "a", 3)
        result = self.engine.apply("r1", "transfer", src="a", dst="a", amount=7)
        self.assertEqual(result["status"], "rejected")
        self.assertEqual(self.engine.balance("a"), 3)

    # -- input validation --

    def test_unknown_op_name_raises_and_is_not_journaled(self):
        self._open("r0", "a", 5)
        with self.assertRaises(LedgerError):
            self.engine.apply("r1", "not_a_real_op", account="a")
        self.assertEqual(len(self.engine.history("a")), 1)  # just the open

    def test_apply_with_negative_opening_balance_raises_and_is_not_journaled(self):
        with self.assertRaises(LedgerError):
            self.engine.apply("r0", "open_account", account="a", opening_balance=-5)
        self.assertFalse(self.engine.history("a"))

    def test_deposit_with_non_positive_amount_raises_and_is_not_journaled(self):
        self._open("r0", "a", 5)
        with self.assertRaises(LedgerError):
            self.engine.apply("r1", "deposit", account="a", amount=0)
        self.assertEqual(len(self.engine.history("a")), 1)

    # -- append-failure rollback --

    def test_journal_append_failure_rolls_back_balance_and_history(self):
        real_journal = Journal(self.path)
        failing = _FailingJournal(real_journal)
        engine = LedgerEngine(self.path, journal=failing)
        engine.apply("r1", "open_account", account="a", opening_balance=10)

        failing.fail_next = True
        with self.assertRaises(OSError):
            engine.apply("r2", "withdraw", account="a", amount=4)

        self.assertEqual(engine.balance("a"), 10)
        self.assertEqual(len(engine.history("a")), 1)  # only r1 recorded
        failing.close()

    def test_request_id_from_a_failed_append_can_be_retried_successfully(self):
        real_journal = Journal(self.path)
        failing = _FailingJournal(real_journal)
        engine = LedgerEngine(self.path, journal=failing)
        engine.apply("r1", "open_account", account="a", opening_balance=10)

        failing.fail_next = True
        with self.assertRaises(OSError):
            engine.apply("r2", "withdraw", account="a", amount=4)

        result = engine.apply("r2", "withdraw", account="a", amount=4)
        self.assertEqual(result["status"], "applied")
        self.assertEqual(engine.balance("a"), 6)
        failing.close()

    # -- seeded/property-style deterministic sequence, independent oracle --

    def test_generated_operation_sequences_match_an_independent_balance_oracle(self):
        for seed in range(10):
            ops = _random_ledger_ops(seed)
            path = Path(self._tmp.name) / f"seeded-{seed}.jsonl"
            engine = LedgerEngine(path)
            for request_id, op, kwargs in ops:
                engine.apply(request_id, op, **kwargs)

            expected = _independent_balance_oracle(ops)
            for account, expected_balance in expected.items():
                self.assertEqual(engine.balance(account), expected_balance,
                                  f"seed={seed} account={account}")
            engine.close()

            replayed = LedgerEngine.replay(path)
            for account, expected_balance in expected.items():
                self.assertEqual(replayed.balance(account), expected_balance,
                                  f"seed={seed} account={account} (replay)")
            replayed.close()


def _independent_balance_oracle(ops):
    """Recomputes expected final balances directly from the contract's rules
    (idempotent by request_id, atomic transfer, no negative balances) using
    plain dicts, independent of whatever `ledger` package is under test."""
    balances = {}
    seen = set()
    for request_id, op, kwargs in ops:
        if request_id in seen:
            continue
        seen.add(request_id)
        if op == "open_account":
            account = kwargs["account"]
            if account not in balances:
                balances[account] = kwargs.get("opening_balance", 0)
        elif op == "deposit":
            account = kwargs["account"]
            if account in balances:
                balances[account] += kwargs["amount"]
        elif op == "withdraw":
            account = kwargs["account"]
            if account in balances and balances[account] >= kwargs["amount"]:
                balances[account] -= kwargs["amount"]
        elif op == "transfer":
            src, dst, amount = kwargs["src"], kwargs["dst"], kwargs["amount"]
            if src in balances and dst in balances and balances[src] >= amount:
                balances[src] -= amount
                balances[dst] += amount
    return balances


def _random_ledger_ops(seed, count=40, num_accounts=4):
    """A deterministic (seeded) sequence of ledger ops, with occasional
    request_id repeats to exercise idempotency alongside normal traffic."""
    rng = random.Random(seed)
    accounts = [f"acct{i}" for i in range(num_accounts)]
    opened = set()
    ops = []
    for i in range(count):
        request_id = f"r{i}"
        if ops and rng.random() < 0.15:
            request_id = rng.choice(ops)[0]  # replay an earlier request_id
        account = rng.choice(accounts)
        choice = rng.random()
        if account not in opened or choice < 0.15:
            kwargs = {"account": account, "opening_balance": rng.randint(0, 50)}
            ops.append((request_id, "open_account", kwargs))
            opened.add(account)
        elif choice < 0.45:
            ops.append((request_id, "deposit",
                        {"account": account, "amount": rng.randint(1, 30)}))
        elif choice < 0.7:
            ops.append((request_id, "withdraw",
                        {"account": account, "amount": rng.randint(1, 30)}))
        else:
            dst = rng.choice(accounts)
            ops.append((request_id, "transfer",
                        {"src": account, "dst": dst, "amount": rng.randint(1, 30)}))
    return ops


if __name__ == "__main__":
    unittest.main()
