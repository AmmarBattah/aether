import unittest

from txstore import Closed, Conflict, Invalid, Store
from support import Journal, delete, put


class TransactionTests(unittest.TestCase):
    def test_repeatable_snapshot_and_own_overlay(self):
        store = Store()
        put(store, "seed", "a", 1)
        tx = store.begin("old")
        put(store, "new", "a", 2)
        self.assertEqual(tx.get("a"), 1)
        tx.put("b", None)
        tx.delete("a")
        self.assertEqual(tx.scan(), [("b", None)])
        self.assertIsNone(tx.get("a"))
        with self.assertRaises(Conflict):
            tx.commit()

    def test_disjoint_blind_writes_can_commit(self):
        store = Store()
        left, right = store.begin("left"), store.begin("right")
        left.put("a", 1)
        right.put("b", 2)
        left.commit()
        self.assertEqual(right.commit()["revision"], 2)
        self.assertEqual(store.snapshot(), {"a": 1, "b": 2})

    def test_disjoint_reads_do_not_conflict(self):
        store = Store()
        tx = store.begin("reader")
        self.assertIsNone(tx.get("a"))
        put(store, "other", "b", 1)
        self.assertEqual(tx.commit()["revision"], 2)

    def test_blind_write_write_conflicts(self):
        store = Store()
        tx = store.begin("old")
        tx.put("a", 1)
        put(store, "new", "a", 2)
        with self.assertRaises(Conflict):
            tx.commit()
        self.assertEqual(store.snapshot(), {"a": 2})

    def test_missing_point_read_detects_insert_delete(self):
        store = Store()
        tx = store.begin("reader")
        self.assertIsNone(tx.get("a"))
        put(store, "add", "a", 1)
        delete(store, "remove", "a")
        with self.assertRaises(Conflict):
            tx.commit()

    def test_same_value_put_is_still_conflicting(self):
        store = Store()
        put(store, "seed", "a", 1)
        tx = store.begin("reader")
        tx.get("a")
        put(store, "noop", "a", 1)
        with self.assertRaises(Conflict):
            tx.commit()

    def test_range_phantom_insertion(self):
        store = Store()
        tx = store.begin("reader")
        self.assertEqual(tx.scan("group/"), [])
        put(store, "new", "group/x", 1)
        with self.assertRaises(Conflict):
            tx.commit()

    def test_range_phantom_insert_delete_and_absent_delete(self):
        for transient in (True, False):
            store = Store()
            tx = store.begin("reader")
            tx.scan("group/")
            if transient:
                put(store, "add", "group/x", 1)
            delete(store, "remove", "group/x")
            with self.assertRaises(Conflict):
                tx.commit()

    def test_range_ignores_nonmatching_keys(self):
        store = Store()
        tx = store.begin("reader")
        tx.scan("group/")
        put(store, "add", "groups/x", 1)
        tx.commit()

    def test_empty_prefix_observes_all_mutations(self):
        store = Store()
        tx = store.begin("reader")
        tx.scan()
        delete(store, "absent", "anything")
        with self.assertRaises(Conflict):
            tx.commit()

    def test_rollback_discards_writes_not_read_dependencies(self):
        for use_scan in (True, False):
            store = Store()
            tx = store.begin("reader")
            tx.savepoint("s")
            tx.put("a", 10)
            if use_scan:
                tx.scan("b")
            else:
                tx.get("b")
            tx.rollback("s")
            put(store, "new", "b", 2)
            with self.assertRaises(Conflict):
                tx.commit()

    def test_rollback_removes_blind_write_conflict(self):
        store = Store()
        tx = store.begin("old")
        tx.savepoint("s")
        tx.put("a", 1)
        tx.rollback("s")
        put(store, "other", "a", 2)
        self.assertEqual(tx.commit()["writes"], [])

    def test_savepoint_remains_reusable_later_names_disappear(self):
        tx = Store().begin("t")
        tx.put("a", {"list": [1]})
        tx.savepoint("first")
        tx.put("a", 2)
        tx.savepoint("second")
        tx.rollback("first")
        tx.put("a", 3)
        tx.rollback("first")
        self.assertEqual(tx.get("a"), {"list": [1]})
        with self.assertRaises(Invalid):
            tx.rollback("second")
        with self.assertRaises(Invalid):
            tx.savepoint("first")

    def test_defensive_copies_at_all_boundaries(self):
        store = Store()
        value = {"nested": [1]}
        tx = store.begin("t")
        tx.put("a", value)
        value["nested"].append(2)
        tx.get("a")["nested"].append(3)
        tx.scan()[0][1]["nested"].append(4)
        result = tx.commit()
        result["writes"][0][2]["nested"].append(5)
        store.snapshot()["a"]["nested"].append(6)
        self.assertEqual(store.snapshot(), {"a": {"nested": [1]}})
        retry = store.begin("t")
        retry.put("a", {"nested": [1]})
        self.assertEqual(retry.commit()["writes"], [["put", "a", {"nested": [1]}]])

    def test_invalid_values_do_not_overwrite_pending_write(self):
        for value in (float("nan"), {"x": float("inf")}, {1: "x"}, (1, 2), {1, 2}):
            tx = Store().begin("t")
            tx.put("a", 1)
            with self.assertRaises(Invalid):
                tx.put("a", value)
            self.assertEqual(tx.get("a"), 1)

    def test_invalid_names_and_prefixes_leave_transaction_open(self):
        store = Store()
        for name in (None, "", 1, []):
            with self.assertRaises(Invalid):
                store.begin(name)
        tx = store.begin("t")
        for action in (lambda: tx.get(""), lambda: tx.put("", 1), lambda: tx.delete(""),
                       lambda: tx.scan(None), lambda: tx.savepoint(""), lambda: tx.rollback("missing")):
            with self.assertRaises(Invalid):
                action()
        self.assertEqual(tx.commit()["writes"], [])

    def test_closed_precedes_argument_validation_everywhere(self):
        for abort in (True, False):
            tx = Store().begin("t")
            tx.abort() if abort else tx.commit()
            for action in (lambda: tx.get(None), lambda: tx.put(None, object()), lambda: tx.delete(None),
                           lambda: tx.scan(None), lambda: tx.savepoint(None), lambda: tx.rollback(None),
                           tx.commit, tx.abort):
                with self.assertRaises(Closed):
                    action()

    def test_append_failure_does_not_publish_and_can_retry(self):
        journal = Journal()
        store = Store(journal)
        tx = store.begin("t")
        tx.put("a", 1)
        failure = OSError("disk full")
        journal.failure = failure
        with self.assertRaises(OSError) as caught:
            tx.commit()
        self.assertIs(caught.exception, failure)
        self.assertEqual((store.revision, store.snapshot(), journal.data), (0, {}, b""))
        journal.failure = None
        self.assertEqual(tx.commit()["revision"], 1)
        self.assertEqual(len(journal.calls), 1)

    def test_idempotency_precedes_conflicts_and_does_not_reapply(self):
        journal = Journal()
        store = Store(journal)
        old = store.begin("same")
        old.get("a")
        old.put("a", 1)
        original = put(store, "same", "a", 1)
        put(store, "later", "a", 2)
        self.assertEqual(old.commit(), original)
        self.assertEqual((store.revision, store.snapshot(), len(journal.calls)), (2, {"a": 2}, 2))
        with self.assertRaises(Closed):
            old.commit()

    def test_idempotency_mismatch_is_open_and_type_sensitive(self):
        for initial, changed in ((True, 1), (1, 1.0), (1.0, True)):
            store = Store()
            original = put(store, "same", "a", initial)
            tx = store.begin("same")
            tx.put("a", changed)
            with self.assertRaises(Invalid):
                tx.commit()
            tx.put("a", initial)
            self.assertEqual(tx.commit(), original)

    def test_receipt_keeps_noops_and_sorts_final_operations(self):
        store = Store()
        tx = store.begin("t")
        tx.put("z", 1)
        tx.delete("missing")
        tx.put("a", False)
        tx.put("z", 2)
        self.assertEqual(tx.commit()["writes"], [["put", "a", False], ["delete", "missing"], ["put", "z", 2]])

    def test_readonly_commits_persist_and_increment_revision(self):
        journal = Journal()
        store = Store(journal)
        first = store.begin("one").commit()
        second = store.begin("two").commit()
        self.assertEqual((first["revision"], second["revision"], len(journal.calls)), (1, 2, 2))
        recovered = Store(journal)
        self.assertEqual(recovered.begin("one").commit(), first)
        self.assertEqual(recovered.revision, 2)
