from .errors import InvalidInput
from .money import allocate, exponent, from_minor, quantize, to_decimal, to_minor
from .periods import prorate, validate_date, validate_window
from .pricing import price_tiers, validate_quantity, validate_tiers

_MISSING = object()


def build_invoice(spec):
    plan = _validate(spec)
    currency = plan["currency"]
    lines = [{"id": line["id"], "subtotal": _line_subtotal(line, plan), "taxable": line["taxable"]}
             for line in plan["lines"]]
    for line in lines:
        line["net"], line["discount"] = line["subtotal"], 0
    applied = []
    for discount in plan["discounts"]:
        targets = [line for line in lines if discount["targets"] is None or line["id"] in discount["targets"]]
        base = sum(line["net"] for line in targets)
        amount = 0 if base == 0 else _discount_minor(discount, base, currency)
        if amount:
            shares = allocate(from_minor(amount, currency), [line["net"] for line in targets], currency)
            for line, share in zip(targets, shares):
                share_minor = to_minor(share, currency)
                line["net"] -= share_minor
                line["discount"] += share_minor
        applied.append({"id": discount["id"], "amount": from_minor(amount, currency)})
    rate = plan["tax_rate"]
    rendered = []
    for line in lines:
        tax = to_minor(quantize(from_minor(line["net"], currency) * rate, currency), currency) if line["taxable"] else 0
        line["tax"], line["total"] = tax, line["net"] + tax
        rendered.append({key: line[key] if key == "id" else from_minor(line[key], currency)
                         for key in ("id", "subtotal", "discount", "net", "tax", "total")})
    sums = {key: sum(line[key] for line in lines) for key in ("subtotal", "discount", "net", "tax", "total")}
    credit_applied = min(plan["credit"], sums["total"])
    result = {"currency": currency, "lines": rendered, "discounts": applied}
    result.update({key: from_minor(value, currency) for key, value in sums.items()})
    result["credit_applied"] = from_minor(credit_applied, currency)
    result["amount_due"] = from_minor(sums["total"] - credit_applied, currency)
    return result


def _line_subtotal(line, plan):
    if line["kind"] == "flat":
        start, end = plan["period"]
        raw = prorate(line["amount"], start, end, line["active_from"], line["active_to"])
    else:
        raw = price_tiers(line["quantity"], line["tiers"], line["mode"])
    return to_minor(quantize(raw, plan["currency"]), plan["currency"])


def _discount_minor(discount, base, currency):
    if discount["kind"] == "fixed":
        return min(discount["amount"], base)
    value = from_minor(base, currency) * discount["percent"] / 100
    return to_minor(quantize(value, currency), currency)


def _get(mapping, key, default=_MISSING):
    if key in mapping:
        return mapping[key]
    if default is _MISSING:
        raise InvalidInput(f"missing required field {key!r}")
    return default


def _nonempty_id(value, what):
    if not isinstance(value, str) or not value:
        raise InvalidInput(f"{what} id must be a nonempty string")
    return value


def _validate(spec):
    if not isinstance(spec, dict):
        raise InvalidInput("spec must be a dict")
    currency = _get(spec, "currency")
    exponent(currency)
    period = _get(spec, "period")
    if not isinstance(period, dict):
        raise InvalidInput("period must be a dict")
    start = validate_date(_get(period, "start"), "period.start")
    end = validate_date(_get(period, "end"), "period.end")
    validate_window(start, end, None, None)
    lines = _get(spec, "lines")
    if not isinstance(lines, list) or not lines:
        raise InvalidInput("lines must be a nonempty list")
    plan_lines = [_validate_line(line, start, end) for line in lines]
    line_ids = [line["id"] for line in plan_lines]
    if len(set(line_ids)) != len(line_ids):
        raise InvalidInput("line ids must be unique")
    discounts = _get(spec, "discounts", None)
    discounts = [] if discounts is None else discounts
    if not isinstance(discounts, list):
        raise InvalidInput("discounts must be a list")
    plan_discounts = [_validate_discount(discount, set(line_ids), currency) for discount in discounts]
    discount_ids = [discount["id"] for discount in plan_discounts]
    if len(set(discount_ids)) != len(discount_ids):
        raise InvalidInput("discount ids must be unique")
    tax_rate = to_decimal(_get(spec, "tax_rate"), "tax_rate")
    if not 0 <= tax_rate < 1:
        raise InvalidInput("tax_rate must be in [0, 1)")
    credit = to_minor(_get(spec, "credit", 0), currency, "credit")
    if credit < 0:
        raise InvalidInput("credit must be nonnegative")
    return {"currency": currency, "period": (start, end), "lines": plan_lines,
            "discounts": plan_discounts, "tax_rate": tax_rate, "credit": credit}


def _validate_line(line, start, end):
    if not isinstance(line, dict):
        raise InvalidInput("each line must be a dict")
    line_id = _nonempty_id(_get(line, "id"), "line")
    taxable = _get(line, "taxable")
    if not isinstance(taxable, bool):
        raise InvalidInput("taxable must be a bool")
    kind = _get(line, "kind")
    if kind == "flat":
        amount = to_decimal(_get(line, "amount"))
        if amount < 0:
            raise InvalidInput("flat amount must be nonnegative")
        active_from, active_to = line.get("active_from"), line.get("active_to")
        validate_window(start, end, active_from, active_to)
        return {"id": line_id, "kind": kind, "taxable": taxable, "amount": amount,
                "active_from": active_from, "active_to": active_to}
    if kind == "usage":
        return {"id": line_id, "kind": kind, "taxable": taxable,
                "quantity": validate_quantity(_get(line, "quantity")),
                "tiers": validate_tiers(_get(line, "tiers")), "mode": _validate_mode(_get(line, "mode"))}
    raise InvalidInput(f"unknown line kind: {kind!r}")


def _validate_mode(mode):
    if mode not in ("graduated", "volume"):
        raise InvalidInput(f"unknown pricing mode: {mode!r}")
    return mode


def _validate_discount(discount, line_ids, currency):
    if not isinstance(discount, dict):
        raise InvalidInput("each discount must be a dict")
    discount_id = _nonempty_id(_get(discount, "id"), "discount")
    targets = discount.get("applies_to")
    if targets is not None:
        if (not isinstance(targets, list) or not targets or len(set(targets)) != len(targets)
                or any(not isinstance(target, str) or target not in line_ids for target in targets)):
            raise InvalidInput("applies_to must be a nonempty list of distinct existing line ids")
        targets = set(targets)
    kind = _get(discount, "kind")
    plan = {"id": discount_id, "kind": kind, "targets": targets}
    if kind == "percent":
        percent = to_decimal(_get(discount, "percent"), "percent")
        if not 0 < percent <= 100:
            raise InvalidInput("percent must be in (0, 100]")
        plan["percent"] = percent
    elif kind == "fixed":
        amount = to_minor(_get(discount, "amount"), currency, "discount amount")
        if amount < 0:
            raise InvalidInput("fixed discount must be nonnegative")
        plan["amount"] = amount
    else:
        raise InvalidInput(f"unknown discount kind: {kind!r}")
    return plan
