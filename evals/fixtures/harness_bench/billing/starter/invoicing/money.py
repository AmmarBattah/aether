from decimal import ROUND_HALF_UP, Decimal

from .errors import InvalidInput

CURRENCIES = {"USD": 2, "EUR": 2, "JPY": 0, "BHD": 3}


def exponent(currency):
    if currency not in CURRENCIES:
        raise InvalidInput(f"unsupported currency: {currency!r}")
    return CURRENCIES[currency]


def to_decimal(value, name="amount"):
    if isinstance(value, float):
        return Decimal(str(value))
    if not isinstance(value, (int, Decimal)):
        raise InvalidInput(f"{name} must be a Decimal or int, got {type(value).__name__}")
    return Decimal(value)


def quantize(amount, currency):
    places = exponent(currency)
    return to_decimal(amount).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)


def to_minor(amount, currency, name="amount"):
    places = exponent(currency)
    scaled = to_decimal(amount, name).scaleb(places)
    if scaled != scaled.to_integral_value():
        raise InvalidInput(f"{name} is more precise than the {currency} minor unit")
    return int(scaled)


def from_minor(units, currency):
    return Decimal(units) / (10 ** exponent(currency))


def allocate(total, weights, currency):
    total_minor = to_minor(total, currency, "total")
    if not weights:
        raise InvalidInput("weights must be a nonempty list")
    weight_sum = sum(weights)
    if weight_sum == 0:
        raise InvalidInput("weights must have a positive sum")
    shares = [total_minor * weight // weight_sum for weight in weights]
    leftover = total_minor - sum(shares)
    for index in range(leftover):
        shares[index % len(shares)] += 1
    return [from_minor(share, currency) for share in shares]
