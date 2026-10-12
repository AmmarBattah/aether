import copy
import random
import unittest

from matchbook import Engine, Forbidden, Invalid
from support import Journal, engine, pairs, status


def busy_history(eng):
    eng.submit("s1", "ann", "sell", 101, 5, 1)
    eng.submit("s2", "cat", "sell", 102, 5, 1, tif="GTD", expires=6)
    eng.submit("b1", "bob", "buy", 101, 2, 2)
    eng.submit("b2", "ann", "buy", 101, 4, 3, stp="decrement")
    eng.submit("b3", "bob", "buy", 99, 3, 3, post_only=True)
    eng.amend("b3", "bob", 4, price=100, quantity=6)
    eng.submit("x1", "dan", "sell", None, 2, 5, tif="FOK")
    eng.cancel("b3", "bob", 5)
    eng.advance(7)


class JournalTests(unittest.TestCase):
    def test_every_accepted_operation_appends_one_exact_record(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 1)
        eng.submit("b1", "bob", "buy", 100, 1, 2, tif="GTD", expires=9, post_only=False, stp="decrement")
        eng.amend("s1", "ann", 3, quantity=2)
        eng.cancel("s1", "ann", 4)
        eng.advance(5)
        self.assertEqual(journal.rows, [
            {"sequence": 1, "operation": "submit", "arguments": {
                "order_id": "s1", "owner": "ann", "side": "sell", "price": 100, "quantity": 5, "now": 1,
                "tif": "GTC", "expires": None, "post_only": False, "stp": "cancel_newest"}},
            {"sequence": 2, "operation": "submit", "arguments": {
                "order_id": "b1", "owner": "bob", "side": "buy", "price": 100, "quantity": 1, "now": 2,
                "tif": "GTD", "expires": 9, "post_only": False, "stp": "decrement"}},
            {"sequence": 3, "operation": "amend", "arguments": {
                "order_id": "s1", "owner": "ann", "now": 3, "price": None, "quantity": 2}},
            {"sequence": 4, "operation": "cancel", "arguments": {"order_id": "s1", "owner": "ann", "now": 4}},
            {"sequence": 5, "operation": "advance", "arguments": {"now": 5}},
        ])
        self.assertEqual(eng.snapshot()["sequence"], 5)

    def test_failed_operations_append_nothing(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 1)
        with self.assertRaises(Forbidden):
            eng.cancel("s1", "bob", 9)
        with self.assertRaises(Invalid):
            eng.advance(0)
        self.assertEqual(len(journal.rows), 1)
        self.assertEqual((eng.snapshot()["sequence"], eng.snapshot()["clock"]), (1, 1))

    def test_append_failure_during_a_trade_publishes_nothing(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 1)
        eng.submit("s2", "cat", "sell", 100, 5, 1, tif="GTD", expires=3)
        before = eng.snapshot()
        journal.fail = OSError("disk full")
        with self.assertRaises(OSError) as raised:
            eng.submit("b1", "bob", "buy", 100, 7, 3)
        self.assertIs(raised.exception, journal.fail)
        self.assertEqual(eng.snapshot(), before)
        self.assertIsNone(eng.order("b1"))
        journal.fail = None
        report = eng.submit("b1", "bob", "buy", 100, 7, 3)
        self.assertEqual((pairs(report["trades"]), report["expired"]), ([("b1", "s1", 100, 5)], ["s2"]))
        self.assertEqual((journal.rows[-1]["sequence"], eng.trades()[0]["id"]), (3, 1))

    def test_append_failure_during_amend_cancel_and_advance(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 1, tif="GTD", expires=8)
        eng.submit("s2", "cat", "sell", 100, 5, 1)
        before = eng.snapshot()
        journal.fail = RuntimeError("down")
        for call in (lambda: eng.amend("s1", "ann", 2, quantity=9), lambda: eng.cancel("s2", "cat", 2),
                     lambda: eng.advance(9)):
            with self.assertRaises(RuntimeError):
                call()
        self.assertEqual(eng.snapshot(), before)
        self.assertEqual(eng.queue("sell", 100), ["s1", "s2"])
        journal.fail = None
        self.assertEqual(eng.advance(9)["expired"], ["s1"])
        self.assertEqual(journal.rows[-1], {"sequence": 3, "operation": "advance", "arguments": {"now": 9}})

    def test_callback_may_mutate_its_record(self):
        eng, journal = engine()
        journal.scribble = True
        eng.submit("s1", "ann", "sell", 100, 5, 1)
        eng.submit("b1", "bob", "buy", 100, 2, 2)
        self.assertEqual(status(eng, "s1"), ("open", None, 2, 3))
        self.assertEqual([row["sequence"] for row in journal.rows], [1, 2])
        self.assertEqual(journal.rows[1]["arguments"]["order_id"], "b1")
        self.assertEqual(Engine(None, journal.rows).snapshot(), eng.snapshot())

    def test_replay_reproduces_the_snapshot(self):
        eng, journal = engine()
        busy_history(eng)
        replayed = Engine(None, journal.rows)
        self.assertEqual(replayed.snapshot(), eng.snapshot())
        self.assertEqual(replayed.book(), eng.book())
        self.assertEqual(eng.snapshot()["sequence"], 9)
        self.assertEqual(pairs(eng.trades()), [("b1", "s1", 101, 2), ("b2", "x1", 101, 1), ("b3", "x1", 100, 1)])

    def test_replay_neither_appends_nor_mutates_its_input(self):
        eng, journal = engine()
        busy_history(eng)
        original = copy.deepcopy(journal.rows)
        second = Journal()
        replayed = Engine(second.append, tuple(journal.rows))
        self.assertEqual(second.rows, [])
        self.assertEqual(journal.rows, original)
        replayed.advance(8)
        self.assertEqual(second.rows, [{"sequence": 10, "operation": "advance", "arguments": {"now": 8}}])

    def test_replayed_engine_keeps_queue_places_and_trade_ids(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 1)
        eng.submit("s2", "cat", "sell", 100, 2, 1)
        eng.amend("s1", "ann", 2, quantity=3)
        eng.submit("b0", "bob", "buy", 100, 1, 2)
        replayed = Engine(None, journal.rows)
        self.assertEqual(replayed.queue("sell", 100), ["s2", "s1"])
        report = replayed.submit("b1", "bob", "buy", 100, 2, 3)
        self.assertEqual(pairs(report["trades"]), [("b1", "s2", 100, 1), ("b1", "s1", 100, 1)])
        self.assertEqual([t["id"] for t in report["trades"]], [2, 3])

    def test_replay_rejects_a_sequence_gap(self):
        self.assert_rejected(lambda rows: rows[3].update(sequence=5))

    def test_replay_rejects_a_missing_first_record(self):
        self.assert_rejected(lambda rows: rows.pop(0))

    def test_replay_rejects_an_unknown_operation(self):
        self.assert_rejected(lambda rows: rows[8].update(operation="tick"))

    def test_replay_rejects_extra_and_missing_keys(self):
        self.assert_rejected(lambda rows: rows[2].update(note="x"))
        self.assert_rejected(lambda rows: rows[2].pop("operation"))
        self.assert_rejected(lambda rows: rows.__setitem__(2, list(rows[2].items())))

    def test_replay_rejects_wrong_argument_names(self):
        self.assert_rejected(lambda rows: rows[8]["arguments"].update(extra=1))
        self.assert_rejected(lambda rows: rows[0]["arguments"].pop("stp"))
        self.assert_rejected(lambda rows: rows[7].update(arguments=None))

    def test_replay_rejects_an_operation_that_would_fail(self):
        self.assert_rejected(lambda rows: rows[7]["arguments"].update(owner="mallory"))
        self.assert_rejected(lambda rows: rows[2]["arguments"].update(order_id="s1"))
        self.assert_rejected(lambda rows: rows[8]["arguments"].update(now=2))
        self.assert_rejected(lambda rows: rows[0]["arguments"].update(quantity=0))

    def assert_rejected(self, damage):
        eng, journal = engine()
        busy_history(eng)
        rows = copy.deepcopy(journal.rows)
        self.assertEqual(Engine(None, rows).snapshot(), eng.snapshot())
        damage(rows)
        with self.assertRaises(Invalid):
            Engine(None, rows)


