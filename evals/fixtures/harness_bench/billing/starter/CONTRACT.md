# Usage-based invoicing

Implement the stdlib-only `invoicing` package. Every function is pure and
deterministic: it never reads clocks, files, the network, or global mutable
state, and never mutates its arguments. Money is `decimal.Decimal`; binary
floats are never accepted where money, prices, rates, or percentages are
expected. Unless stated otherwise, a "money value" is a finite `Decimal` or an
`int` (never `bool`, `float`, or `str`). Every invalid argument raises
`InvalidInput`, a subclass of `ValueError`. All Decimal arithmetic uses the
default context (28 significant digits).

## Currencies and rounding

- `CURRENCIES` is exactly `{"USD": 2, "EUR": 2, "JPY": 0, "BHD": 3}`: ISO code to
  minor-unit exponent. Any other code (including lowercase) is invalid.
- `quantize(amount, currency) -> Decimal` rounds a money value to the currency's
  minor unit with **round-half-even**. The result always carries exactly the
  currency's exponent: `quantize(5, "USD")` is `Decimal("5.00")`,
  `quantize(Decimal("0"), "BHD")` is `Decimal("0.000")`, and
  `quantize(Decimal("2.5"), "JPY")` is `Decimal("2")`.

## Allocation

`allocate(total, weights, currency) -> list[Decimal]` splits `total` across
`weights` with the largest-remainder method in minor units.

- `total` is a money value already at minor-unit precision
  (`Decimal("1.005")` is invalid for USD; `Decimal("1.50")` and `Decimal("1.5")`
  are both valid). It may be negative or zero.
- `weights` is a nonempty list of nonnegative `int` (not `bool`) whose sum is
  positive.
- Work on `T = abs(total)` in minor units and `W = sum(weights)`. Share `i`
  starts at `floor(T * w_i / W)`. The units still unallocated go one each to
  the shares with the largest remainder `(T * w_i) mod W`; equal remainders go
  to the lower index first. A zero weight always receives zero. For a negative
  total, compute the positive allocation and negate every share.
- Return one quantized Decimal per weight (same order); they sum exactly to
  `total`.

## Tiered pricing

`price_tiers(quantity, tiers, mode) -> Decimal` returns an unrounded price.

- `quantity` is a nonnegative `int` (not `bool`).
- `tiers` is a nonempty list of `(up_to, unit_price)` pairs (tuples or
  two-element lists). Every `up_to` except the last is a positive `int` (not
  `bool`), strictly increasing; the last `up_to` is `None` (unbounded) and no
  other may be `None`. `unit_price` is a nonnegative money value at any
  precision (e.g. `Decimal("0.0015")`).
- Tier `k` covers units `up_to[k-1] + 1` through `up_to[k]` inclusive (the
  first tier starts at unit 1). A quantity equal to `up_to` belongs to that tier.
- `"graduated"`: every unit is priced at the unit price of the tier containing
  that unit.
- `"volume"`: every unit is priced at the unit price of the tier containing the
  quantity itself. Quantity zero costs zero in both modes.
- Any other mode is invalid.

## Billing dates and proration

Dates are `datetime.date` values; `datetime.datetime` instances are invalid.

- `next_anchor(current, anchor_day) -> date`: monthly billing dates fall on
  `anchor_day` (an `int` 1..31, not `bool`) of every month, clamped to the last
  day of shorter months. Return the earliest billing date **strictly after**
  `current`. Clamping always starts from `anchor_day`, never from the previous
  billing date: with anchor 31, the dates after 2026-01-31 are 2026-02-28,
  2026-03-31, 2026-04-30. With anchor 31, the date after 2026-01-15 is
  2026-01-31. Leap years are respected.
- `prorate(amount, period_start, period_end, active_from=None, active_to=None)
  -> Decimal`: periods and activity windows are half-open day ranges
  `[start, end)`. `period_end` must be after `period_start`. `active_from=None`
  means since forever; `active_to=None` means forever. If both are given,
  `active_to` must not be before `active_from`. The overlap with the period is
  `overlap_days`; return `amount * overlap_days / period_days` (unrounded,
  evaluated in that order). Zero or negative overlap returns `Decimal(0)`.
  `amount` is any money value.

## Invoices

`build_invoice(spec) -> dict` validates the whole spec before computing
anything and returns a new dictionary. `spec` keys:

- `currency`: a supported code.
- `period`: `{"start": date, "end": date}` with end after start (half-open).
- `lines`: a nonempty list of line dicts, each with a nonempty string `id`
  (unique among lines), a `kind`, and a required `bool` `taxable`.
  - `kind == "flat"`: `amount` is a nonnegative money value at any precision;
    optional `active_from` / `active_to` (missing means `None`) as in `prorate`.
  - `kind == "usage"`: `quantity`, `tiers`, and `mode` as in `price_tiers`.
- `discounts`: optional list (missing means empty), applied in list order. Each
  has a nonempty string `id` (unique among discounts), a `kind`, and an
  optional `applies_to`: missing or `None` means all lines; otherwise a
  nonempty list of distinct existing line ids.
  - `kind == "percent"`: `percent` is a money value with `0 < percent <= 100`.
  - `kind == "fixed"`: `amount` is a nonnegative money value at minor-unit
    precision.
- `tax_rate`: required money value with `0 <= tax_rate < 1`.
- `credit`: optional (missing means 0) nonnegative money value at minor-unit
  precision.

Computation:

1. Line `subtotal`: `quantize(prorate(amount, period.start, period.end,
   active_from, active_to))` for flat lines and `quantize(price_tiers(...))` for
   usage lines. Each line's `net` starts at its subtotal.
2. For each discount in order, its targets are the applicable lines **in invoice
   line order** (not `applies_to` order). `base` is the sum of the targets'
   current `net`. If `base` is zero the discount amount is zero. Otherwise a
   percent discount amount is `quantize(base * percent / 100)` and a fixed
   discount amount is `min(amount, base)`. Allocate the discount amount across
   the targets with `allocate`, using each target's current net in minor units
   as its weight, then subtract each share from that line's net and add it to
   the line's `discount`. Later discounts therefore see earlier discounts.
3. Each taxable line's `tax` is `quantize(net * tax_rate)` (line-level
   rounding); non-taxable lines have zero tax. Line `total` is `net + tax`.
4. Invoice `subtotal`, `discount`, `net`, `tax`, and `total` are the sums of the
   line values. `credit_applied` is `min(credit, total)` and `amount_due` is
   `total - credit_applied`.

The result is:

```python
{
    "currency": str,
    "lines": [{"id", "subtotal", "discount", "net", "tax", "total"}, ...],  # line order
    "discounts": [{"id", "amount"}, ...],  # discount order
    "subtotal", "discount", "net", "tax", "total", "credit_applied", "amount_due",
}
```

Every money value in the result is a Decimal quantized to the currency's
exponent, including zeros.
