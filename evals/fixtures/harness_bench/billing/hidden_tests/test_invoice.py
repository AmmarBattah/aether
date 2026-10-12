import copy
import unittest
from datetime import date
from decimal import Decimal

from invoicing import InvalidInput, build_invoice
from support import MoneyCase

TIERS = [(100, Decimal("1.00")), (1000, Decimal("0.50")), (None, Decimal("0.10"))]


def make_spec(**overrides):
    spec = {
        "currency": "USD",
        "period": {"start": date(2026, 1, 1), "end": date(2026, 2, 1)},
        "lines": [
            {"id": "seat", "kind": "flat", "amount": Decimal("31.00"), "taxable": True},
            {"id": "api", "kind": "usage", "quantity": 1500, "tiers": TIERS, "mode": "graduated", "taxable": True},
            {"id": "support", "kind": "flat", "amount": Decimal("10.00"), "taxable": False},
        ],
        "tax_rate": Decimal("0.0825"),
    }
    spec.update(overrides)
    return spec


def flat(line_id, amount, taxable=False, **extra):
    return {"id": line_id, "kind": "flat", "amount": Decimal(amount), "taxable": taxable, **extra}


def lines_by_id(invoice):
    return {line["id"]: line for line in invoice["lines"]}


class InvoiceTotalsTests(MoneyCase):
    def test_simple_invoice(self):
        invoice = build_invoice(make_spec())
        self.assertEqual(invoice["currency"], "USD")
        self.assertEqual([line["id"] for line in invoice["lines"]], ["seat", "api", "support"])
        self.assertMoney(invoice["subtotal"], "641.00")
        self.assertMoney(invoice["discount"], "0.00")
        self.assertMoney(invoice["tax"], "52.06")
        self.assertMoney(invoice["total"], "693.06")
        self.assertMoney(invoice["credit_applied"], "0.00")
        self.assertMoney(invoice["amount_due"], "693.06")
        self.assertEqual(invoice["discounts"], [])

    def test_line_fields(self):
        seat = lines_by_id(build_invoice(make_spec()))["seat"]
        self.assertEqual(set(seat), {"id", "subtotal", "discount", "net", "tax", "total"})
        self.assertMoney(seat["subtotal"], "31.00")
        self.assertMoney(seat["discount"], "0.00")
        self.assertMoney(seat["net"], "31.00")
        self.assertMoney(seat["tax"], "2.56")
        self.assertMoney(seat["total"], "33.56")

    def test_non_taxable_line_has_zero_tax(self):
        support = lines_by_id(build_invoice(make_spec()))["support"]
        self.assertMoney(support["tax"], "0.00")
        self.assertMoney(support["total"], "10.00")

    def test_tax_is_rounded_per_line(self):
        spec = make_spec(lines=[flat("a", "0.30", True), flat("b", "0.30", True)], tax_rate=Decimal("0.05"))
        self.assertMoney(build_invoice(spec)["tax"], "0.04")

    def test_tax_rounding_is_half_even(self):
        spec = make_spec(lines=[flat("a", "0.10", True)], tax_rate=Decimal("0.05"))
        self.assertMoney(build_invoice(spec)["tax"], "0.00")

    def test_proration_inside_invoice(self):
        lines = [flat("seat", "31.00", True, active_from=date(2026, 1, 16))]
        invoice = build_invoice(make_spec(lines=lines))
        self.assertMoney(invoice["lines"][0]["subtotal"], "16.00")
        self.assertMoney(invoice["tax"], "1.32")

    def test_subtotal_rounding_is_half_even(self):
        invoice = build_invoice(make_spec(lines=[flat("a", "10.005")], tax_rate=0))
        self.assertMoney(invoice["subtotal"], "10.00")

    def test_volume_usage_line(self):
        line = {"id": "u", "kind": "usage", "quantity": 1000, "tiers": TIERS, "mode": "volume", "taxable": False}
        self.assertMoney(build_invoice(make_spec(lines=[line]))["subtotal"], "500.00")

    def test_zero_invoice_keeps_exponents(self):
        invoice = build_invoice(make_spec(lines=[flat("a", "0")], tax_rate=0))
        for key in ("subtotal", "discount", "net", "tax", "total", "credit_applied", "amount_due"):
            self.assertMoney(invoice[key], "0.00")
        for key in ("subtotal", "discount", "net", "tax", "total"):
            self.assertMoney(invoice["lines"][0][key], "0.00")

    def test_zero_exponent_currency(self):
        lines = [flat("plan", 1000, True),
                 {"id": "u", "kind": "usage", "quantity": 5, "tiers": [(None, Decimal("0.5"))],
                  "mode": "graduated", "taxable": True}]
        invoice = build_invoice(make_spec(currency="JPY", lines=lines, tax_rate=Decimal("0.1")))
        self.assertMoneyList([line["subtotal"] for line in invoice["lines"]], ["1000", "2"])
        self.assertMoney(invoice["tax"], "100")
        self.assertMoney(invoice["total"], "1102")


