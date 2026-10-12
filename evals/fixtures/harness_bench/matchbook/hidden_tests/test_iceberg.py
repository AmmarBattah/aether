import random
import unittest

from matchbook import Conflict, Engine, Forbidden, Invalid
from support import engine, pairs, status

VIEW_FIELDS = {"id", "owner", "side", "price", "quantity", "filled", "remaining", "tif", "expires", "status", "reason"}


def shelf():
    """Asks at 100: ann's iceberg of 6 showing 2, then bob's plain order of 3."""
    eng, journal = engine()
    eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
    eng.submit("s2", "bob", "sell", 100, 3, 2)
    return eng, journal


def sells(trades):
    return [(trade["sell"], trade["quantity"]) for trade in trades]


class IcebergRestingTests(unittest.TestCase):
    def test_it_rests_showing_one_tranche(self):
        eng, _ = engine()
        report = eng.submit_iceberg("i1", "ann", "sell", 100, 10, 3, 1)
        self.assertEqual(report, {"order": {
            "id": "i1", "owner": "ann", "side": "sell", "price": 100, "quantity": 10, "filled": 0,
            "remaining": 10, "tif": "GTC", "expires": None, "status": "open", "reason": None},
            "trades": [], "cancelled": [], "expired": []})
        self.assertEqual(eng.iceberg("i1"), {"display": 3, "visible": 3, "hidden": 7})
        self.assertEqual(eng.book(), {"bids": [], "asks": [[100, 3, 1]]})

    def test_display_equal_to_quantity_hides_nothing(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "buy", 100, 4, 4, 1)
        self.assertEqual(eng.iceberg("i1"), {"display": 4, "visible": 4, "hidden": 0})
        self.assertEqual(eng.book(), {"bids": [[100, 4, 1]], "asks": []})

    def test_the_incoming_iceberg_matches_with_its_whole_quantity(self):
        eng, _ = engine()
        eng.submit("b1", "bob", "buy", 100, 4, 1)
        eng.submit("b2", "cat", "buy", 99, 4, 1)
        report = eng.submit_iceberg("i1", "ann", "sell", 99, 10, 1, 2)
        self.assertEqual(pairs(report["trades"]), [("b1", "i1", 100, 4), ("b2", "i1", 99, 4)])
        self.assertEqual({trade["taker"] for trade in report["trades"]}, {"i1"})
        self.assertEqual(status(eng, "i1"), ("open", None, 8, 2))
        self.assertEqual(eng.iceberg("i1"), {"display": 1, "visible": 1, "hidden": 1})

    def test_the_book_counts_only_visible_quantity(self):
        eng, _ = shelf()
        eng.submit_iceberg("i3", "cat", "sell", 100, 9, 4, 3)
        eng.submit_iceberg("i4", "dan", "sell", 101, 5, 1, 3)
        self.assertEqual(eng.book()["asks"], [[100, 9, 3], [101, 1, 1]])
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2", "i3"])
        self.assertEqual(eng.order("i3")["remaining"], 9)

    def test_iceberg_read_is_none_for_other_orders(self):
        eng, _ = shelf()
        self.assertIsNone(eng.iceberg("s2"))
        self.assertIsNone(eng.iceberg("nope"))
        self.assertEqual(set(eng.order("i1")), VIEW_FIELDS)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})

    def test_the_iceberg_read_is_a_copy(self):
        eng, _ = shelf()
        eng.iceberg("i1")["visible"] = 99
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})


