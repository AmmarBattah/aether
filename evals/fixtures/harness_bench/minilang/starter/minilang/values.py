class Function:
    __slots__ = ("name", "params", "body", "closure")

    def __init__(self, name, params, body, closure):
        self.name, self.params, self.body, self.closure = name, params, body, closure


class Builtin:
    __slots__ = ("name", "implementation")

    def __init__(self, name, implementation):
        self.name, self.implementation = name, implementation


def truthy(value):
    return bool(value)


def display(value):
    if value is None:
        return "nil"
    if value is True or value is False:
        return "true" if value else "false"
    if isinstance(value, (int, str)):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(display(item) for item in value) + "]"
    if isinstance(value, Function):
        return f"<fn {value.name}>" if value.name else "<fn>"
    return f"<builtin {value.name}>"
