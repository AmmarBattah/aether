from decimal import ROUND_HALF_EVEN, Decimal

from .errors import InvalidInput

CURRENCIES = {"USD": 2, "EUR": 2, "JPY": 0, "BHD": 3}


def exponent(currency):
    if not isinstance(currency, str) or currency not in CURRENCIES:
        raise InvalidInput(f"unsupported currency: {currency!r}")
    return CURRENCIES[currency]


def to_decimal(value, name="amount"):
    if isinstance(value, bool) or not isinstance(value, (int, Decimal)):
        raise InvalidInput(f"{name} must be a Decimal or int, got {type(value).__name__}")
    value = Decimal(value)
    if not value.is_finite():
        raise InvalidInput(f"{name} must be finite")
    return value


def _unit(currency):
    return Decimal(1).scaleb(-exponent(currency))


def quantize(amount, currency):
    unit = _unit(currency)
    return to_decimal(amount).quantize(unit, rounding=ROUND_HALF_EVEN)


def to_minor(amount, currency, name="amount"):
    places = exponent(currency)
    scaled = to_decimal(amount, name).scaleb(places)
    if scaled != scaled.to_integral_value():
        raise InvalidInput(f"{name} is more precise than the {currency} minor unit")
    return int(scaled)


def from_minor(units, currency):
    return Decimal(units).scaleb(-exponent(currency)).quantize(_unit(currency))


def allocate(total, weights, currency):
    total_minor = to_minor(total, currency, "total")
    if not isinstance(weights, list) or not weights:
        raise InvalidInput("weights must be a nonempty list")
    for weight in weights:
        if isinstance(weight, bool) or not isinstance(weight, int) or weight < 0:
            raise InvalidInput("weights must be nonnegative integers")
    weight_sum = sum(weights)
    if weight_sum == 0:
        raise InvalidInput("weights must have a positive sum")
    magnitude = abs(total_minor)
    shares, remainders = [], []
    for index, weight in enumerate(weights):
        share, remainder = divmod(magnitude * weight, weight_sum)
        shares.append(share)
        remainders.append((remainder, index))
    leftover = magnitude - sum(shares)
    for _, index in sorted(remainders, key=lambda item: (-item[0], item[1]))[:leftover]:
        shares[index] += 1
    sign = -1 if total_minor < 0 else 1
    return [from_minor(sign * share, currency) for share in shares]