class IcebergMatchingTests(unittest.TestCase):
    def test_a_partly_taken_tranche_keeps_its_place(self):
        eng, _ = shelf()
        report = eng.submit("b1", "cat", "buy", 100, 1, 3)
        self.assertEqual(sells(report["trades"]), [("i1", 1)])
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 4})
        self.assertEqual(eng.book()["asks"], [[100, 4, 2]])

    def test_an_emptied_tranche_refills_at_the_back(self):
        eng, _ = shelf()
        report = eng.submit("b1", "cat", "buy", 100, 2, 3)
        self.assertEqual(sells(report["trades"]), [("i1", 2)])
        self.assertEqual(eng.queue("sell", 100), ["s2", "i1"])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 2})
        self.assertEqual(eng.book()["asks"], [[100, 5, 2]])
        self.assertEqual(status(eng, "i1"), ("open", None, 2, 4))

    def test_one_order_trades_with_the_same_iceberg_tranche_by_tranche(self):
        eng, _ = shelf()
        report = eng.submit("b1", "cat", "buy", 100, 8, 3)
        self.assertEqual(sells(report["trades"]), [("i1", 2), ("s2", 3), ("i1", 2), ("i1", 1)])
        self.assertEqual([trade["id"] for trade in report["trades"]], [1, 2, 3, 4])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 0})
        self.assertEqual(status(eng, "i1"), ("open", None, 5, 1))
        self.assertEqual(status(eng, "b1"), ("filled", None, 8, 0))

    def test_the_last_tranche_may_be_smaller_than_display(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 5, 2, 1)
        eng.submit("b1", "cat", "buy", 100, 4, 2)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 0})
        self.assertEqual(eng.book()["asks"], [[100, 1, 1]])

    def test_an_exhausted_iceberg_is_filled(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        report = eng.submit("b1", "cat", "buy", 100, 6, 2)
        self.assertEqual(sells(report["trades"]), [("i1", 2), ("i1", 2), ("i1", 2)])
        self.assertEqual(status(eng, "i1"), ("filled", None, 6, 0))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 0, "hidden": 0})
        self.assertEqual(eng.book()["asks"], [])

    def test_price_priority_comes_before_refills(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 4, 1, 1)
        eng.submit("s2", "bob", "sell", 101, 2, 1)
        report = eng.submit("b1", "cat", "buy", 101, 5, 2)
        self.assertEqual(pairs(report["trades"]), [("b1", "i1", 100, 1)] * 4 + [("b1", "s2", 101, 1)])
        self.assertEqual(status(eng, "i1"), ("filled", None, 4, 0))

    def test_two_icebergs_at_one_level_take_turns(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "buy", 100, 4, 2, 1)
        eng.submit_iceberg("i2", "bob", "buy", 100, 3, 1, 1)
        report = eng.submit("s1", "cat", "sell", 100, 6, 2)
        self.assertEqual([(trade["buy"], trade["quantity"]) for trade in report["trades"]],
                         [("i1", 2), ("i2", 1), ("i1", 2), ("i2", 1)])
        self.assertEqual(eng.iceberg("i2"), {"display": 1, "visible": 1, "hidden": 0})
        self.assertEqual(eng.book()["bids"], [[100, 1, 1]])

    def test_immediate_or_cancel_walks_the_tranches(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 4, 1, 1)
        report = eng.submit("b1", "cat", "buy", 100, 5, 2, tif="IOC")
        self.assertEqual(sells(report["trades"]), [("i1", 1)] * 4)
        self.assertEqual(status(eng, "b1"), ("cancelled", "ioc", 4, 0))
        self.assertEqual(status(eng, "i1"), ("filled", None, 4, 0))

    def test_fill_or_kill_counts_the_refills(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        rejected = eng.submit("b1", "cat", "buy", 100, 7, 2, tif="FOK")
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual(rejected["trades"], [])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})
        filled = eng.submit("b2", "cat", "buy", 100, 6, 3, tif="FOK")
        self.assertEqual(sells(filled["trades"]), [("i1", 2)] * 3)
        self.assertEqual(status(eng, "b2"), ("filled", None, 6, 0))

    def test_post_only_is_rejected_against_a_visible_tranche(self):
        eng, _ = shelf()
        eng.submit("b1", "cat", "buy", 100, 1, 3, post_only=True)
        eng.submit("b2", "cat", "buy", 99, 1, 3, post_only=True)
        self.assertEqual(status(eng, "b1"), ("rejected", "post_only", 0, 0))
        self.assertEqual(status(eng, "b2"), ("open", None, 0, 1))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})

    def test_an_iceberg_submit_applies_expiry_first(self):
        eng, _ = engine()
        eng.submit("g1", "bob", "buy", 100, 5, 1, tif="GTD", expires=5)
        report = eng.submit_iceberg("i1", "ann", "sell", 100, 4, 2, 5)
        self.assertEqual(report["expired"], ["g1"])
        self.assertEqual(report["trades"], [])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 2})


