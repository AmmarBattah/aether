import unittest
from datetime import date, datetime
from decimal import Decimal

from invoicing import InvalidInput, next_anchor, price_tiers, prorate, quantize
from support import MoneyCase

TIERS = [(100, Decimal("1.00")), (1000, Decimal("0.50")), (None, Decimal("0.10"))]


class GraduatedTests(MoneyCase):
    def test_within_first_tier(self):
        self.assertEqual(price_tiers(50, TIERS, "graduated"), Decimal("50"))

    def test_boundary_belongs_to_lower_tier(self):
        self.assertEqual(price_tiers(100, TIERS, "graduated"), Decimal("100"))
        self.assertEqual(price_tiers(101, TIERS, "graduated"), Decimal("100.50"))

    def test_spans_every_tier(self):
        self.assertEqual(price_tiers(1500, TIERS, "graduated"), Decimal("600"))
        self.assertEqual(price_tiers(1000, TIERS, "graduated"), Decimal("550"))

    def test_zero_quantity(self):
        self.assertEqual(price_tiers(0, TIERS, "graduated"), 0)
        self.assertEqual(price_tiers(0, TIERS, "volume"), 0)

    def test_sub_cent_unit_price_is_unrounded(self):
        self.assertEqual(price_tiers(1001, [(None, Decimal("0.0015"))], "graduated"), Decimal("1.5015"))

    def test_list_pairs_are_accepted(self):
        self.assertEqual(price_tiers(12, [[10, 1], [None, 2]], "graduated"), Decimal("14"))


class VolumeTests(MoneyCase):
    def test_volume_prices_every_unit_at_the_quantity_tier(self):
        self.assertEqual(price_tiers(101, TIERS, "volume"), Decimal("50.50"))
        self.assertEqual(price_tiers(1001, TIERS, "volume"), Decimal("100.10"))

    def test_volume_boundary_belongs_to_lower_tier(self):
        self.assertEqual(price_tiers(100, TIERS, "volume"), Decimal("100"))
        self.assertEqual(price_tiers(1000, TIERS, "volume"), Decimal("500"))


class TierValidationTests(unittest.TestCase):
    def test_rejects_malformed_tiers(self):
        bad = [
            [],
            [(100, Decimal("1"))],
            [(None, Decimal("1")), (None, Decimal("2"))],
            [(100, Decimal("1")), (100, Decimal("2")), (None, Decimal("3"))],
            [(0, Decimal("1")), (None, Decimal("2"))],
            [(True, Decimal("1")), (None, Decimal("2"))],
            [(10, 0.5), (None, Decimal("2"))],
            [(10, Decimal("-1")), (None, Decimal("2"))],
            [(10,), (None, Decimal("2"))],
        ]
        for tiers in bad:
            with self.subTest(tiers=tiers), self.assertRaises(InvalidInput):
                price_tiers(5, tiers, "graduated")

    def test_rejects_bad_quantity_and_mode(self):
        for quantity in (-1, True, 1.0):
            with self.subTest(quantity=quantity), self.assertRaises(InvalidInput):
                price_tiers(quantity, TIERS, "graduated")
        with self.assertRaises(InvalidInput):
            price_tiers(5, TIERS, "tiered")


class AnchorTests(unittest.TestCase):
    def test_month_end_anchor_never_drifts(self):
        dates, current = [], date(2026, 1, 31)
        for _ in range(4):
            current = next_anchor(current, 31)
            dates.append(current)
        self.assertEqual(dates, [date(2026, 2, 28), date(2026, 3, 31), date(2026, 4, 30), date(2026, 5, 31)])

    def test_later_anchor_in_the_same_month(self):
        self.assertEqual(next_anchor(date(2026, 1, 15), 31), date(2026, 1, 31))
        self.assertEqual(next_anchor(date(2026, 1, 15), 20), date(2026, 1, 20))

    def test_strictly_after_current(self):
        self.assertEqual(next_anchor(date(2026, 1, 20), 20), date(2026, 2, 20))

    def test_leap_years(self):
        self.assertEqual(next_anchor(date(2024, 1, 31), 30), date(2024, 2, 29))
        self.assertEqual(next_anchor(date(2024, 2, 29), 31), date(2024, 3, 31))
        self.assertEqual(next_anchor(date(2026, 2, 27), 29), date(2026, 2, 28))

    def test_year_rollover(self):
        self.assertEqual(next_anchor(date(2026, 12, 31), 31), date(2027, 1, 31))
        self.assertEqual(next_anchor(date(2026, 12, 15), 10), date(2027, 1, 10))

    def test_rejects_invalid_arguments(self):
        for anchor in (0, 32, True, 1.0):
            with self.subTest(anchor=anchor), self.assertRaises(InvalidInput):
                next_anchor(date(2026, 1, 1), anchor)
        with self.assertRaises(InvalidInput):
            next_anchor(datetime(2026, 1, 1), 5)


JAN, FEB = date(2026, 1, 1), date(2026, 2, 1)


class ProrateTests(MoneyCase):
    def test_full_period_is_the_amount(self):
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB), Decimal("31"))

    def test_activity_before_period_is_clipped(self):
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB, active_from=date(2025, 12, 15)), Decimal("31"))

    def test_partial_start_is_half_open(self):
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB, active_from=date(2026, 1, 16)), Decimal("16"))

    def test_partial_end_is_exclusive(self):
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB, active_to=date(2026, 1, 11)), Decimal("10"))

    def test_both_bounds(self):
        value = prorate(Decimal("100.00"), JAN, FEB, date(2026, 1, 10), date(2026, 1, 20))
        self.assertMoney(quantize(value, "USD"), "32.26")

    def test_no_overlap_is_zero(self):
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB, active_from=FEB), 0)
        self.assertEqual(prorate(Decimal("31.00"), JAN, FEB, active_to=JAN), 0)
        self.assertIsInstance(prorate(Decimal("31.00"), JAN, FEB, active_from=date(2026, 3, 1)), Decimal)

    def test_negative_amounts_prorate(self):
        self.assertEqual(prorate(Decimal("-31"), JAN, FEB, active_from=date(2026, 1, 16)), Decimal("-16"))

    def test_rejects_invalid_windows(self):
        with self.assertRaises(InvalidInput):
            prorate(Decimal("1"), JAN, JAN)
        with self.assertRaises(InvalidInput):
            prorate(Decimal("1"), JAN, FEB, date(2026, 1, 20), date(2026, 1, 10))
        with self.assertRaises(InvalidInput):
            prorate(Decimal("1"), datetime(2026, 1, 1), FEB)
        with self.assertRaises(InvalidInput):
            prorate(1.0, JAN, FEB)


if __name__ == "__main__":
    unittest.main()