class InvariantTests(unittest.TestCase):
    def run_random_history(self, seed):
        rng = random.Random(seed)
        eng, journal = engine()
        now, ids = 0, []
        for step in range(120):
            now += rng.choice((0, 0, 1, 2))
            roll = rng.random()
            if roll < 0.7 or not ids:
                tif = rng.choice(("GTC", "GTC", "GTD", "IOC", "FOK"))
                price = None if tif in ("IOC", "FOK") and rng.random() < 0.3 else rng.randint(95, 105)
                order_id = f"o{step}"
                eng.submit(order_id, rng.choice("abc"), rng.choice(("buy", "sell")), price, rng.randint(1, 9), now,
                           tif=tif, expires=now + rng.randint(1, 6) if tif == "GTD" else None,
                           post_only=tif in ("GTC", "GTD") and rng.random() < 0.2,
                           stp=rng.choice(("cancel_newest", "cancel_oldest", "decrement")))
                ids.append(order_id)
                continue
            target = eng.order(rng.choice(ids))
            if target["status"] != "open" or (target["expires"] is not None and target["expires"] <= now):
                eng.advance(now)
            elif roll < 0.85:
                eng.amend(target["id"], target["owner"], now, price=rng.randint(95, 105), quantity=rng.randint(1, 9))
            else:
                eng.cancel(target["id"], target["owner"], now)
        return eng, journal

    def test_book_is_never_crossed_and_levels_match_open_orders(self):
        for seed in (1, 2, 3):
            eng, _ = self.run_random_history(seed)
            book, snap = eng.book(), eng.snapshot()
            if book["bids"] and book["asks"]:
                self.assertLess(book["bids"][0][0], book["asks"][0][0], seed)
            open_orders = [o for o in snap["orders"] if o["status"] == "open"]
            self.assertEqual(sum(level[1] for level in book["bids"] + book["asks"]),
                             sum(o["remaining"] for o in open_orders), seed)
            self.assertEqual(sum(level[2] for level in book["bids"] + book["asks"]), len(open_orders), seed)
            self.assertGreater(len(snap["trades"]), 10, seed)

    def test_filled_quantities_equal_the_trade_tape(self):
        for seed in (4, 5, 6):
            eng, _ = self.run_random_history(seed)
            snap = eng.snapshot()
            traded = {}
            for trade in snap["trades"]:
                self.assertNotEqual(eng.order(trade["buy"])["owner"], eng.order(trade["sell"])["owner"], seed)
                for side in ("buy", "sell"):
                    traded[trade[side]] = traded.get(trade[side], 0) + trade["quantity"]
            for order in snap["orders"]:
                self.assertEqual(order["filled"], traded.get(order["id"], 0), (seed, order["id"]))
                self.assertEqual(order["remaining"] == 0, order["status"] != "open", (seed, order["id"]))
            self.assertEqual([t["id"] for t in snap["trades"]], list(range(1, len(snap["trades"]) + 1)), seed)
            self.assertGreater(len(snap["trades"]), 10, seed)

    def test_random_histories_replay_exactly(self):
        for seed in (7, 8, 9):
            eng, journal = self.run_random_history(seed)
            self.assertEqual(len(journal.rows), 120, seed)
            replayed = Engine(None, journal.rows)
            self.assertEqual(replayed.snapshot(), eng.snapshot(), seed)
            self.assertEqual(replayed.book(), eng.book(), seed)


if __name__ == "__main__":
    unittest.main()