class IcebergSelfTradeTests(unittest.TestCase):
    def test_cancel_newest_leaves_the_iceberg_untouched(self):
        eng, _ = shelf()
        report = eng.submit("b1", "ann", "buy", 100, 5, 3)
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(report["cancelled"], [])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])

    def test_cancel_oldest_cancels_the_reserve_too(self):
        eng, _ = shelf()
        report = eng.submit("b1", "ann", "buy", 100, 3, 3, stp="cancel_oldest")
        self.assertEqual(report["cancelled"], ["i1"])
        self.assertEqual(sells(report["trades"]), [("s2", 3)])
        self.assertEqual(status(eng, "i1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 0, "hidden": 0})

    def test_decrement_empties_a_tranche_which_refills_at_the_back(self):
        eng, _ = shelf()
        report = eng.submit("b1", "ann", "buy", 100, 4, 3, stp="decrement")
        self.assertEqual(sells(report["trades"]), [("s2", 2)])
        self.assertEqual(report["cancelled"], [])
        self.assertEqual(status(eng, "b1"), ("filled", None, 2, 0))
        self.assertEqual(status(eng, "i1"), ("open", None, 0, 4))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 2})
        self.assertEqual(eng.queue("sell", 100), ["s2", "i1"])

    def test_decrement_of_part_of_a_tranche_keeps_the_place(self):
        eng, _ = shelf()
        report = eng.submit("b1", "ann", "buy", 100, 1, 3, stp="decrement")
        self.assertEqual(report["trades"], [])
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 4})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])

    def test_decrement_can_consume_the_whole_iceberg(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 3, 2, 1)
        report = eng.submit("b1", "ann", "buy", 100, 5, 2, stp="decrement")
        self.assertEqual(report["cancelled"], ["i1"])
        self.assertEqual(report["trades"], [])
        self.assertEqual(status(eng, "i1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(status(eng, "b1"), ("open", None, 0, 2))
        self.assertEqual(eng.book(), {"bids": [[100, 2, 1]], "asks": []})

    def test_a_rejected_fill_or_kill_leaves_no_decrement_or_refill_behind(self):
        eng, _ = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 4, 2, 1)
        eng.submit("s2", "bob", "sell", 100, 1, 2)
        report = eng.submit("b1", "ann", "buy", 100, 3, 3, tif="FOK", stp="decrement")
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual((report["trades"], report["cancelled"]), ([], []))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 2})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])

    def test_an_incoming_iceberg_uses_its_own_stp_mode(self):
        eng, _ = engine()
        eng.submit("b1", "ann", "buy", 100, 2, 1)
        eng.submit("b2", "bob", "buy", 100, 2, 1)
        report = eng.submit_iceberg("i1", "ann", "sell", 100, 6, 1, 2, stp="cancel_oldest")
        self.assertEqual(report["cancelled"], ["b1"])
        self.assertEqual(pairs(report["trades"]), [("b2", "i1", 100, 2)])
        self.assertEqual(eng.iceberg("i1"), {"display": 1, "visible": 1, "hidden": 3})


