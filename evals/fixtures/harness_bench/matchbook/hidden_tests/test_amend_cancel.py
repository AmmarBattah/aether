import unittest

from matchbook import Closed, Conflict, Forbidden, Invalid, NotFound
from support import engine, pairs, status


def three_asks(eng):
    for order_id, owner in (("s1", "ann"), ("s2", "cat"), ("s3", "dan")):
        eng.submit(order_id, owner, "sell", 100, 4, 0)


class AmendTests(unittest.TestCase):
    def test_reducing_quantity_keeps_the_place(self):
        eng, _ = engine()
        three_asks(eng)
        report = eng.amend("s1", "ann", 1, quantity=2)
        self.assertEqual((report["order"]["remaining"], report["order"]["quantity"]), (2, 2))
        self.assertEqual(eng.queue("sell", 100), ["s1", "s2", "s3"])
        self.assertEqual(eng.book()["asks"], [[100, 10, 3]])

    def test_increasing_quantity_loses_the_place(self):
        eng, _ = engine()
        three_asks(eng)
        eng.amend("s1", "ann", 1, quantity=5)
        self.assertEqual(eng.queue("sell", 100), ["s2", "s3", "s1"])
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 5))

    def test_price_change_goes_to_the_back_of_the_new_level(self):
        eng, _ = engine()
        three_asks(eng)
        eng.submit("s4", "eve", "sell", 101, 1, 0)
        report = eng.amend("s2", "cat", 1, price=101)
        self.assertEqual((report["order"]["price"], report["trades"]), (101, []))
        self.assertEqual(eng.queue("sell", 100), ["s1", "s3"])
        self.assertEqual(eng.queue("sell", 101), ["s4", "s2"])

    def test_same_price_given_explicitly_with_less_quantity_keeps_the_place(self):
        eng, _ = engine()
        three_asks(eng)
        eng.amend("s1", "ann", 1, price=100, quantity=1)
        self.assertEqual(eng.queue("sell", 100), ["s1", "s2", "s3"])
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 1))

    def test_amend_that_changes_nothing_is_accepted_and_journaled(self):
        eng, journal = engine()
        three_asks(eng)
        report = eng.amend("s1", "ann", 7, quantity=4)
        self.assertEqual(report["order"], eng.order("s1"))
        self.assertEqual(eng.queue("sell", 100), ["s1", "s2", "s3"])
        self.assertEqual((len(journal.rows), eng.snapshot()["clock"]), (4, 7))

    def test_amend_to_a_crossing_price_trades_as_the_taker(self):
        eng, _ = engine()
        three_asks(eng)
        eng.submit("b1", "bob", "buy", 99, 6, 0)
        report = eng.amend("b1", "bob", 2, price=100)
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 4), ("b1", "s2", 100, 2)])
        self.assertEqual([t["taker"] for t in report["trades"]], ["b1", "b1"])
        self.assertEqual(status(eng, "b1"), ("filled", None, 6, 0))
        self.assertEqual(eng.queue("sell", 100), ["s2", "s3"])

    def test_quantity_is_filled_plus_new_remaining(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 10, 0)
        eng.submit("b1", "bob", "buy", 100, 4, 1)
        view = eng.amend("s1", "ann", 2, quantity=3)["order"]
        self.assertEqual((view["quantity"], view["filled"], view["remaining"]), (7, 4, 3))
        eng.submit("b2", "bob", "buy", 100, 3, 3)
        self.assertEqual(status(eng, "s1"), ("filled", None, 7, 0))

    def test_amended_remainder_rests_at_the_new_price_after_trading(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        eng.submit("b0", "cat", "buy", 100, 1, 0, post_only=False)
        eng.submit("b1", "bob", "buy", 98, 5, 0)
        self.assertEqual(status(eng, "s1"), ("open", None, 1, 1))
        report = eng.amend("b1", "bob", 1, price=101, quantity=3)
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 1)])
        self.assertEqual(status(eng, "b1"), ("open", None, 1, 2))
        self.assertEqual(eng.order("b1")["quantity"], 3)
        self.assertEqual(eng.book(), {"bids": [[101, 2, 1]], "asks": []})

    def test_amend_reentry_applies_self_trade_prevention(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 2, 0)
        eng.submit("b1", "bob", "buy", 99, 2, 0, stp="cancel_oldest")
        report = eng.amend("b1", "bob", 1, price=100)
        self.assertEqual((report["trades"], report["cancelled"]), ([], ["s1"]))
        self.assertEqual(status(eng, "b1"), ("open", None, 0, 2))
        self.assertEqual(status(eng, "s1"), ("cancelled", "stp", 0, 0))

    def test_amend_errors_follow_the_documented_order(self):
        eng, journal = engine()
        three_asks(eng)
        eng.submit("b1", "bob", "buy", 100, 4, 1)
        with self.assertRaises(NotFound):
            eng.amend("nope", "ann", 2, quantity=1)
        with self.assertRaises(Forbidden):
            eng.amend("s2", "ann", 2, quantity=1)
        with self.assertRaises(Closed):
            eng.amend("s1", "ann", 2, quantity=1)
        with self.assertRaises(Forbidden):
            eng.amend("s1", "cat", 2, quantity=1)
        with self.assertRaises(Invalid):
            eng.amend("nope", "ann", 2)
        self.assertEqual((len(journal.rows), eng.snapshot()["clock"]), (4, 1))
        self.assertEqual(eng.queue("sell", 100), ["s2", "s3"])

    def test_amend_arguments_are_validated(self):
        eng, journal = engine()
        three_asks(eng)
        for kwargs in ({}, {"quantity": 0}, {"quantity": True}, {"price": 0}, {"price": 1.5}, {"quantity": "2"}):
            with self.assertRaises(Invalid, msg=kwargs):
                eng.amend("s1", "ann", 1, **kwargs)
        self.assertEqual(len(journal.rows), 3)
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 4))


