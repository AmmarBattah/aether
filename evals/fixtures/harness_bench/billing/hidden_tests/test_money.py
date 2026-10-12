import random
import unittest
from decimal import Decimal

from invoicing import CURRENCIES, InvalidInput, allocate, quantize
from support import MoneyCase


class QuantizeTests(MoneyCase):
    def test_currency_table_is_exact(self):
        self.assertEqual(CURRENCIES, {"USD": 2, "EUR": 2, "JPY": 0, "BHD": 3})

    def test_invalid_input_is_a_value_error(self):
        self.assertTrue(issubclass(InvalidInput, ValueError))

    def test_half_even_rounds_down_to_even(self):
        self.assertMoney(quantize(Decimal("2.345"), "USD"), "2.34")

    def test_half_even_rounds_up_to_even(self):
        self.assertMoney(quantize(Decimal("2.355"), "USD"), "2.36")

    def test_negative_half_even(self):
        self.assertMoney(quantize(Decimal("-0.125"), "EUR"), "-0.12")

    def test_result_carries_currency_exponent(self):
        self.assertMoney(quantize(5, "USD"), "5.00")
        self.assertMoney(quantize(Decimal("0"), "BHD"), "0.000")
        self.assertMoney(quantize(Decimal("7.10000"), "EUR"), "7.10")

    def test_zero_exponent_currency(self):
        self.assertMoney(quantize(Decimal("2.5"), "JPY"), "2")
        self.assertMoney(quantize(Decimal("3.5"), "JPY"), "4")

    def test_rejects_float_bool_and_string(self):
        for value in (1.5, True, "1.00", None):
            with self.subTest(value=value), self.assertRaises(InvalidInput):
                quantize(value, "USD")

    def test_rejects_non_finite(self):
        for value in (Decimal("NaN"), Decimal("Infinity"), Decimal("-Infinity")):
            with self.subTest(value=value), self.assertRaises(InvalidInput):
                quantize(value, "USD")

    def test_rejects_unknown_currency(self):
        for currency in ("usd", "GBP", "", None):
            with self.subTest(currency=currency), self.assertRaises(InvalidInput):
                quantize(Decimal("1"), currency)


class AllocateTests(MoneyCase):
    def test_even_split_gives_leftover_to_first(self):
        self.assertMoneyList(allocate(Decimal("10.00"), [1, 1, 1], "USD"), ["3.34", "3.33", "3.33"])

    def test_largest_remainder_wins_not_lowest_index(self):
        self.assertMoneyList(allocate(Decimal("0.10"), [1, 2, 4], "USD"), ["0.01", "0.03", "0.06"])

    def test_equal_remainders_prefer_lower_index(self):
        self.assertMoneyList(allocate(Decimal("0.05"), [2, 1, 1, 2], "USD"), ["0.02", "0.01", "0.01", "0.01"])

    def test_zero_weight_receives_zero(self):
        self.assertMoneyList(allocate(Decimal("0.03"), [0, 1, 1], "USD"), ["0.00", "0.02", "0.01"])

    def test_negative_total_negates_positive_allocation(self):
        self.assertMoneyList(allocate(Decimal("-10.00"), [1, 1, 1], "USD"), ["-3.34", "-3.33", "-3.33"])
        self.assertMoneyList(allocate(Decimal("-0.10"), [1, 2, 4], "USD"), ["-0.01", "-0.03", "-0.06"])

    def test_other_exponents(self):
        self.assertMoneyList(allocate(Decimal("100"), [1, 2], "JPY"), ["33", "67"])
        self.assertMoneyList(allocate(Decimal("1.000"), [1, 1, 1], "BHD"), ["0.334", "0.333", "0.333"])

    def test_zero_total(self):
        self.assertMoneyList(allocate(Decimal("0"), [1, 2], "USD"), ["0.00", "0.00"])

    def test_total_precision(self):
        self.assertMoneyList(allocate(Decimal("1.5"), [1, 1], "USD"), ["0.75", "0.75"])
        self.assertMoneyList(allocate(3, [1, 1], "USD"), ["1.50", "1.50"])
        with self.assertRaises(InvalidInput):
            allocate(Decimal("1.005"), [1], "USD")
        with self.assertRaises(InvalidInput):
            allocate(Decimal("1.5"), [1], "JPY")

    def test_rejects_invalid_weights(self):
        for weights in ([], [0, 0], [1, -1], [True, 1], [1.0, 1]):
            with self.subTest(weights=weights), self.assertRaises(InvalidInput):
                allocate(Decimal("1.00"), weights, "USD")

    def test_shares_sum_exactly_and_stay_within_one_unit(self):
        rng = random.Random(4242)
        for _ in range(300):
            weights = [rng.choice([0, 1, 2, 3, 7, 11, 100]) for _ in range(rng.randint(1, 6))]
            if not sum(weights):
                weights[0] = 1
            minor = rng.randint(-5000, 5000)
            total = Decimal(minor).scaleb(-2)
            shares = allocate(total, weights, "USD")
            self.assertEqual(sum(shares), total)
            for share, weight in zip(shares, weights):
                exact = Decimal(abs(minor) * weight) / sum(weights)
                self.assertLess(abs(abs(share) * 100 - exact), 1)
                if weight == 0:
                    self.assertEqual(share, 0)


if __name__ == "__main__":
    unittest.main()