class IcebergAmendCancelTests(unittest.TestCase):
    def test_amend_down_keeps_the_place_and_trims_the_reserve(self):
        eng, _ = shelf()
        report = eng.amend("i1", "ann", 3, quantity=3)
        self.assertEqual(report["order"]["remaining"], 3)
        self.assertEqual(report["order"]["quantity"], 3)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 1})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])

    def test_amend_below_the_visible_tranche_shrinks_it(self):
        eng, _ = shelf()
        eng.amend("i1", "ann", 3, quantity=1)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 0})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])
        self.assertEqual(eng.book()["asks"], [[100, 4, 2]])

    def test_amend_to_the_same_remaining_keeps_a_partly_taken_tranche(self):
        eng, _ = shelf()
        eng.submit("b1", "cat", "buy", 100, 1, 3)
        eng.amend("i1", "ann", 4, quantity=5)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 4})
        self.assertEqual(eng.queue("sell", 100), ["i1", "s2"])
        self.assertEqual(eng.order("i1")["quantity"], 6)

    def test_amend_up_loses_the_place_and_shows_a_fresh_tranche(self):
        eng, _ = shelf()
        eng.submit("b1", "cat", "buy", 100, 1, 3)
        eng.amend("i1", "ann", 4, quantity=6)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})
        self.assertEqual(eng.queue("sell", 100), ["s2", "i1"])
        self.assertEqual(status(eng, "i1"), ("open", None, 1, 6))

    def test_amend_to_a_crossing_price_trades_the_whole_remaining(self):
        eng, _ = shelf()
        eng.submit("b1", "bob", "buy", 99, 5, 3)
        report = eng.amend("i1", "ann", 4, price=99)
        self.assertEqual(pairs(report["trades"]), [("b1", "i1", 99, 5)])
        self.assertEqual(report["trades"][0]["taker"], "i1")
        self.assertEqual(status(eng, "i1"), ("open", None, 5, 1))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 1, "hidden": 0})
        self.assertEqual(eng.book()["asks"], [[99, 1, 1], [100, 3, 1]])

    def test_cancel_removes_the_whole_iceberg(self):
        eng, _ = shelf()
        eng.cancel("i1", "ann", 3)
        self.assertEqual(status(eng, "i1"), ("cancelled", "user", 0, 0))
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 0, "hidden": 0})
        self.assertEqual(eng.book()["asks"], [[100, 3, 1]])

    def test_ids_and_ownership_are_shared_with_plain_orders(self):
        eng, journal = shelf()
        with self.assertRaises(Conflict):
            eng.submit_iceberg("s2", "cat", "sell", 100, 4, 2, 3)
        with self.assertRaises(Conflict):
            eng.submit("i1", "cat", "sell", 100, 4, 3)
        with self.assertRaises(Forbidden):
            eng.amend("i1", "bob", 3, quantity=1)
        self.assertEqual(len(journal.rows), 2)
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})

    def test_bad_iceberg_arguments_are_invalid_and_change_nothing(self):
        eng, journal = shelf()
        good = {"order_id": "i9", "owner": "cat", "side": "sell", "price": 100, "quantity": 4, "display": 2, "now": 3}
        for change in ({"display": 0}, {"display": 5}, {"display": True}, {"display": 2.0}, {"price": None},
                       {"price": 0}, {"quantity": 0}, {"quantity": True}, {"side": "both"}, {"now": 1},
                       {"owner": ""}, {"stp": "none"}):
            with self.subTest(change=change), self.assertRaises(Invalid):
                eng.submit_iceberg(**{**good, **change})
        self.assertIsNone(eng.order("i9"))
        self.assertEqual(len(journal.rows), 2)
        eng.submit_iceberg(**good)
        self.assertEqual(eng.iceberg("i9"), {"display": 2, "visible": 2, "hidden": 2})


def iceberg_history(eng):
    eng.submit_iceberg("i1", "ann", "sell", 100, 9, 2, 1)
    eng.submit("s2", "bob", "sell", 100, 3, 1)
    eng.submit_iceberg("i3", "cat", "buy", 98, 7, 3, 2, stp="decrement")
    eng.submit("b1", "dan", "buy", 100, 5, 3)
    eng.amend("i1", "ann", 4, quantity=4)
    eng.submit("x1", "cat", "sell", 98, 4, 5, stp="decrement")
    eng.submit("x2", "dan", "sell", 98, 2, 6, tif="IOC")
    eng.amend("i3", "cat", 7, price=99)


