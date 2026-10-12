import hashlib
import json
import random
import unittest

from txstore import Corrupt, Invalid, Store
from txstore.codec import decode, encode
from support import Journal, canonical, delete, frame, put, receipt


class JournalTests(unittest.TestCase):
    def test_unicode_framing_uses_bytes(self):
        row = receipt("é", writes=[["put", "日本", "🙂\n:"]])
        expected = frame(canonical(row))
        self.assertEqual(encode(row), expected)
        self.assertEqual(decode(expected), [row])

    def test_all_valid_truncations_of_final_frame(self):
        first = receipt("first")
        last = receipt("last", 2, [["put", "é", [True, None, 1.5]]])
        prefix, suffix = frame(canonical(first)), frame(canonical(last))
        for cut in range(len(suffix)):
            self.assertEqual(decode(prefix + suffix[:cut]), [first], cut)
        self.assertEqual(decode(prefix + suffix), [first, last])

    def test_corrupt_header_bytes_even_when_short(self):
        for raw in (b"x", b"000Z", b"0000000A", b"00000000;", b"-0000001:"):
            with self.assertRaises(Corrupt):
                decode(raw)

    def test_corrupt_available_trailer_bytes_even_when_short(self):
        payload = canonical(receipt())
        prefix = f"{len(payload):08x}:".encode() + payload
        for tail in (b";", b":x", b":AAA", b":" + b"0" * 64, b":" + hashlib.sha256(payload).hexdigest().encode() + b"!"):
            with self.assertRaises(Corrupt):
                decode(prefix + tail)

    def test_checksum_mismatch_is_not_truncated_tail(self):
        row = frame(canonical(receipt(writes=[["put", "a", 1]])))
        changed = row.replace(b'"a"', b'"b"')
        with self.assertRaises(Corrupt):
            decode(changed)

    def test_noncanonical_json_and_duplicate_fields_rejected(self):
        for payload in (json.dumps(receipt()).encode(),
                        b'{"revision":1,"txid":"r","txid":"r","writes":[]}',
                        b'{"revision":1,"txid":"r","writes":[["put","a",NaN]]}'):
            with self.assertRaises(Corrupt):
                decode(frame(payload))

    def test_invalid_json_and_utf8_rejected(self):
        for payload in (b"\xff", b"{", b"null", b"[]"):
            with self.assertRaises(Corrupt):
                decode(frame(payload))

    def test_invalid_receipt_shapes_rejected_by_encoder_and_decoder(self):
        invalid = [receipt(revision=True), receipt(revision=0), receipt(txid=""),
                   {**receipt(), "extra": 1}, {"txid": "r", "revision": 1},
                   receipt(writes={}), receipt(writes=[["put", "a"]]),
                   receipt(writes=[["delete", "a", None]]),
                   receipt(writes=[["other", "a"]]), receipt(writes=[["delete", ""]]),
                   receipt(writes=[["put", "b", 1], ["put", "a", 2]]),
                   receipt(writes=[["delete", "a"], ["put", "a", 1]])]
        for row in invalid:
            with self.assertRaises(Invalid):
                encode(row)
            with self.assertRaises(Corrupt):
                decode(frame(canonical(row)))

    def test_recovery_duplicate_receipt_after_later_commit_is_ignored(self):
        first = receipt("a", 1, [["put", "key", 1]])
        second = receipt("b", 2, [["put", "key", 2]])
        journal = Journal(b"".join(frame(canonical(r)) for r in (first, second, first)))
        store = Store(journal)
        self.assertEqual((store.revision, store.snapshot()), (2, {"key": 2}))
        self.assertEqual((journal.reads, journal.calls), (1, []))

    def test_recovery_rejects_revision_gaps_and_reuse(self):
        for rows in ([receipt("a", 2)], [receipt("a"), receipt("b", 3)],
                     [receipt("a"), receipt("b", 1)]):
            with self.assertRaises(Corrupt):
                Store(Journal(b"".join(frame(canonical(r)) for r in rows)))

    def test_recovery_rejects_changed_duplicate_id_or_revision(self):
        original = receipt("a", 1, [["put", "k", True]])
        for changed in (receipt("a", 2, original["writes"]), receipt("a", 1, [["put", "k", 1]])):
            with self.assertRaises(Corrupt):
                Store(Journal(frame(canonical(original)) + frame(canonical(changed))))

    def test_recovery_restores_idempotency_without_reapplying(self):
        journal = Journal()
        store = Store(journal)
        first = put(store, "first", "a", 1)
        delete(store, "remove", "a")
        restored = Store(journal)
        self.assertEqual(put(restored, "first", "a", 1), first)
        self.assertEqual((restored.snapshot(), restored.revision, len(journal.calls)), ({}, 2, 2))

    def test_recovery_with_partial_tail_exposes_only_committed_prefix(self):
        first = receipt("a", 1, [["put", "k", 1]])
        second = receipt("b", 2, [["put", "k", 2]])
        journal = Journal(frame(canonical(first)) + frame(canonical(second))[:-1])
        restored = Store(journal)
        self.assertEqual((restored.revision, restored.snapshot()), (1, {"k": 1}))
        self.assertEqual(journal.calls, [])

    def test_deterministic_histories_match_external_state_model(self):
        rng = random.Random(7319)
        journal = Journal()
        store = Store(journal)
        expected = {}
        for index in range(50):
            tx = store.begin(f"t{index}")
            operations = {}
            for _ in range(4):
                key = rng.choice(["a", "b", "é", "group/x", "group/y"])
                if rng.randrange(3):
                    value = rng.choice([None, True, 1, 1.5, ["é", {"n": index}]])
                    tx.put(key, value)
                    operations[key] = ["put", key, value]
                else:
                    tx.delete(key)
                    operations[key] = ["delete", key]
            row = tx.commit()
            self.assertEqual(row, receipt(f"t{index}", index + 1, [operations[k] for k in sorted(operations)]))
            for operation in operations.values():
                if operation[0] == "put":
                    expected[operation[1]] = operation[2]
                else:
                    expected.pop(operation[1], None)
            self.assertEqual(store.snapshot(), expected)
            self.assertEqual(Store(journal).snapshot(), expected)
        self.assertEqual(journal.data, b"".join(frame(canonical(r)) for r in decode(journal.data)))

    def test_interleaved_prefix_conflicts_match_mutation_oracle(self):
        from txstore import Conflict
        for prefix in ("", "a", "group/", "z"):
            for key in ("a", "b", "group/x", "grouped", "é"):
                store = Store()
                tx = store.begin("reader")
                tx.scan(prefix)
                delete(store, "mutation", key)
                if key.startswith(prefix):
                    with self.assertRaises(Conflict):
                        tx.commit()
                else:
                    tx.commit()

    def test_journal_observes_old_state_until_append_returns(self):
        journal = Journal()
        store = Store(journal)
        observed = []
        original_append = journal.append

        def inspect(record):
            observed.append((store.revision, store.snapshot()))
            original_append(record)

        journal.append = inspect
        put(store, "a", "key", 1)
        put(store, "b", "key", 2)
        self.assertEqual(observed, [(0, {}), (1, {"key": 1})])

    def test_replay_read_error_propagates_unchanged(self):
        failure = OSError("unavailable")
        journal = Journal()

        def read():
            raise failure

        journal.read = read
        with self.assertRaises(OSError) as caught:
            Store(journal)
        self.assertIs(caught.exception, failure)

    def test_failed_commit_can_acquire_conflict_before_retry(self):
        from txstore import Conflict
        journal = Journal()
        store = Store(journal)
        tx = store.begin("old")
        tx.put("a", 1)
        journal.failure = OSError("full")
        with self.assertRaises(OSError):
            tx.commit()
        journal.failure = None
        put(store, "new", "a", 2)
        with self.assertRaises(Conflict):
            tx.commit()
        self.assertEqual((store.snapshot(), store.revision, len(journal.calls)), ({"a": 2}, 1, 1))
