import unittest

from matchbook import Closed, Engine


class SmokeTests(unittest.TestCase):
    def test_resting_order_shows_in_the_book(self):
        engine = Engine()
        report = engine.submit("s1", "ann", "sell", 100, 5, 1)
        self.assertEqual(report["order"]["status"], "open")
        self.assertEqual(engine.book(), {"bids": [], "asks": [[100, 5, 1]]})

    def test_equal_prices_trade(self):
        engine = Engine()
        engine.submit("s1", "ann", "sell", 100, 5, 1)
        report = engine.submit("b1", "bob", "buy", 100, 2, 2)
        self.assertEqual([(t["buy"], t["sell"], t["price"], t["quantity"]) for t in report["trades"]],
                         [("b1", "s1", 100, 2)])
        self.assertEqual(engine.order("s1")["remaining"], 3)

    def test_cancel_removes_the_order(self):
        engine = Engine()
        engine.submit("s1", "ann", "sell", 100, 5, 1)
        engine.cancel("s1", "ann", 2)
        self.assertEqual(engine.book()["asks"], [])
        with self.assertRaises(Closed):
            engine.cancel("s1", "ann", 3)

    def test_journal_replays_to_the_same_book(self):
        records = []
        engine = Engine(records.append)
        engine.submit("s1", "ann", "sell", 100, 5, 1)
        engine.submit("b1", "bob", "buy", 99, 2, 2)
        self.assertEqual([record["sequence"] for record in records], [1, 2])
        self.assertEqual(Engine(None, records).book(), engine.book())


if __name__ == "__main__":
    unittest.main()
