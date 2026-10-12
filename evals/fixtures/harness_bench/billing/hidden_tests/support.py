import unittest
from decimal import Decimal


class MoneyCase(unittest.TestCase):
    def assertMoney(self, value, text):
        self.assertIsInstance(value, Decimal)
        self.assertEqual(str(value), text)

    def assertMoneyList(self, values, texts):
        self.assertEqual(len(values), len(texts))
        for value in values:
            self.assertIsInstance(value, Decimal)
        self.assertEqual([str(value) for value in values], texts)
