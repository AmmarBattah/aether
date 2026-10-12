class Function:
    __slots__ = ("name", "params", "body", "closure")

    def __init__(self, name, params, body, closure):
        self.name, self.params, self.body, self.closure = name, params, body, closure


class Builtin:
    __slots__ = ("name", "arity", "implementation")

    def __init__(self, name, arity, implementation):
        self.name, self.arity, self.implementation = name, arity, implementation


def is_int(value):
    return type(value) is int


def truthy(value):
    return value is not None and value is not False


def equal(left, right):
    if type(left) is not type(right):
        return False
    if isinstance(left, list):
        return left is right or (len(left) == len(right) and all(map(equal, left, right)))
    if isinstance(left, (Function, Builtin)):
        return left is right
    return left == right


def _quote(text):
    escaped = text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t")
    return f'"{escaped}"'


def display(value, nested=False, active=None):
    if value is None:
        return "nil"
    if value is True or value is False:
        return "true" if value else "false"
    if is_int(value):
        return str(value)
    if isinstance(value, str):
        return _quote(value) if nested else value
    if isinstance(value, list):
        active = set() if active is None else active
        if id(value) in active:
            return "[...]"
        active.add(id(value))
        try:
            return "[" + ", ".join(display(item, True, active) for item in value) + "]"
        finally:
            active.discard(id(value))
    if isinstance(value, Function):
        return f"<fn {value.name}>" if value.name else "<fn>"
    return f"<builtin {value.name}>"
