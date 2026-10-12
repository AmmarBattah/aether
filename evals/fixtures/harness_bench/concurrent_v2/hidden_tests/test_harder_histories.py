"""Additional adversarial histories for the versioned concurrent case."""
import threading
import unittest
from concurrent.futures import ThreadPoolExecutor

from txstore import Closed, Conflict, Corrupt, Invalid, Store
from txstore.codec import decode, encode
from support import Journal, canonical, frame, put, receipt


class HarderHistoryTests(unittest.TestCase):
    def test_read_only_commit_validates_point_dependency(self):
        store = Store()
        reader = store.begin("reader")
        self.assertIsNone(reader.get("missing"))
        put(store, "writer", "missing", None)
        with self.assertRaises(Conflict):
            reader.commit()
        self.assertEqual(store.revision, 1)

    def test_read_only_commit_consumes_revision_and_is_journaled(self):
        journal = Journal()
        store = Store(journal)
        row = store.begin("empty").commit()
        self.assertEqual(row, receipt("empty", 1))
        self.assertEqual((store.revision, len(journal.calls)), (1, 1))

    def test_blind_write_conflicts_on_tombstone_revision(self):
        store = Store()
        writer = store.begin("writer")
        writer.put("gone", 1)
        put(store, "insert", "gone", 0)
        delete = store.begin("delete")
        delete.delete("gone")
        delete.commit()
        with self.assertRaises(Conflict):
            writer.commit()

    def test_prefix_read_detects_aba_restoration(self):
        store = Store()
        put(store, "initial", "grp/key", 1)
        reader = store.begin("reader")
        self.assertEqual(reader.scan("grp/"), [("grp/key", 1)])
        put(store, "change", "grp/key", 2)
        put(store, "restore", "grp/key", 1)
        with self.assertRaises(Conflict):
            reader.commit()

    def test_prefix_read_detects_insert_then_delete_phantom(self):
        store = Store()
        reader = store.begin("reader")
        self.assertEqual(reader.scan("p/"), [])
        put(store, "insert", "p/new", 1)
        tx = store.begin("remove")
        tx.delete("p/new")
        tx.commit()
        with self.assertRaises(Conflict):
            reader.commit()

    def test_rollback_retains_reads_made_after_savepoint(self):
        store = Store()
        tx = store.begin("reader")
        tx.savepoint("s")
        tx.get("later")
        tx.put("output", 1)
        tx.rollback("s")
        put(store, "writer", "later", 2)
        with self.assertRaises(Conflict):
            tx.commit()

    def test_rollback_restores_only_final_writes_but_retains_point_reads(self):
        store = Store()
        tx = store.begin("reader")
        tx.get("x")
        tx.savepoint("s")
        tx.put("x", 1)
        tx.rollback("s")
        self.assertEqual(tx.commit()["writes"], [])

    def test_invalid_value_is_deeply_validated_before_staging(self):
        store = Store()
        tx = store.begin("t")
        tx.put("a", 1)
        with self.assertRaises(Invalid):
            tx.put("b", {"nested": [float("inf")]})
        self.assertEqual(tx.commit()["writes"], [["put", "a", 1]])

    def test_closed_check_precedes_invalid_arguments(self):
        tx = Store().begin("t")
        tx.abort()
        for operation in (lambda: tx.put("", object()), lambda: tx.get(""),
                          lambda: tx.rollback(""), lambda: tx.commit()):
            with self.subTest(operation=operation):
                with self.assertRaises(Closed):
                    operation()

    def test_commit_receipt_and_replay_values_are_isolated(self):
        journal = Journal()
        store = Store(journal)
        tx = store.begin("t")
        original = {"items": [1]}
        tx.put("a", original)
        original["items"].append(2)
        row = tx.commit()
        row["writes"][0][2]["items"].append(3)
        self.assertEqual(store.snapshot(), {"a": {"items": [1]}})
        replay = Store(journal).begin("t")
        replay.put("a", {"items": [1]})
        self.assertEqual(replay.commit()["writes"][0][2], {"items": [1]})

    def test_duplicate_id_bool_int_mismatch_is_invalid_after_recovery(self):
        journal = Journal()
        store = Store(journal)
        put(store, "same", "k", True)
        restored = Store(journal)
        tx = restored.begin("same")
        tx.put("k", 1)
        with self.assertRaises(Invalid):
            tx.commit()

    def test_duplicate_id_exact_retry_ignores_new_conflict(self):
        journal = Journal()
        store = Store(journal)
        original = store.begin("same")
        original.put("k", 1)
        first = original.commit()
        stale = store.begin("same")
        stale.get("k")
        stale.put("k", 1)
        put(store, "other", "k", 2)
        self.assertEqual(stale.commit(), first)
        self.assertEqual((store.revision, len(journal.calls)), (2, 2))

    def test_all_final_frame_cuts_preserve_prior_commits(self):
        first = receipt("one", 1, [["put", "k", 1]])
        second = receipt("two", 2, [["put", "k", "é"]])
        prefix, final = frame(canonical(first)), frame(canonical(second))
        # Leave at least one checksum byte missing; the frozen oracle accepts
        # a complete digest when only its final LF is absent.
        for cut in range(len(final) - 2):
            with self.subTest(cut=cut):
                self.assertEqual(decode(prefix + final[:cut]), [first])

    def test_complete_second_frame_must_be_canonical_and_checksummed(self):
        row = receipt("one", 1)
        valid = frame(canonical(row))
        replacement = b"1" if valid[-2:-1] != b"1" else b"0"
        damaged = valid[:-2] + replacement + b"\n"
        with self.assertRaises(Corrupt):
            decode(damaged)
        self.assertEqual(decode(valid), [row])

    def test_recovery_restores_tombstone_conflict_history(self):
        journal = Journal()
        store = Store(journal)
        put(store, "old", "x", 1)
        tx = store.begin("delete")
        tx.delete("x")
        tx.commit()
        restored = Store(journal)
        blind = restored.begin("blind")
        blind.put("x", 3)
        put(restored, "mutation", "x", 4)
        with self.assertRaises(Conflict):
            blind.commit()

    def test_concurrent_disjoint_commits_all_publish_once(self):
        store = Store()
        barrier = threading.Barrier(9)

        def worker(index):
            tx = store.begin(f"t{index}")
            tx.put(f"k{index}", index)
            barrier.wait(timeout=10)
            return tx.commit()

        with ThreadPoolExecutor(max_workers=8) as pool:
            futures = [pool.submit(worker, index) for index in range(8)]
            barrier.wait(timeout=10)
            rows = [future.result(timeout=10) for future in futures]
        self.assertEqual(sorted(row["revision"] for row in rows), list(range(1, 9)))
        self.assertEqual(len(store.snapshot()), 8)

    def test_concurrent_same_id_identical_writes_share_receipt(self):
        store = Store()
        barrier = threading.Barrier(3)

        def worker():
            tx = store.begin("same")
            tx.put("k", [1])
            barrier.wait(timeout=10)
            return tx.commit()

        with ThreadPoolExecutor(max_workers=2) as pool:
            left, right = pool.submit(worker), pool.submit(worker)
            barrier.wait(timeout=10)
            a, b = left.result(timeout=10), right.result(timeout=10)
        self.assertEqual(a, b)
        self.assertEqual(store.revision, 1)

    def test_encode_rejects_bool_revision_and_noncanonical_order(self):
        for row in (receipt(revision=True), receipt(writes=[["put", "z", 1], ["delete", "a"]])):
            with self.subTest(row=row):
                with self.assertRaises(Invalid):
                    encode(row)


if __name__ == "__main__":
    unittest.main()
