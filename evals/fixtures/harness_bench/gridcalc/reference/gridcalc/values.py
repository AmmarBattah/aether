import decimal
import math

from .errors import CellError

_CONTEXT = decimal.Context(prec=400)


def is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def normalize(number):
    """A float with an integral value becomes the equal int."""
    assert is_number(number), f"not a number: {number!r}"
    if isinstance(number, float) and number.is_integer():
        return int(number)
    return number


def round_half_away(number, digits):
    """Round the shortest decimal representation half away from zero."""
    assert is_number(number) and isinstance(digits, int), "round needs a number and integer digits"
    exact = decimal.Decimal(repr(number)) if isinstance(number, float) else decimal.Decimal(number)
    rounded = exact.quantize(decimal.Decimal(1).scaleb(-digits), decimal.ROUND_HALF_UP, _CONTEXT)
    return int(rounded) if rounded == rounded.to_integral_value() else float(rounded)


def display(value):
    if value is None:
        return ""
    if isinstance(value, CellError):
        return value.code
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    if isinstance(value, str):
        return value
    assert is_number(value) and math.isfinite(value), f"cannot display {value!r}"
    if isinstance(value, int):
        return str(value)
    rounded = decimal.Decimal(repr(value)).quantize(decimal.Decimal("0.000001"), decimal.ROUND_HALF_UP, _CONTEXT)
    text = format(rounded, "f").rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


def to_number(value):
    if isinstance(value, CellError):
        return value
    if isinstance(value, bool):
        return int(value)
    if value is None:
        return 0
    if isinstance(value, str):
        return CellError("#VALUE!")
    return value


def to_text(value):
    return value if isinstance(value, CellError) else display(value)


def to_boolean(value):
    if isinstance(value, (CellError, bool)):
        return value
    if value is None:
        return False
    if isinstance(value, str):
        return CellError("#VALUE!")
    return value != 0