class CancelTests(unittest.TestCase):
    def test_cancel_removes_the_order_from_the_book(self):
        eng, _ = engine()
        three_asks(eng)
        report = eng.cancel("s2", "cat", 1)
        self.assertEqual(status(eng, "s2"), ("cancelled", "user", 0, 0))
        self.assertEqual(report["order"], eng.order("s2"))
        self.assertEqual(eng.queue("sell", 100), ["s1", "s3"])
        with self.assertRaises(Closed):
            eng.cancel("s2", "cat", 1)

    def test_cancel_keeps_the_filled_quantity(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 0)
        eng.submit("b1", "bob", "buy", 100, 2, 0)
        eng.cancel("s1", "ann", 1)
        view = eng.order("s1")
        self.assertEqual((view["quantity"], view["filled"], view["remaining"], view["reason"]), (5, 2, 0, "user"))

    def test_cancel_by_another_owner_is_forbidden_and_changes_nothing(self):
        eng, journal = engine()
        three_asks(eng)
        with self.assertRaises(Forbidden):
            eng.cancel("s1", "cat", 5)
        with self.assertRaises(NotFound):
            eng.cancel("zzz", "cat", 5)
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 4))
        self.assertEqual((len(journal.rows), eng.snapshot()["clock"]), (3, 0))

    def test_cancel_expires_other_orders_due_at_that_time(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0, tif="GTD", expires=3)
        eng.submit("s2", "cat", "sell", 100, 1, 0)
        report = eng.cancel("s2", "cat", 3)
        self.assertEqual(report["expired"], ["s1"])
        self.assertEqual(eng.book(), {"bids": [], "asks": []})


class ValidationTests(unittest.TestCase):
    def test_bad_submit_arguments_are_invalid_and_not_journaled(self):
        eng, journal = engine()
        eng.submit("ok", "ann", "sell", 100, 1, 2)
        good = {"order_id": "b1", "owner": "bob", "side": "buy", "price": 100, "quantity": 1, "now": 2}
        bad = ({"quantity": 0}, {"quantity": -1}, {"quantity": True}, {"quantity": 1.0}, {"price": 0},
               {"price": True}, {"price": "100"}, {"side": "BUY"}, {"side": "hold"}, {"order_id": ""},
               {"order_id": 7}, {"owner": ""}, {"owner": None}, {"tif": "DAY"}, {"stp": "none"},
               {"now": -1}, {"now": 1}, {"now": True}, {"now": 2.0})
        for change in bad:
            with self.assertRaises(Invalid, msg=change):
                eng.submit(**{**good, **change})
        self.assertEqual(len(journal.rows), 1)
        self.assertEqual(pairs(eng.submit(**good)["trades"]), [("b1", "ok", 100, 1)])

    def test_time_may_repeat_but_not_go_backwards(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 5)
        eng.submit("s2", "ann", "sell", 100, 1, 5)
        for call in (lambda: eng.advance(4), lambda: eng.cancel("s1", "ann", 4),
                     lambda: eng.amend("s1", "ann", 4, quantity=1), lambda: eng.submit("b", "bob", "buy", 1, 1, 4)):
            with self.assertRaises(Invalid):
                call()
        self.assertEqual(eng.snapshot()["clock"], 5)
        self.assertEqual(eng.advance(5)["expired"], [])

    def test_order_ids_are_never_reusable(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("b1", "bob", "buy", 100, 1, 0)
        eng.submit("c1", "ann", "sell", 100, 1, 0)
        eng.cancel("c1", "ann", 0)
        for order_id in ("s1", "b1", "c1"):
            with self.assertRaises(Conflict, msg=order_id):
                eng.submit(order_id, "eve", "buy", 50, 1, 1)
        self.assertEqual(eng.snapshot()["clock"], 0)
        self.assertEqual(len(eng.snapshot()["orders"]), 3)

    def test_invalid_arguments_win_over_a_duplicate_id(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        with self.assertRaises(Invalid):
            eng.submit("s1", "ann", "sell", 100, 0, 0)
        with self.assertRaises(Conflict):
            eng.submit("s1", "ann", "sell", 100, 1, 0)
        self.assertEqual(eng.book()["asks"], [[100, 1, 1]])


if __name__ == "__main__":
    unittest.main()