class DiscountTests(MoneyCase):
    def test_percent_discount_on_all_lines(self):
        spec = make_spec(discounts=[{"id": "ten", "kind": "percent", "percent": Decimal("10")}])
        invoice = build_invoice(spec)
        self.assertEqual(len(invoice["discounts"]), 1)
        self.assertEqual(invoice["discounts"][0]["id"], "ten")
        self.assertMoney(invoice["discounts"][0]["amount"], "64.10")
        self.assertMoneyList([line["net"] for line in invoice["lines"]], ["27.90", "540.00", "9.00"])
        self.assertMoney(invoice["tax"], "46.85")
        self.assertMoney(invoice["total"], "623.75")

    def test_discounts_compound_in_list_order(self):
        spec = make_spec(discounts=[
            {"id": "ten", "kind": "percent", "percent": 10},
            {"id": "half-api", "kind": "percent", "percent": 50, "applies_to": ["api"]},
        ])
        invoice = build_invoice(spec)
        self.assertMoneyList([item["amount"] for item in invoice["discounts"]], ["64.10", "270.00"])
        api = lines_by_id(invoice)["api"]
        self.assertMoney(api["discount"], "330.00")
        self.assertMoney(api["net"], "270.00")
        self.assertMoney(api["tax"], "22.28")

    def test_fixed_discount_is_capped_at_its_base(self):
        spec = make_spec(discounts=[{"id": "f", "kind": "fixed", "amount": Decimal("50.00"), "applies_to": ["support"]}])
        invoice = build_invoice(spec)
        self.assertMoney(invoice["discounts"][0]["amount"], "10.00")
        self.assertMoney(lines_by_id(invoice)["support"]["net"], "0.00")
        self.assertMoney(invoice["discount"], "10.00")

    def test_fixed_discount_uses_largest_remainder(self):
        lines = [flat("a", "1.00"), flat("b", "1.00"), flat("c", "1.00")]
        invoice = build_invoice(make_spec(lines=lines, discounts=[{"id": "f", "kind": "fixed", "amount": Decimal("1.00")}]))
        self.assertMoneyList([line["discount"] for line in invoice["lines"]], ["0.34", "0.33", "0.33"])

    def test_targets_follow_invoice_line_order(self):
        lines = [flat("a", "1.00"), flat("b", "1.00"), flat("c", "1.00")]
        discounts = [{"id": "f", "kind": "fixed", "amount": Decimal("1.00"), "applies_to": ["c", "b", "a"]}]
        invoice = build_invoice(make_spec(lines=lines, discounts=discounts))
        self.assertMoneyList([line["discount"] for line in invoice["lines"]], ["0.34", "0.33", "0.33"])

    def test_subset_targets_tie_break_by_line_order(self):
        lines = [flat("a", "1.00"), flat("b", "1.00"), flat("c", "1.00")]
        discounts = [{"id": "f", "kind": "fixed", "amount": Decimal("0.01"), "applies_to": ["c", "a"]}]
        invoice = build_invoice(make_spec(lines=lines, discounts=discounts))
        self.assertMoneyList([line["discount"] for line in invoice["lines"]], ["0.01", "0.00", "0.00"])

    def test_allocation_weights_are_current_net(self):
        lines = [flat("a", "1.00"), flat("b", "1.00"), flat("c", "1.00")]
        discounts = [
            {"id": "first", "kind": "fixed", "amount": Decimal("1.00"), "applies_to": ["a"]},
            {"id": "all", "kind": "percent", "percent": 100},
        ]
        invoice = build_invoice(make_spec(lines=lines, discounts=discounts))
        self.assertMoneyList([item["amount"] for item in invoice["discounts"]], ["1.00", "2.00"])
        self.assertMoneyList([line["net"] for line in invoice["lines"]], ["0.00", "0.00", "0.00"])
        self.assertMoneyList([line["discount"] for line in invoice["lines"]], ["1.00", "1.00", "1.00"])

    def test_uneven_percent_allocation(self):
        lines = [flat("a", "0.01"), flat("b", "0.02"), flat("c", "0.04")]
        discounts = [{"id": "p", "kind": "percent", "percent": Decimal("50")}]
        invoice = build_invoice(make_spec(lines=lines, discounts=discounts))
        # base 0.07 -> 0.035 -> half-even 0.04; weights 1,2,4 of 7.
        self.assertMoney(invoice["discounts"][0]["amount"], "0.04")
        self.assertMoneyList([line["discount"] for line in invoice["lines"]], ["0.01", "0.01", "0.02"])

    def test_zero_base_discount_is_zero(self):
        line = {"id": "u", "kind": "usage", "quantity": 0, "tiers": TIERS, "mode": "graduated", "taxable": True}
        discounts = [{"id": "f", "kind": "fixed", "amount": Decimal("5.00")}]
        invoice = build_invoice(make_spec(lines=[line], discounts=discounts))
        self.assertMoney(invoice["discounts"][0]["amount"], "0.00")
        self.assertMoney(invoice["total"], "0.00")

    def test_full_percent_discount(self):
        invoice = build_invoice(make_spec(discounts=[{"id": "all", "kind": "percent", "percent": 100}]))
        self.assertMoney(invoice["net"], "0.00")
        self.assertMoney(invoice["tax"], "0.00")
        self.assertMoney(invoice["total"], "0.00")

    def test_missing_and_none_discounts_are_empty(self):
        with_none = build_invoice(make_spec(discounts=None))
        self.assertEqual(with_none, build_invoice(make_spec()))