class IcebergJournalTests(unittest.TestCase):
    def test_the_record_holds_every_parameter(self):
        eng, journal = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        eng.submit_iceberg("i2", "bob", "buy", 90, 5, 5, 2, stp="decrement")
        self.assertEqual(journal.rows, [
            {"sequence": 1, "operation": "submit_iceberg", "arguments": {
                "order_id": "i1", "owner": "ann", "side": "sell", "price": 100, "quantity": 6,
                "display": 2, "now": 1, "stp": "cancel_newest"}},
            {"sequence": 2, "operation": "submit_iceberg", "arguments": {
                "order_id": "i2", "owner": "bob", "side": "buy", "price": 90, "quantity": 5,
                "display": 5, "now": 2, "stp": "decrement"}}])

    def test_replay_restores_tranches_and_queue_places(self):
        eng, journal = engine()
        iceberg_history(eng)
        replayed = Engine(None, journal.rows)
        self.assertEqual(replayed.snapshot(), eng.snapshot())
        self.assertEqual(eng.snapshot()["sequence"], 8)
        for order_id in ("i1", "i3", "s2"):
            self.assertEqual(replayed.iceberg(order_id), eng.iceberg(order_id))
        self.assertEqual(replayed.book(), eng.book())
        self.assertEqual(replayed.queue("sell", 100), eng.queue("sell", 100))
        self.assertEqual(replayed.submit("z1", "eve", "buy", 100, 9, 9), eng.submit("z1", "eve", "buy", 100, 9, 9))

    def test_the_history_ends_in_the_stated_state(self):
        eng, _ = engine()
        iceberg_history(eng)
        self.assertEqual(sells(eng.trades()), [("i1", 2), ("s2", 3), ("x2", 2)])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 2})
        self.assertEqual(eng.iceberg("i3"), {"display": 3, "visible": 1, "hidden": 0})
        self.assertEqual(status(eng, "x1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(eng.book(), {"bids": [[99, 1, 1]], "asks": [[100, 2, 1]]})

    def test_a_failed_append_leaves_no_iceberg_behind(self):
        eng, journal = engine()
        journal.fail = OSError("disk full")
        with self.assertRaises(OSError):
            eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        self.assertIsNone(eng.order("i1"))
        self.assertIsNone(eng.iceberg("i1"))
        journal.fail = None
        eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        self.assertEqual([row["sequence"] for row in journal.rows], [1])
        self.assertEqual(eng.iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})

    def test_replay_rejects_a_malformed_iceberg_record(self):
        eng, journal = engine()
        eng.submit_iceberg("i1", "ann", "sell", 100, 6, 2, 1)
        good = journal.rows[0]
        self.assertEqual(Engine(None, [good]).iceberg("i1"), {"display": 2, "visible": 2, "hidden": 4})
        without_display = {name: value for name, value in good["arguments"].items() if name != "display"}
        for arguments in (without_display, {**good["arguments"], "tif": "GTC"}, {**good["arguments"], "display": 7},
                          {**good["arguments"], "display": 0}):
            with self.subTest(arguments=arguments), self.assertRaises(Invalid):
                Engine(None, [{**good, "arguments": arguments}])


class IcebergInvariantTests(unittest.TestCase):
    def run_history(self, seed):
        rng = random.Random(seed)
        eng, journal = engine()
        for step in range(1, 121):
            owner, side = rng.choice(["ann", "bob", "cat"]), rng.choice(["buy", "sell"])
            price, quantity = rng.randint(98, 102), rng.randint(1, 9)
            stp = rng.choice(["cancel_newest", "cancel_oldest", "decrement"])
            if rng.random() < 0.5:
                eng.submit_iceberg(f"o{step}", owner, side, price, quantity, rng.randint(1, quantity), step, stp=stp)
            else:
                tif = rng.choice(["GTC", "IOC", "FOK"])
                eng.submit(f"o{step}", owner, side, price, quantity, step, tif=tif, stp=stp)
        return eng, journal

    def test_tranches_stay_consistent_under_random_flow(self):
        for seed in (11, 12, 13):
            eng, _ = self.run_history(seed)
            shown = {"buy": {}, "sell": {}}
            for order in eng.snapshot()["orders"]:
                split = eng.iceberg(order["id"])
                if order["status"] != "open":
                    if split is not None:
                        self.assertEqual((split["visible"], split["hidden"]), (0, 0), (seed, order["id"]))
                    continue
                visible = order["remaining"] if split is None else split["visible"]
                if split is not None:
                    self.assertEqual(split["visible"] + split["hidden"], order["remaining"])
                    self.assertTrue(1 <= split["visible"] <= split["display"], (seed, order["id"], split))
                level = shown[order["side"]].setdefault(order["price"], [0, 0])
                level[0] += visible
                level[1] += 1
            book = eng.book()
            self.assertEqual({row[0]: row[1:] for row in book["bids"]}, shown["buy"])
            self.assertEqual({row[0]: row[1:] for row in book["asks"]}, shown["sell"])
            self.assertGreater(len(eng.trades()), 20)

    def test_random_flow_replays_to_the_same_engine(self):
        for seed in (21, 22):
            eng, journal = self.run_history(seed)
            replayed = Engine(None, journal.rows)
            self.assertEqual(replayed.snapshot(), eng.snapshot())
            self.assertEqual(replayed.book(), eng.book())
            self.assertEqual(len(journal.rows), 120)
            for order in eng.snapshot()["orders"]:
                self.assertEqual(replayed.iceberg(order["id"]), eng.iceberg(order["id"]))
