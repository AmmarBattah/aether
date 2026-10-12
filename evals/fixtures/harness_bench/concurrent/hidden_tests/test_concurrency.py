import threading
import unittest
from concurrent.futures import Future

from txstore import Store, Conflict, Invalid, ReentrantCommit
from support import Journal


def launch(function):
    result = Future()

    def run():
        try:
            result.set_result(function())
        except BaseException as error:
            result.set_exception(error)

    thread = threading.Thread(target=run, daemon=True)
    thread.start()
    return result


class GatedJournal(Journal):
    def __init__(self):
        super().__init__()
        self.entered = threading.Event()
        self.release = threading.Event()
        self.call_count = 0
        self.first_failure = None
        self.callback = None
        self.lock = threading.Lock()

    def append(self, record):
        with self.lock:
            self.call_count += 1
            first = self.call_count == 1
        if first:
            self.entered.set()
            if self.callback:
                self.callback()
            if not self.release.wait(3):
                raise TimeoutError("test append gate was not released")
            if self.first_failure:
                raise self.first_failure
        super().append(record)


class ConcurrencyTests(unittest.TestCase):
    def fixture(self):
        journal = GatedJournal()
        self.addCleanup(journal.release.set)
        return Store(journal), journal

    def held_commit(self, transaction, journal):
        result = launch(transaction.commit)
        self.assertTrue(journal.entered.wait(1), "append never started")
        return result

    def queued_commit(self, transaction):
        started = threading.Event()
        finished = threading.Event()

        def commit():
            started.set()
            return transaction.commit()

        result = launch(commit)
        result.add_done_callback(lambda _: finished.set())
        self.assertTrue(started.wait(1), "contender thread did not start")
        # The first append remains gated for this bounded observation window.
        # A queued committer must neither publish nor reject on an in-flight
        # transaction's unpublished state.
        self.assertFalse(finished.wait(0.1), "contender finished before prior append")
        return result

    def test_reads_and_begin_progress_during_append(self):
        store, journal = self.fixture()
        tx = store.begin("writer")
        tx.put("a", 1)
        commit = self.held_commit(tx, journal)
        reader = launch(lambda: (store.snapshot(), store.revision, store.begin("reader")))
        try:
            state, revision, snapshot = reader.result(timeout=1)
            self.assertEqual((state, revision), ({}, 0))
            self.assertIsNone(snapshot.get("a"))
        finally:
            journal.release.set()
        commit.result(timeout=1)
        self.assertIsNone(snapshot.get("a"))
        with self.assertRaises(Conflict):
            snapshot.commit()

    def test_callback_can_read_and_begin_without_deadlock(self):
        store, journal = self.fixture()
        observations = []
        journal.callback = lambda: observations.append(
            (store.snapshot(), store.revision, store.begin("reader").scan()))
        journal.release.set()
        tx = store.begin("write")
        tx.put("a", 1)
        launch(tx.commit).result(timeout=1)
        self.assertEqual(observations, [({}, 0, [])])

    def test_reentrant_commit_is_rejected_and_remains_retryable(self):
        store, journal = self.fixture()
        nested = store.begin("nested")
        nested.put("b", 2)
        errors = []

        def callback():
            try:
                nested.commit()
            except Exception as error:
                errors.append(error)

        journal.callback = callback
        journal.release.set()
        outer = store.begin("outer")
        outer.put("a", 1)
        launch(outer.commit).result(timeout=1)
        self.assertEqual(len(errors), 1)
        self.assertIsInstance(errors[0], ReentrantCommit)
        self.assertEqual(store.snapshot(), {"a": 1})
        self.assertEqual(nested.commit()["revision"], 2)

    def competing(self, first_writes, second_writes, same_id=False, read_prefix=None):
        store, journal = self.fixture()
        first = store.begin("one")
        second = store.begin("one" if same_id else "two")
        if read_prefix is not None:
            second.scan(read_prefix)
        for key, value in first_writes.items():
            first.put(key, value)
        for key, value in second_writes.items():
            second.put(key, value)
        leader = self.held_commit(first, journal)
        contender = self.queued_commit(second)
        journal.release.set()
        return store, journal, leader.result(timeout=1), contender

    def test_disjoint_commits_get_distinct_revisions_and_replay(self):
        store, journal, first, second = self.competing({"a": 1}, {"b": 2})
        self.assertEqual((first["revision"], second.result(timeout=1)["revision"]), (1, 2))
        self.assertEqual(store.snapshot(), {"a": 1, "b": 2})
        recovered = Store(journal)
        self.assertEqual(recovered.snapshot(), store.snapshot())
        self.assertEqual(recovered.revision, 2)

    def test_conflicting_committer_revalidates_before_append(self):
        store, journal, _, second = self.competing({"a": 1}, {"a": 2})
        with self.assertRaises(Conflict):
            second.result(timeout=1)
        self.assertEqual(journal.call_count, 1)
        self.assertEqual(store.snapshot(), {"a": 1})

    def test_range_reader_revalidates_phantom_before_append(self):
        store, journal, _, second = self.competing({"a": 1}, {"b": 2}, read_prefix="a")
        with self.assertRaises(Conflict):
            second.result(timeout=1)
        self.assertEqual(journal.call_count, 1)

    def test_same_id_identical_writes_append_once(self):
        store, journal, first, second = self.competing({"a": [1]}, {"a": [1]}, same_id=True)
        duplicate = second.result(timeout=1)
        self.assertEqual(duplicate, first)
        duplicate["writes"][0][2].append(2)
        self.assertEqual(store.snapshot(), {"a": [1]})
        self.assertEqual((journal.call_count, store.revision), (1, 1))

    def test_same_id_different_writes_fail_without_append(self):
        store, journal, _, second = self.competing({"a": True}, {"a": 1}, same_id=True)
        with self.assertRaises(Invalid):
            second.result(timeout=1)
        self.assertEqual((journal.call_count, store.revision), (1, 1))

    def test_failed_append_releases_slot_and_retry_rechecks_conflict(self):
        store, journal = self.fixture()
        failure = OSError("injected append failure")
        journal.first_failure = failure
        first, second = store.begin("one"), store.begin("two")
        first.put("a", 1)
        second.put("a", 2)
        leader = self.held_commit(first, journal)
        contender = self.queued_commit(second)
        journal.release.set()
        with self.assertRaises(OSError) as caught:
            leader.result(timeout=1)
        self.assertIs(caught.exception, failure)
        self.assertEqual(contender.result(timeout=1)["revision"], 1)
        with self.assertRaises(Conflict):
            first.commit()
        self.assertEqual(Store(journal).snapshot(), {"a": 2})
