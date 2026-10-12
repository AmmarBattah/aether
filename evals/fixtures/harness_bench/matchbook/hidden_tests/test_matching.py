import unittest

from support import engine, pairs, status


class MatchingTests(unittest.TestCase):
    def test_resting_order_view_has_every_field(self):
        eng, _ = engine()
        report = eng.submit("b1", "ann", "buy", 100, 7, 3)
        expected = {"id": "b1", "owner": "ann", "side": "buy", "price": 100, "quantity": 7, "filled": 0,
                    "remaining": 7, "tif": "GTC", "expires": None, "status": "open", "reason": None}
        self.assertEqual(report, {"order": expected, "trades": [], "cancelled": [], "expired": []})
        self.assertEqual(eng.order("b1"), expected)
        self.assertIsNone(eng.order("missing"))

    def test_trade_executes_at_the_resting_price(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 0)
        report = eng.submit("b1", "bob", "buy", 105, 5, 1)
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 5)])
        eng.submit("b2", "bob", "buy", 90, 4, 2)
        report = eng.submit("s2", "ann", "sell", 80, 4, 3)
        self.assertEqual(pairs(report["trades"]), [("b2", "s2", 90, 4)])

    def test_trade_record_fields_are_exact(self):
        eng, _ = engine()
        eng.submit("b1", "ann", "buy", 50, 5, 2)
        report = eng.submit("s1", "bob", "sell", 50, 2, 9)
        trade = {"id": 1, "time": 9, "price": 50, "quantity": 2, "buy": "b1", "sell": "s1", "taker": "s1"}
        self.assertEqual(report["trades"], [trade])
        self.assertEqual(eng.trades(), [trade])

    def test_best_price_is_taken_first(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 103, 1, 0)
        eng.submit("s2", "ann", "sell", 101, 1, 0)
        eng.submit("s3", "ann", "sell", 102, 1, 0)
        report = eng.submit("b1", "bob", "buy", 103, 3, 1)
        self.assertEqual(pairs(report["trades"]), [("b1", "s2", 101, 1), ("b1", "s3", 102, 1), ("b1", "s1", 103, 1)])

    def test_highest_bid_is_taken_first_by_a_sell(self):
        eng, _ = engine()
        for order_id, price in (("b1", 97), ("b2", 99), ("b3", 98)):
            eng.submit(order_id, "ann", "buy", price, 1, 0)
        report = eng.submit("s1", "bob", "sell", 98, 5, 1)
        self.assertEqual(pairs(report["trades"]), [("b2", "s1", 99, 1), ("b3", "s1", 98, 1)])
        self.assertEqual(status(eng, "s1"), ("open", None, 2, 3))
        self.assertEqual(eng.book(), {"bids": [[97, 1, 1]], "asks": [[98, 3, 1]]})

    def test_time_priority_within_a_level(self):
        eng, _ = engine()
        for order_id in ("s1", "s2", "s3"):
            eng.submit(order_id, "ann", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 5, 1)
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 2), ("b1", "s2", 100, 2), ("b1", "s3", 100, 1)])
        self.assertEqual(eng.queue("sell", 100), ["s3"])

    def test_partial_fill_leaves_the_remainder_resting(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 10, 0)
        eng.submit("b1", "bob", "buy", 100, 4, 1)
        self.assertEqual(status(eng, "s1"), ("open", None, 4, 6))
        self.assertEqual(status(eng, "b1"), ("filled", None, 4, 0))
        self.assertEqual(eng.order("s1")["quantity"], 10)

    def test_incoming_remainder_rests_after_sweeping(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 101, 5, 1)
        self.assertEqual(report["order"]["status"], "open")
        self.assertEqual((report["order"]["filled"], report["order"]["remaining"]), (2, 3))
        self.assertEqual(eng.book(), {"bids": [[101, 3, 1]], "asks": []})

    def test_orders_that_do_not_cross_do_not_trade(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 101, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 2, 1)
        self.assertEqual(report["trades"], [])
        self.assertEqual(eng.book(), {"bids": [[100, 2, 1]], "asks": [[101, 2, 1]]})

    def test_trade_ids_are_contiguous_across_operations(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("s2", "ann", "sell", 100, 1, 0)
        eng.submit("b1", "bob", "buy", 100, 1, 1)
        eng.submit("b2", "bob", "buy", 100, 1, 2)
        self.assertEqual([(t["id"], t["time"]) for t in eng.trades()], [(1, 1), (2, 2)])

    def test_book_aggregates_levels_best_first(self):
        eng, _ = engine()
        for order_id, side, price, quantity in (("b1", "buy", 98, 1), ("b2", "buy", 99, 2), ("b3", "buy", 99, 3),
                                                ("s1", "sell", 102, 4), ("s2", "sell", 101, 5), ("s3", "sell", 102, 6)):
            eng.submit(order_id, "ann", side, price, quantity, 0)
        self.assertEqual(eng.book(), {"bids": [[99, 5, 2], [98, 1, 1]], "asks": [[101, 5, 1], [102, 10, 2]]})
        self.assertEqual(eng.queue("buy", 99), ["b2", "b3"])
        self.assertEqual(eng.queue("sell", 100), [])

    def test_snapshot_lists_orders_in_submission_order(self):
        eng, _ = engine()
        eng.submit("z", "ann", "sell", 100, 1, 1)
        eng.submit("a", "bob", "buy", 100, 1, 4)
        eng.advance(6)
        snap = eng.snapshot()
        self.assertEqual((snap["clock"], snap["sequence"]), (6, 3))
        self.assertEqual([o["id"] for o in snap["orders"]], ["z", "a"])
        self.assertEqual([o["status"] for o in snap["orders"]], ["filled", "filled"])
        self.assertEqual(snap["trades"], eng.trades())

    def test_returned_values_are_defensive_copies(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 1, 1)
        report["trades"][0]["price"] = 1
        report["order"]["status"] = "hacked"
        eng.order("s1")["remaining"] = 99
        eng.book()["asks"][0][1] = 99
        eng.trades().clear()
        eng.snapshot()["orders"].clear()
        self.assertEqual(eng.trades()[0]["price"], 100)
        self.assertEqual(status(eng, "b1"), ("filled", None, 1, 0))
        self.assertEqual(eng.book(), {"bids": [], "asks": [[100, 1, 1]]})
        self.assertEqual(len(eng.snapshot()["orders"]), 2)


if __name__ == "__main__":
    unittest.main()
