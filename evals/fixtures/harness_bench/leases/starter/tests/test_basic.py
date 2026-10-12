import unittest
from leasepool import Pool


class BasicTests(unittest.TestCase):
    def test_disjoint_grants(self):
        records = []
        pool = Pool(records.append)
        self.assertEqual(pool.request("a", ["disk"], 0, 5, 0)["state"], "active")
        self.assertEqual(pool.request("b", ["network"], 0, 5, 0)["state"], "active")
        self.assertEqual(len(records), 2)

    def test_release_grants_waiter(self):
        pool = Pool(lambda record: None)
        first = pool.request("a", ["disk"], 0, 5, 0)
        self.assertEqual(pool.request("b", ["disk"], 0, 5, 0)["state"], "queued")
        pool.release("a", first["fences"], 1)
        self.assertEqual(pool.get("b")["state"], "active")
        self.assertEqual(pool.get("b")["fences"], {"disk": 2})


if __name__ == "__main__":
    unittest.main()
