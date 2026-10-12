from decimal import Decimal

from .errors import InvalidInput
from .money import to_decimal


def validate_quantity(quantity):
    if isinstance(quantity, bool) or not isinstance(quantity, int) or quantity < 0:
        raise InvalidInput("quantity must be a nonnegative int")
    return quantity


def validate_tiers(tiers):
    if not isinstance(tiers, list) or not tiers:
        raise InvalidInput("tiers must be a nonempty list")
    validated, previous = [], 0
    for position, tier in enumerate(tiers):
        if not isinstance(tier, (list, tuple)) or len(tier) != 2:
            raise InvalidInput("each tier is an (up_to, unit_price) pair")
        up_to, price = tier
        last = position == len(tiers) - 1
        if last:
            if up_to is not None:
                raise InvalidInput("the last tier must be unbounded")
        elif isinstance(up_to, bool) or not isinstance(up_to, int) or up_to <= previous:
            raise InvalidInput("tier bounds must be strictly increasing positive ints")
        price = to_decimal(price, "unit_price")
        if price < 0:
            raise InvalidInput("unit_price must be nonnegative")
        validated.append((up_to, price))
        previous = up_to
    return validated


def price_tiers(quantity, tiers, mode):
    quantity = validate_quantity(quantity)
    tiers = validate_tiers(tiers)
    if mode == "graduated":
        return _graduated(quantity, tiers)
    if mode == "volume":
        return _volume(quantity, tiers)
    raise InvalidInput(f"unknown pricing mode: {mode!r}")


def _graduated(quantity, tiers):
    total, lower = Decimal(0), 0
    for up_to, price in tiers:
        upper = quantity if up_to is None else min(quantity, up_to)
        if upper > lower:
            total += (upper - lower) * price
        if up_to is None or quantity <= up_to:
            break
        lower = up_to
    return total


def _volume(quantity, tiers):
    if quantity == 0:
        return Decimal(0)
    for up_to, price in tiers:
        if up_to is None or quantity <= up_to:
            return quantity * price
    raise AssertionError("validated tiers always end unbounded")
