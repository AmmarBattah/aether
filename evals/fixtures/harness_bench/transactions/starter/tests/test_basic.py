import unittest
from txstore import Store


class SmokeTests(unittest.TestCase):
    def test_single_commit(self):
        store = Store()
        tx = store.begin("first")
        tx.put("a", 1)
        self.assertEqual(tx.commit()["revision"], 1)
        self.assertEqual(store.snapshot(), {"a": 1})

    def test_abort(self):
        store = Store()
        tx = store.begin("first")
        tx.put("a", 1)
        tx.abort()
        self.assertEqual(store.snapshot(), {})
