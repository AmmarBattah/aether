from .errors import CellError
from .functions import ARITY, COLLECTING, INSPECT, SCALAR, call_scalar, collect
from .values import is_number, normalize, to_boolean, to_number, to_text

VALUE_ERROR = CellError("#VALUE!")
COMPARE = {"=": lambda c: c == 0, "<>": lambda c: c != 0, "<": lambda c: c < 0,
           "<=": lambda c: c <= 0, ">": lambda c: c > 0, ">=": lambda c: c >= 0}


def evaluate(node, lookup):
    """Single value of a node; a range where one value is needed is #VALUE!."""
    kind = node[0]
    if kind in ("num", "str", "bool"):
        return node[1]
    if kind == "err":
        return CellError(node[1])
    if kind == "cell":
        return lookup(node[1], node[2])
    if kind == "range":
        return VALUE_ERROR
    if kind == "percent":
        return _arithmetic("/", evaluate(node[1], lookup), 100)
    if kind == "unary":
        return _arithmetic("-" if node[1] == "-" else "+", 0, evaluate(node[2], lookup))
    if kind == "binary":
        return _binary(node[1], evaluate(node[2], lookup), evaluate(node[3], lookup))
    assert kind == "call", f"unknown node {kind!r}"
    return _call(node[1], node[2], lookup)


def _binary(operator, left, right):
    if operator in COMPARE or operator == "&":
        if isinstance(left, CellError):
            return left
        if isinstance(right, CellError):
            return right
        return to_text(left) + to_text(right) if operator == "&" else COMPARE[operator](_order(left, right))
    return _arithmetic(operator, left, right)


def _arithmetic(operator, left, right):
    left = to_number(left)
    if isinstance(left, CellError):
        return left
    right = to_number(right)
    if isinstance(right, CellError):
        return right
    if operator == "^":
        return _power(left, right)
    if operator == "/" and right == 0:
        return CellError("#DIV/0!")
    if operator == "/":
        return normalize(left / right)
    return normalize({"+": left + right, "-": left - right, "*": left * right}[operator])


def _power(base, exponent):
    if base == 0 and exponent < 0:
        return CellError("#DIV/0!")
    try:
        result = base ** exponent
    except OverflowError:
        return CellError("#NUM!")
    if isinstance(result, complex) or result != result or result in (float("inf"), float("-inf")):
        return CellError("#NUM!")
    return normalize(result)


def _rank(value):
    return 2 if isinstance(value, bool) else 1 if isinstance(value, str) else 0


def _order(left, right):
    """Negative, zero or positive as left sorts before, with or after right."""
    if left is None and right is None:
        return 0
    left = _like(right) if left is None else left
    right = _like(left) if right is None else right
    if _rank(left) != _rank(right):
        return _rank(left) - _rank(right)
    if isinstance(left, str):
        left, right = left.lower(), right.lower()
    return (left > right) - (left < right)


def _like(other):
    assert other is not None, "two empties are compared before this"
    return False if isinstance(other, bool) else "" if isinstance(other, str) else 0


def _call(name, arguments, lookup):
    if name not in ARITY:
        return CellError("#NAME?")
    if name == "IF":
        return _if(arguments, lookup)
    if name == "IFERROR":
        value = evaluate(arguments[0], lookup)
        return evaluate(arguments[1], lookup) if isinstance(value, CellError) else value
    if name == "ISBLANK":
        return arguments[0][0] == "cell" and evaluate(arguments[0], lookup) is None
    if name in INSPECT:
        return INSPECT[name](evaluate(arguments[0], lookup))
    if name in COLLECTING:
        return collect(name, [item for argument in arguments for item in _items(argument, lookup)])
    assert name in SCALAR, f"{name} has an arity but no implementation"
    return call_scalar(name, [evaluate(argument, lookup) for argument in arguments])


def _if(arguments, lookup):
    condition = to_boolean(evaluate(arguments[0], lookup))
    if isinstance(condition, CellError):
        return condition
    if condition:
        return evaluate(arguments[1], lookup)
    return evaluate(arguments[2], lookup) if len(arguments) == 3 else False


def _items(argument, lookup):
    if argument[0] == "cell":
        return [(True, lookup(argument[1], argument[2]))]
    if argument[0] == "range":
        _, top, left, bottom, right = argument
        return [(True, lookup(row, col)) for row in range(top, bottom + 1) for col in range(left, right + 1)]
    return [(False, evaluate(argument, lookup))]


def formula_value(node, lookup):
    """Value of a whole formula: never empty, numbers normalised."""
    value = evaluate(node, lookup)
    if value is None:
        return 0
    assert isinstance(value, (CellError, str, bool)) or is_number(value), f"not a cell value: {value!r}"
    return normalize(value) if is_number(value) else value
