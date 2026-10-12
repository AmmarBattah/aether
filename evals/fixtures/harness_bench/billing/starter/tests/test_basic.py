import unittest
from datetime import date
from decimal import Decimal

from invoicing import allocate, build_invoice, price_tiers, quantize


class SmokeTests(unittest.TestCase):
    def test_quantize_usd(self):
        self.assertEqual(quantize(Decimal("1.234"), "USD"), Decimal("1.23"))

    def test_allocate_sums_to_total(self):
        self.assertEqual(sum(allocate(Decimal("10.00"), [1, 1, 1], "USD")), Decimal("10.00"))

    def test_graduated_first_tier(self):
        tiers = [(10, Decimal("2")), (None, Decimal("1"))]
        self.assertEqual(price_tiers(5, tiers, "graduated"), Decimal("10"))

    def test_single_line_invoice(self):
        spec = {
            "currency": "USD",
            "period": {"start": date(2026, 1, 1), "end": date(2026, 2, 1)},
            "lines": [{"id": "plan", "kind": "flat", "amount": Decimal("20.00"), "taxable": False}],
            "tax_rate": Decimal("0"),
        }
        self.assertEqual(build_invoice(spec)["amount_due"], Decimal("20.00"))
