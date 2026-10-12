from .errors import InvalidInput
from .money import allocate, from_minor, quantize, to_decimal, to_minor
from .periods import prorate
from .pricing import price_tiers


def build_invoice(spec):
    currency = spec["currency"]
    period = spec["period"]
    lines = []
    for line in spec["lines"]:
        if line["kind"] == "flat":
            line.setdefault("active_from", None)
            line.setdefault("active_to", None)
            raw = prorate(line["amount"], period["start"], period["end"], line["active_from"], line["active_to"])
        elif line["kind"] == "usage":
            raw = price_tiers(line["quantity"], line["tiers"], line["mode"])
        else:
            raise InvalidInput(f"unknown line kind: {line['kind']!r}")
        subtotal = to_minor(quantize(raw, currency), currency)
        lines.append({"id": line["id"], "subtotal": subtotal, "net": subtotal, "discount": 0,
                      "taxable": line["taxable"]})
    by_id = {line["id"]: line for line in lines}
    applied = []
    for discount in spec.get("discounts") or []:
        targets = lines if discount.get("applies_to") is None else [
            by_id[line_id] for line_id in discount["applies_to"] if line_id in by_id]
        base = sum(line["subtotal"] for line in targets)
        if discount["kind"] == "percent":
            amount = to_minor(quantize(from_minor(base, currency) * to_decimal(discount["percent"]) / 100, currency), currency)
        elif discount["kind"] == "fixed":
            amount = to_minor(discount["amount"], currency)
        else:
            raise InvalidInput(f"unknown discount kind: {discount['kind']!r}")
        if base and amount:
            shares = allocate(from_minor(amount, currency), [line["subtotal"] for line in targets], currency)
            for line, share in zip(targets, shares):
                line["net"] -= to_minor(share, currency)
                line["discount"] += to_minor(share, currency)
        applied.append({"id": discount["id"], "amount": from_minor(amount, currency)})
    rate = to_decimal(spec["tax_rate"])
    if not 0 <= rate < 1:
        raise InvalidInput("tax_rate must be in [0, 1)")
    rendered = []
    for line in lines:
        tax = to_minor(quantize(from_minor(line["net"], currency) * rate, currency), currency) if line["taxable"] else 0
        rendered.append({"id": line["id"], "subtotal": from_minor(line["subtotal"], currency),
                         "discount": from_minor(line["discount"], currency), "net": from_minor(line["net"], currency),
                         "tax": from_minor(tax, currency), "total": from_minor(line["net"] + tax, currency)})
    taxable_net = sum(line["net"] for line in lines if line["taxable"])
    tax = to_minor(quantize(from_minor(taxable_net, currency) * rate, currency), currency)
    net = sum(line["net"] for line in lines)
    credit = to_minor(spec.get("credit", 0), currency)
    return {
        "currency": currency,
        "lines": rendered,
        "discounts": applied,
        "subtotal": from_minor(sum(line["subtotal"] for line in lines), currency),
        "discount": from_minor(sum(line["discount"] for line in lines), currency),
        "net": from_minor(net, currency),
        "tax": from_minor(tax, currency),
        "total": from_minor(net + tax, currency),
        "credit_applied": from_minor(credit, currency),
        "amount_due": from_minor(net + tax - credit, currency),
    }
