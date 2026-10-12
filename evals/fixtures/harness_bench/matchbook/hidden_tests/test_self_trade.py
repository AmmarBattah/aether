import unittest

from support import engine, pairs, status


class SelfTradeTests(unittest.TestCase):
    def test_cancel_newest_stops_matching_and_keeps_earlier_trades(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("s2", "bob", "sell", 101, 1, 0)
        eng.submit("s3", "ann", "sell", 102, 1, 0)
        report = eng.submit("b1", "bob", "buy", 102, 3, 1)
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 1)])
        self.assertEqual(report["cancelled"], [])
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 1, 0))
        self.assertEqual(eng.book(), {"bids": [], "asks": [[101, 1, 1], [102, 1, 1]]})

    def test_cancel_newest_is_the_default(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 2, 1)
        self.assertEqual(report["trades"], [])
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 2))

    def test_cancel_oldest_removes_own_order_and_continues(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 1, 0)
        eng.submit("s2", "ann", "sell", 101, 1, 0)
        report = eng.submit("b1", "bob", "buy", 101, 3, 1, stp="cancel_oldest")
        self.assertEqual(report["cancelled"], ["s1"])
        self.assertEqual(pairs(report["trades"]), [("b1", "s2", 101, 1)])
        self.assertEqual(status(eng, "s1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(status(eng, "b1"), ("open", None, 1, 2))
        self.assertEqual(eng.book(), {"bids": [[101, 2, 1]], "asks": []})

    def test_decrement_equal_quantities_cancels_both_without_a_trade(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 3, 0)
        report = eng.submit("b1", "bob", "buy", 100, 3, 1, stp="decrement")
        self.assertEqual((report["trades"], report["cancelled"]), ([], ["s1"]))
        self.assertEqual(status(eng, "s1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 0, 0))

    def test_decrement_larger_resting_order_keeps_its_place(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 5, 0)
        eng.submit("s2", "ann", "sell", 100, 1, 0)
        report = eng.submit("b1", "bob", "buy", 100, 2, 1, stp="decrement")
        self.assertEqual((report["trades"], report["cancelled"]), ([], []))
        self.assertEqual(status(eng, "b1"), ("cancelled", "stp", 0, 0))
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 3))
        self.assertEqual(eng.queue("sell", 100), ["s1", "s2"])
        self.assertEqual(eng.book()["asks"], [[100, 4, 2]])

    def test_decrement_larger_incoming_order_keeps_matching(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 2, 0)
        eng.submit("s2", "ann", "sell", 101, 1, 0)
        report = eng.submit("b1", "bob", "buy", 101, 5, 1, stp="decrement")
        self.assertEqual(report["cancelled"], ["s1"])
        self.assertEqual(pairs(report["trades"]), [("b1", "s2", 101, 1)])
        self.assertEqual(status(eng, "b1"), ("open", None, 1, 2))
        self.assertEqual(eng.order("b1")["quantity"], 5)

    def test_own_order_behind_the_limit_is_not_touched(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("s2", "bob", "sell", 105, 1, 0)
        report = eng.submit("b1", "bob", "buy", 102, 2, 1, stp="cancel_oldest")
        self.assertEqual((pairs(report["trades"]), report["cancelled"]), ([("b1", "s1", 100, 1)], []))
        self.assertEqual(status(eng, "s2"), ("open", None, 0, 1))
        self.assertEqual(status(eng, "b1"), ("open", None, 1, 1))

    def test_own_order_never_reached_is_not_touched(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        eng.submit("s2", "bob", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 2, 1, stp="cancel_oldest")
        self.assertEqual((pairs(report["trades"]), report["cancelled"]), ([("b1", "s1", 100, 2)], []))
        self.assertEqual(status(eng, "s2"), ("open", None, 0, 2))

    def test_fok_cancel_newest_is_unfillable_through_an_own_order(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("s2", "bob", "sell", 100, 1, 0)
        eng.submit("s3", "ann", "sell", 100, 5, 0)
        report = eng.submit("b1", "bob", "buy", 100, 3, 1, tif="FOK")
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual((report["trades"], eng.trades()), ([], []))
        self.assertEqual(eng.book()["asks"], [[100, 7, 3]])
        self.assertEqual(eng.submit("b2", "bob", "buy", 100, 1, 1, tif="FOK")["order"]["status"], "filled")

    def test_fok_cancel_oldest_skips_own_orders_and_cancels_them(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 4, 0)
        eng.submit("s2", "ann", "sell", 101, 3, 0)
        report = eng.submit("b1", "bob", "buy", 101, 3, 1, tif="FOK", stp="cancel_oldest")
        self.assertEqual((pairs(report["trades"]), report["cancelled"]), ([("b1", "s2", 101, 3)], ["s1"]))
        self.assertEqual(status(eng, "b1"), ("filled", None, 3, 0))
        self.assertEqual(status(eng, "s1"), ("cancelled", "stp", 0, 0))

    def test_unfillable_fok_cancel_oldest_cancels_nothing(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 4, 0)
        eng.submit("s2", "ann", "sell", 101, 2, 0)
        report = eng.submit("b1", "bob", "buy", 101, 3, 1, tif="FOK", stp="cancel_oldest")
        self.assertEqual((report["trades"], report["cancelled"]), ([], []))
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 4))
        self.assertEqual(eng.book()["asks"], [[100, 4, 1], [101, 2, 1]])

    def test_fok_decrement_does_not_count_decremented_quantity(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 1, 0)
        eng.submit("s2", "ann", "sell", 100, 9, 0)
        report = eng.submit("b1", "bob", "buy", 100, 3, 1, tif="FOK", stp="decrement")
        self.assertEqual((report["trades"], report["cancelled"]), ([], []))
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 1))
        self.assertEqual(eng.book()["asks"], [[100, 10, 2]])


if __name__ == "__main__":
    unittest.main()
