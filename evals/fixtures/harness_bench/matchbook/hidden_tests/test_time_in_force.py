import unittest

from matchbook import Closed, Conflict, Invalid
from support import engine, pairs, status


class TimeInForceTests(unittest.TestCase):
    def test_ioc_remainder_is_cancelled_not_rested(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        report = eng.submit("b1", "bob", "buy", 100, 5, 1, tif="IOC")
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 2)])
        self.assertEqual(status(eng, "b1"), ("cancelled", "ioc", 2, 0))
        self.assertEqual(eng.book(), {"bids": [], "asks": []})

    def test_ioc_without_a_match_is_cancelled_and_uses_its_id(self):
        eng, journal = engine()
        report = eng.submit("b1", "bob", "buy", 100, 5, 1, tif="IOC")
        self.assertEqual((report["order"]["status"], report["order"]["reason"]), ("cancelled", "ioc"))
        self.assertEqual(len(journal.rows), 1)
        with self.assertRaises(Conflict):
            eng.submit("b1", "bob", "buy", 100, 5, 2)

    def test_fully_filled_ioc_is_filled(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 5, 0)
        eng.submit("b1", "bob", "buy", 100, 5, 1, tif="IOC")
        self.assertEqual(status(eng, "b1"), ("filled", None, 5, 0))

    def test_market_order_sweeps_any_price_and_cancels_the_rest(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        eng.submit("s2", "ann", "sell", 5000, 1, 0)
        report = eng.submit("b1", "bob", "buy", None, 3, 1, tif="IOC")
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 1), ("b1", "s2", 5000, 1)])
        self.assertIsNone(report["order"]["price"])
        self.assertEqual(status(eng, "b1"), ("cancelled", "ioc", 2, 0))

    def test_market_order_must_be_ioc_or_fok(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        for tif, expires in (("GTC", None), ("GTD", 9)):
            with self.assertRaises(Invalid):
                eng.submit("b1", "bob", "buy", None, 1, 1, tif=tif, expires=expires)
        self.assertEqual(len(journal.rows), 1)
        self.assertEqual(eng.submit("b1", "bob", "sell", None, 1, 1, tif="FOK")["order"]["status"], "rejected")

    def test_fok_fills_across_levels_when_fully_available(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        eng.submit("s2", "cat", "sell", 101, 3, 0)
        report = eng.submit("b1", "bob", "buy", 101, 5, 1, tif="FOK")
        self.assertEqual(pairs(report["trades"]), [("b1", "s1", 100, 2), ("b1", "s2", 101, 3)])
        self.assertEqual(status(eng, "b1"), ("filled", None, 5, 0))

    def test_fok_without_enough_quantity_changes_nothing(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        eng.submit("s2", "cat", "sell", 101, 2, 0)
        before = eng.book()
        report = eng.submit("b1", "bob", "buy", 101, 5, 1, tif="FOK")
        self.assertEqual(report["trades"], [])
        self.assertEqual(status(eng, "b1"), ("rejected", "fok", 0, 0))
        self.assertEqual(eng.book(), before)
        self.assertEqual((eng.trades(), len(journal.rows), eng.snapshot()["clock"]), ([], 3, 1))

    def test_fok_counts_only_quantity_within_its_limit(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0)
        eng.submit("s2", "cat", "sell", 102, 9, 0)
        self.assertEqual(eng.submit("b1", "bob", "buy", 101, 3, 1, tif="FOK")["order"]["reason"], "fok")
        self.assertEqual(eng.submit("b2", "bob", "buy", None, 3, 1, tif="FOK")["order"]["status"], "filled")
        self.assertEqual(pairs(eng.trades()), [("b2", "s1", 100, 2), ("b2", "s2", 102, 1)])

    def test_gtd_expiry_arguments_are_validated(self):
        eng, journal = engine()
        eng.advance(5)
        for kwargs in ({"tif": "GTD"}, {"tif": "GTD", "expires": 5}, {"tif": "GTD", "expires": True},
                       {"tif": "GTC", "expires": 9}, {"tif": "IOC", "expires": 9}):
            with self.assertRaises(Invalid, msg=kwargs):
                eng.submit("b1", "bob", "buy", 100, 1, 5, **kwargs)
        view = eng.submit("b1", "bob", "buy", 100, 1, 5, tif="GTD", expires=6)["order"]
        self.assertEqual((view["tif"], view["expires"], view["status"]), ("GTD", 6, "open"))
        self.assertEqual(len(journal.rows), 2)

    def test_order_expires_at_equality_and_never_trades(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0, tif="GTD", expires=4)
        report = eng.submit("b1", "bob", "buy", 100, 2, 4)
        self.assertEqual((report["trades"], report["expired"]), ([], ["s1"]))
        self.assertEqual(status(eng, "s1"), ("expired", None, 0, 0))
        self.assertEqual(eng.book(), {"bids": [[100, 2, 1]], "asks": []})

    def test_order_still_trades_one_tick_before_expiry(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0, tif="GTD", expires=4)
        report = eng.submit("b1", "bob", "buy", 100, 1, 3)
        self.assertEqual((pairs(report["trades"]), report["expired"]), ([("b1", "s1", 100, 1)], []))
        self.assertEqual(eng.advance(4)["expired"], ["s1"])
        self.assertEqual(status(eng, "s1"), ("expired", None, 1, 0))

    def test_advance_reports_expiries_in_submission_order(self):
        eng, _ = engine()
        eng.submit("late", "ann", "sell", 105, 1, 0, tif="GTD", expires=9)
        eng.submit("zed", "ann", "buy", 95, 1, 0, tif="GTD", expires=7)
        eng.submit("abe", "ann", "sell", 104, 1, 0, tif="GTD", expires=3)
        eng.submit("keep", "ann", "sell", 106, 1, 0)
        report = eng.advance(8)
        self.assertEqual(report, {"order": None, "trades": [], "cancelled": [], "expired": ["zed", "abe"]})
        self.assertEqual(eng.book(), {"bids": [], "asks": [[105, 1, 1], [106, 1, 1]]})
        self.assertEqual(eng.advance(8)["expired"], [])

    def test_cancel_at_expiry_time_is_closed_and_changes_nothing(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0, tif="GTD", expires=4)
        with self.assertRaises(Closed):
            eng.cancel("s1", "ann", 4)
        with self.assertRaises(Closed):
            eng.amend("s1", "ann", 6, quantity=1)
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 2))
        self.assertEqual((eng.snapshot()["clock"], len(journal.rows)), (0, 1))
        self.assertEqual(eng.cancel("s1", "ann", 3)["order"]["reason"], "user")

    def test_expired_order_is_closed_for_cancel(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 2, 0, tif="GTD", expires=4)
        eng.advance(4)
        with self.assertRaises(Closed):
            eng.cancel("s1", "ann", 4)
        self.assertEqual(status(eng, "s1"), ("expired", None, 0, 0))


class PostOnlyTests(unittest.TestCase):
    def test_post_only_that_does_not_cross_rests(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 101, 1, 0)
        report = eng.submit("b1", "bob", "buy", 100, 1, 1, post_only=True)
        self.assertEqual(report["order"]["status"], "open")
        self.assertEqual(eng.book(), {"bids": [[100, 1, 1]], "asks": [[101, 1, 1]]})

    def test_post_only_that_would_cross_is_rejected_and_journaled(self):
        eng, journal = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0)
        report = eng.submit("b1", "bob", "buy", 100, 1, 3, post_only=True)
        self.assertEqual(report["trades"], [])
        self.assertEqual(status(eng, "b1"), ("rejected", "post_only", 0, 0))
        self.assertEqual(eng.book(), {"bids": [], "asks": [[100, 1, 1]]})
        self.assertEqual((len(journal.rows), eng.snapshot()["clock"]), (2, 3))
        with self.assertRaises(Conflict):
            eng.submit("b1", "bob", "buy", 99, 1, 3)

    def test_post_only_is_rejected_even_against_the_same_owner(self):
        eng, _ = engine()
        eng.submit("s1", "bob", "sell", 100, 1, 0)
        report = eng.submit("b1", "bob", "buy", 101, 1, 1, post_only=True, stp="cancel_oldest")
        self.assertEqual((report["order"]["reason"], report["cancelled"]), ("post_only", []))
        self.assertEqual(status(eng, "s1"), ("open", None, 0, 1))

    def test_post_only_ignores_an_order_expiring_now(self):
        eng, _ = engine()
        eng.submit("s1", "ann", "sell", 100, 1, 0, tif="GTD", expires=2)
        report = eng.submit("b1", "bob", "buy", 100, 1, 2, post_only=True)
        self.assertEqual((report["order"]["status"], report["expired"]), ("open", ["s1"]))

    def test_post_only_needs_a_resting_limit_order(self):
        eng, journal = engine()
        for kwargs in ({"tif": "IOC"}, {"tif": "FOK"}):
            with self.assertRaises(Invalid, msg=kwargs):
                eng.submit("b1", "bob", "buy", 100, 1, 0, post_only=True, **kwargs)
        with self.assertRaises(Invalid):
            eng.submit("b1", "bob", "buy", 100, 1, 0, post_only=1)
        self.assertEqual(journal.rows, [])
        view = eng.submit("b1", "bob", "buy", 100, 1, 0, tif="GTD", expires=3, post_only=True)["order"]
        self.assertEqual(view["status"], "open")


if __name__ == "__main__":
    unittest.main()