class CreditTests(MoneyCase):
    def test_credit_is_capped_at_total(self):
        invoice = build_invoice(make_spec(credit=Decimal("1000.00")))
        self.assertMoney(invoice["credit_applied"], "693.06")
        self.assertMoney(invoice["amount_due"], "0.00")

    def test_partial_credit(self):
        invoice = build_invoice(make_spec(credit=Decimal("100")))
        self.assertMoney(invoice["credit_applied"], "100.00")
        self.assertMoney(invoice["amount_due"], "593.06")


class PurityTests(unittest.TestCase):
    def test_spec_is_not_mutated(self):
        spec = make_spec(discounts=[{"id": "ten", "kind": "percent", "percent": 10, "applies_to": ["api", "seat"]}],
                         credit=Decimal("5.00"))
        before = copy.deepcopy(spec)
        build_invoice(spec)
        self.assertEqual(spec, before)

    def test_repeated_builds_are_identical(self):
        spec = make_spec(discounts=[{"id": "ten", "kind": "percent", "percent": 10}])
        self.assertEqual(build_invoice(spec), build_invoice(spec))


class InvoiceValidationTests(unittest.TestCase):
    def assertInvalid(self, spec):
        with self.assertRaises(InvalidInput):
            build_invoice(spec)

    def test_duplicate_line_ids(self):
        self.assertInvalid(make_spec(lines=[flat("a", "1"), flat("a", "2")]))

    def test_bad_applies_to(self):
        for targets in (["nope"], [], ["api", "api"]):
            with self.subTest(targets=targets):
                self.assertInvalid(make_spec(discounts=[{"id": "d", "kind": "percent", "percent": 5, "applies_to": targets}]))

    def test_duplicate_discount_ids(self):
        discount = {"id": "d", "kind": "percent", "percent": 5}
        self.assertInvalid(make_spec(discounts=[discount, dict(discount)]))

    def test_percent_bounds(self):
        for percent in (0, Decimal("100.01"), -5, 0.5):
            with self.subTest(percent=percent):
                self.assertInvalid(make_spec(discounts=[{"id": "d", "kind": "percent", "percent": percent}]))

    def test_fixed_amount_precision(self):
        self.assertInvalid(make_spec(discounts=[{"id": "d", "kind": "fixed", "amount": Decimal("0.005")}]))
        self.assertInvalid(make_spec(discounts=[{"id": "d", "kind": "fixed", "amount": Decimal("-1")}]))

    def test_taxable_must_be_a_bool(self):
        line = flat("a", "1")
        line["taxable"] = 1
        self.assertInvalid(make_spec(lines=[line]))
        del line["taxable"]
        self.assertInvalid(make_spec(lines=[line]))

    def test_tax_rate_bounds(self):
        for rate in (1, Decimal("-0.01"), 0.1):
            with self.subTest(rate=rate):
                self.assertInvalid(make_spec(tax_rate=rate))
        spec = make_spec()
        del spec["tax_rate"]
        self.assertInvalid(spec)

    def test_credit_validation(self):
        self.assertInvalid(make_spec(credit=Decimal("0.001")))
        self.assertInvalid(make_spec(credit=Decimal("-1")))

    def test_line_validation(self):
        self.assertInvalid(make_spec(lines=[]))
        self.assertInvalid(make_spec(lines=[flat("a", "-1")]))
        self.assertInvalid(make_spec(lines=[{"id": "u", "kind": "usage", "quantity": 1, "tiers": TIERS,
                                             "mode": "tiered", "taxable": True}]))
        self.assertInvalid(make_spec(lines=[{"id": "x", "kind": "other", "taxable": True}]))
        self.assertInvalid(make_spec(lines=[flat("", "1")]))

    def test_period_validation(self):
        self.assertInvalid(make_spec(period={"start": date(2026, 2, 1), "end": date(2026, 2, 1)}))
        self.assertInvalid(make_spec(currency="GBP"))


if __name__ == "__main__":
    unittest.main()
