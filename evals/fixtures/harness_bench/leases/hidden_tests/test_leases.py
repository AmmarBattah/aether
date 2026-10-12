import copy
import unittest
from leasepool import Pool, Conflict, StaleLease


class LeaseTests(unittest.TestCase):
    def setUp(self):
        self.records = []
        self.pool = Pool(self.records.append)

    def request(self, key="a", resources=None, priority=0, ttl=5, now=0):
        return self.pool.request(key, resources or ["x"], priority, ttl, now)

    def test_deadline_equality_expires(self):
        self.request()
        self.pool.advance(5)
        self.assertEqual(self.pool.get("a")["state"], "expired")

    def test_failed_release_does_not_advance_time(self):
        lease = self.request()
        before = self.pool.snapshot()
        with self.assertRaises(StaleLease):
            self.pool.release("a", lease["fences"], 5)
        self.assertEqual(self.pool.snapshot(), before)
        self.assertEqual(len(self.records), 1)

    def test_blocked_request_reserves_all_resources(self):
        self.request("holder", ["x"])
        self.request("wide", ["x", "y"], priority=10)
        self.assertEqual(self.request("small", ["y"])["state"], "queued")
        self.assertEqual(self.request("free", ["z"])["state"], "active")

    def test_priority_beats_arrival(self):
        held = self.request("holder")
        self.request("low", priority=-1)
        self.request("high", priority=10)
        self.pool.release("holder", held["fences"], 1)
        self.assertEqual(self.pool.get("high")["state"], "active")
        self.assertEqual(self.pool.get("low")["state"], "queued")

    def test_equal_priority_is_fifo(self):
        self.request("holder", ttl=1)
        self.request("first")
        self.request("second")
        self.assertEqual([x["key"] for x in self.pool.advance(1)], ["first"])

    def test_idempotent_request_still_journals(self):
        first = self.request(resources=["y", "x"])
        self.assertEqual(self.request(resources=["x", "y"]), first)
        self.assertEqual(len(self.records), 2)
        self.assertEqual(self.records[0]["arguments"]["resources"], ["x", "y"])

    def test_conflict_does_not_expire_others(self):
        self.request()
        before = self.pool.snapshot()
        with self.assertRaises(Conflict):
            self.request(ttl=6, now=9)
        self.assertEqual(self.pool.snapshot(), before)

    def test_cancelled_identity_cannot_be_reused(self):
        self.request()
        self.assertTrue(self.pool.cancel("a", 1))
        self.assertEqual(self.request(now=2)["state"], "cancelled")
        self.assertEqual(self.pool.snapshot()["counters"], {"x": 1})

    def test_unknown_cancel_advances_and_grants(self):
        self.request("holder", ttl=1)
        self.request("waiter")
        self.assertFalse(self.pool.cancel("missing", 1))
        self.assertEqual(self.pool.get("waiter")["state"], "active")
        self.assertEqual(len(self.records), 3)

    def test_renew_preserves_original_identity(self):
        first = self.request()
        renewed = self.pool.renew("a", first["fences"], 20, 1)
        self.assertEqual(renewed["ttl"], 5)
        self.assertEqual(renewed["deadline"], 21)
        self.assertEqual(self.request(now=2)["deadline"], 21)

    def test_boolean_fence_is_rejected(self):
        self.request()
        with self.assertRaises(ValueError):
            self.pool.release("a", {"x": True}, 1)

    def test_old_fences_cannot_release_successor(self):
        first = self.request("old", ttl=1)
        self.request("new")
        self.pool.advance(1)
        with self.assertRaises(StaleLease):
            self.pool.release("new", first["fences"], 1)
        self.assertEqual(self.pool.get("new")["fences"], {"x": 2})

    def test_boolean_priority_rejected(self):
        with self.assertRaises(ValueError):
            self.request(priority=True)
        self.assertEqual(self.records, [])

    def test_backwards_time_rejected(self):
        self.pool.advance(3)
        with self.assertRaises(ValueError):
            self.pool.advance(2)
        self.assertEqual(len(self.records), 1)

    def test_defensive_returns(self):
        result = self.request()
        result["fences"]["x"] = 900
        self.pool.get("a")["resources"].clear()
        self.pool.snapshot()["requests"].clear()
        self.assertEqual(self.pool.get("a")["fences"], {"x": 1})
        self.assertEqual(self.pool.get("a")["resources"], ["x"])

    def test_callback_cannot_modify_published_state(self):
        pool = Pool(lambda record: record["arguments"]["resources"].clear())
        self.assertEqual(pool.request("a", ["x"], 0, 1, 0)["resources"], ["x"])

    def test_append_failure_rolls_back_and_retries_identically(self):
        attempts = []
        def append(record):
            attempts.append(copy.deepcopy(record))
            if len(attempts) == 1:
                raise OSError("injected")
        pool = Pool(append)
        before = pool.snapshot()
        with self.assertRaises(OSError):
            pool.request("a", ["x"], 0, 1, 0)
        self.assertEqual(pool.snapshot(), before)
        pool.request("a", ["x"], 0, 1, 0)
        self.assertEqual(attempts[0], attempts[1])

    def test_replay_is_silent_and_equivalent(self):
        first = self.request()
        self.request("b", ["x", "y"])
        self.pool.release("a", first["fences"], 1)
        untouched = copy.deepcopy(self.records)
        replayed = Pool(lambda _: self.fail("replay appended"), self.records)
        self.assertEqual(replayed.snapshot(), self.pool.snapshot())
        self.assertEqual(self.records, untouched)

    def test_replay_rejects_sequence_gap(self):
        self.request()
        self.records[0]["sequence"] = 2
        with self.assertRaises(ValueError):
            Pool(lambda _: None, self.records)

    def test_replay_rejects_noncanonical_resources(self):
        self.request(resources=["x", "y"])
        self.records[0]["arguments"]["resources"] = ["y", "x"]
        with self.assertRaises(ValueError):
            Pool(lambda _: None, self.records)
