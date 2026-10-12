import math

from .errors import CellError
from .values import is_number, normalize, round_half_away, to_boolean, to_number, to_text

ARITY = {
    "SUM": (1, None), "AVERAGE": (1, None), "MIN": (1, None), "MAX": (1, None), "COUNT": (1, None),
    "CONCAT": (1, None), "IF": (2, 3), "IFERROR": (2, 2), "AND": (1, None), "OR": (1, None), "NOT": (1, 1),
    "ABS": (1, 1), "INT": (1, 1), "ROUND": (2, 2), "MOD": (2, 2), "LEN": (1, 1), "UPPER": (1, 1),
    "LOWER": (1, 1), "LEFT": (2, 2), "RIGHT": (2, 2), "ISBLANK": (1, 1), "ISNUMBER": (1, 1),
    "ISTEXT": (1, 1), "ISERROR": (1, 1),
}
COLLECTING = ("SUM", "AVERAGE", "MIN", "MAX", "COUNT", "CONCAT")
VALUE_ERROR = CellError("#VALUE!")


def _round(number, digits):
    return round_half_away(number, digits) if isinstance(digits, int) else VALUE_ERROR


def _mod(dividend, divisor):
    return CellError("#DIV/0!") if divisor == 0 else dividend - divisor * math.floor(dividend / divisor)


def _left(text, count):
    return text[:count] if isinstance(count, int) and count >= 0 else VALUE_ERROR


def _right(text, count):
    if not isinstance(count, int) or count < 0:
        return VALUE_ERROR
    return text[len(text) - count:] if count < len(text) else text


# name -> (coercion per argument, repeated for variadic functions; implementation)
SCALAR = {
    "AND": ((to_boolean,), lambda *flags: all(flags)),
    "OR": ((to_boolean,), lambda *flags: any(flags)),
    "NOT": ((to_boolean,), lambda flag: not flag),
    "ABS": ((to_number,), abs),
    "INT": ((to_number,), math.floor),
    "ROUND": ((to_number, to_number), _round),
    "MOD": ((to_number, to_number), _mod),
    "LEN": ((to_text,), len),
    "UPPER": ((to_text,), str.upper),
    "LOWER": ((to_text,), str.lower),
    "LEFT": ((to_text, to_number), _left),
    "RIGHT": ((to_text, to_number), _right),
}
INSPECT = {
    "ISNUMBER": is_number,
    "ISTEXT": lambda value: isinstance(value, str),
    "ISERROR": lambda value: isinstance(value, CellError),
}


def call_scalar(name, values):
    """Coerce left to right; the first error, own or from coercion, is the result."""
    coercions, implementation = SCALAR[name]
    assert values and len(coercions) in (1, len(values)), f"{name} got {len(values)} arguments"
    coerced = []
    for index, value in enumerate(values):
        item = coercions[min(index, len(coercions) - 1)](value)
        if isinstance(item, CellError):
            return item
        coerced.append(item)
    result = implementation(*coerced)
    return normalize(result) if is_number(result) else result


def collect(name, items):
    """items: (is_reference, value) in argument and row-major order."""
    assert name in COLLECTING and items, f"{name} needs something to collect"
    numbers, texts = [], []
    for is_reference, value in items:
        if name == "COUNT":
            numbers += [value] if is_number(value) else []
            continue
        if isinstance(value, CellError):
            return value
        if name == "CONCAT":
            texts.append(to_text(value))
        elif not is_reference:
            numbers.append(to_number(value))
            if isinstance(numbers[-1], CellError):
                return numbers[-1]
        elif is_number(value):
            numbers.append(value)
    return "".join(texts) if name == "CONCAT" else _aggregate(name, numbers)


def _aggregate(name, numbers):
    if name == "COUNT":
        return len(numbers)
    if name == "SUM":
        return normalize(sum(numbers))
    if name == "AVERAGE":
        return normalize(sum(numbers) / len(numbers)) if numbers else CellError("#DIV/0!")
    assert name in ("MIN", "MAX"), f"not an aggregate: {name}"
    return (min if name == "MIN" else max)(numbers) if numbers else 0
