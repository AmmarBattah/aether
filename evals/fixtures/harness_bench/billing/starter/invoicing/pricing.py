from decimal import Decimal

from .errors import InvalidInput
from .money import to_decimal


def validate_quantity(quantity):
    if not isinstance(quantity, int) or quantity < 0:
        raise InvalidInput("quantity must be a nonnegative int")
    return quantity


def validate_tiers(tiers):
    if not tiers:
        raise InvalidInput("tiers must be a nonempty list")
    validated = []
    for up_to, price in tiers:
        price = to_decimal(price, "unit_price")
        if price < 0:
            raise InvalidInput("unit_price must be nonnegative")
        validated.append((up_to, price))
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
        lower = up_to + 1
    return total


def _volume(quantity, tiers):
    for up_to, price in tiers:
        if up_to is None or quantity < up_to:
            return quantity * price
    return Decimal(0)
