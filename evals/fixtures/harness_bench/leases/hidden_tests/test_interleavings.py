import copy
import random
import unittest

from leasepool import Pool, StaleLease


class InterleavingTests(unittest.TestCase):
    def test_each_mutator_is_atomic_when_append_fails(self):
        for operation in ("request", "advance", "renew", "release", "cancel"):
            with self.subTest(operation=operation):
                records, attempts = [], []
                fail = False
                def append(record):
                    attempts.append(copy.deepcopy(record))
                    if fail:
                        self.assertEqual(pool.snapshot(), before)
                        record["arguments"].clear()
                        raise OSError("injected append failure")
                    records.append(copy.deepcopy(record))
                pool = Pool(append)
                lease = pool.request("holder", ["x", "y"], 0, 5, 0)
                pool.request("waiter", ["y", "z"], 1, 10, 0)
                args = {
                    "request": ("new", ["z"], 2, 10, 1),
                    "advance": (5,),
                    "renew": ("holder", lease["fences"], 10, 1),
                    "release": ("holder", lease["fences"], 1),
                    "cancel": ("holder", 1),
                }[operation]
                before = pool.snapshot()
                count = len(records)
                fail = True
                with self.assertRaises(OSError):
                    getattr(pool, operation)(*args)
                self.assertEqual(pool.snapshot(), before)
                self.assertEqual(len(records), count)
                fail = False
                getattr(pool, operation)(*args)
                self.assertEqual(attempts[-2], attempts[-1])
                replayed = Pool(lambda _: self.fail("replay wrote"), records)
                self.assertEqual(replayed.snapshot(), pool.snapshot())

    def test_callback_observes_precommit_state(self):
        observations = []
        pool = Pool(lambda _: observations.append(pool.snapshot()))
        before = pool.snapshot()
        pool.request("a", ["x"], 0, 5, 0)
        self.assertEqual(observations, [before])
        before = pool.snapshot()
        pool.advance(5)
        self.assertEqual(observations[-1], before)

    def test_transitive_reservations_do_not_block_disjoint_work(self):
        pool = Pool(lambda _: None)
        held = pool.request("holder", ["a"], 0, 10, 0)
        pool.request("first", ["a", "b"], 30, 10, 0)
        pool.request("second", ["b", "c"], 20, 10, 0)
        self.assertEqual(pool.request("third", ["c"], 10, 10, 0)["state"], "queued")
        self.assertEqual(pool.request("free", ["d"], -100, 10, 0)["state"], "active")
        pool.release("holder", held["fences"], 1)
        self.assertEqual(pool.get("first")["state"], "active")
        self.assertEqual(pool.get("third")["state"], "queued")
        pool.cancel("second", 1)
        self.assertEqual(pool.get("third")["state"], "active")
        self.assertEqual(pool.snapshot()["counters"], {"a": 2, "b": 1, "c": 1, "d": 1})

    def test_expired_renewal_cannot_expire_unrelated_lease(self):
        pool = Pool(lambda _: None)
        lease = pool.request("a", ["a"], 0, 1, 0)
        pool.request("b", ["b"], 0, 1, 0)
        before = pool.snapshot()
        with self.assertRaises(StaleLease):
            pool.renew("a", lease["fences"], 100, 1)
        self.assertEqual(pool.snapshot(), before)

    def test_malformed_replay_never_appends_or_mutates_records(self):
        good = {"sequence": 1, "operation": "advance", "arguments": {"now": 0}}
        bad_records = [
            dict(good, sequence=True), dict(good, extra=1),
            dict(good, operation="snapshot"), dict(good, arguments={"now": 0, "extra": 1}),
            dict(good, arguments={"now": True}), dict(good, arguments={}),
            dict(good, operation="request", arguments=dict(key="a", resources=["x", "x"], priority=0, ttl=1, now=0)),
        ]
        for bad in bad_records:
            with self.subTest(record=bad):
                before = copy.deepcopy(bad)
                with self.assertRaises(ValueError):
                    Pool(lambda _: self.fail("invalid replay appended"), [bad])
                self.assertEqual(bad, before)
        with self.assertRaises(ValueError):
            Pool(lambda _: self.fail("replay appended"), [good, good])

    def test_input_and_retained_journal_objects_do_not_alias_state(self):
        records = []
        resources = ["x"]
        pool = Pool(records.append)
        lease = pool.request("a", resources, 0, 5, 0)
        resources.append("y")
        records[0]["arguments"]["resources"].append("z")
        fences = lease["fences"]
        pool.renew("a", fences, 10, 1)
        fences["x"] = 800
        records[1]["arguments"]["fences"]["x"] = 900
        self.assertEqual(pool.get("a")["resources"], ["x"])
        self.assertEqual(pool.get("a")["fences"], {"x": 1})

    def test_random_interleavings_preserve_exclusion_fences_and_replay(self):
        rng = random.Random(91827)
        records = []
        pool = Pool(records.append)
        seen_grants, counters = set(), {}
        now = 0
        for step in range(100):
            now += rng.randrange(2)
            pool.advance(now)
            active = [item for item in pool.snapshot()["requests"] if item["state"] == "active"]
            choice = rng.randrange(4)
            if choice == 0 and active:
                item = rng.choice(active)
                pool.renew(item["key"], item["fences"], rng.randrange(1, 8), now)
            elif choice == 1 and active:
                item = rng.choice(active)
                pool.release(item["key"], item["fences"], now)
            elif choice == 2:
                pool.cancel(f"job{rng.randrange(step + 1)}", now)
            else:
                pool.request(f"job{step}", rng.sample(list("abcde"), rng.randrange(1, 4)),
                             rng.randrange(-2, 3), rng.randrange(1, 8), now)
            snapshot = pool.snapshot()
            occupied = set()
            for item in snapshot["requests"]:
                if item["state"] == "active":
                    self.assertGreater(item["deadline"], now)
                    self.assertFalse(occupied.intersection(item["resources"]))
                    occupied.update(item["resources"])
                if item["fences"] and item["key"] not in seen_grants:
                    seen_grants.add(item["key"])
                    for resource, fence in item["fences"].items():
                        counters[resource] = max(counters.get(resource, 0), fence)
                if item["state"] == "queued":
                    self.assertEqual(item["fences"], {})
            self.assertEqual(snapshot["counters"], counters)
            self.assertEqual(snapshot["sequence"], len(records))
            replayed = Pool(lambda _: self.fail("replay appended"), copy.deepcopy(records))
            self.assertEqual(replayed.snapshot(), snapshot)

    def test_replay_continuation_keeps_fencing_counters(self):
        records = []
        pool = Pool(records.append)
        for index in range(20):
            lease = pool.request(str(index), ["a", "b"], 0, 1, index)
            self.assertEqual(lease["fences"], {"a": index + 1, "b": index + 1})
            pool = Pool(records.append, copy.deepcopy(records))
        self.assertEqual(len(records), 20)
