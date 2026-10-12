from .errors import MiniNameError, MiniRuntimeError, MiniTypeError, at
from .parser import parse
from .values import Builtin, Function, display, truthy


class _Return(Exception):
    def __init__(self, value):
        self.value = value


class _Break(Exception):
    pass


class _Continue(Exception):
    pass


class Scope:
    def __init__(self, parent=None, call=False):
        self.values, self.parent, self.call = {}, parent, call

    def declare(self, name, value, token):
        self.values[name] = value

    def lookup(self, name, token):
        scope = self
        while scope is not None:
            if name in scope.values:
                return scope.values[name]
            scope = scope.parent
        raise at(MiniNameError, token, f"undeclared name {name!r}")

    def owner(self, name, token):
        scope = self
        while scope is not None:
            if name in scope.values:
                return scope
            if scope.call:
                return scope
            scope = scope.parent
        raise at(MiniNameError, token, f"undeclared name {name!r}")


def run(source):
    output = []
    program = parse(source)
    _Interpreter(output).execute_block(program, Scope(_builtins(output)))
    return output


def _builtins(output):
    def print_(args, token):
        output.append(" ".join(display(arg) for arg in args))

    def push(args, token):
        args[0].append(args[1])

    scope = Scope()
    for name, implementation in (("print", print_), ("len", lambda args, token: len(args[0])),
                                 ("str", lambda args, token: display(args[0])), ("push", push)):
        scope.values[name] = Builtin(name, implementation)
    return scope


class _Interpreter:
    def __init__(self, output):
        self.output = output

    def execute_block(self, statements, scope):
        for statement in statements:
            self.execute(statement, scope)

    def execute(self, node, scope):
        kind = node.kind
        if kind == "expr":
            self.evaluate(node.value, scope)
        elif kind == "let":
            scope.declare(node.name, self.evaluate(node.value, scope), node.token)
        elif kind == "fndecl":
            function = node.function
            scope.declare(node.name, Function(node.name, function.params, function.body, scope), node.token)
        elif kind == "assign":
            self.assign(node.target, node.value, scope)
        elif kind == "if":
            for condition, body in node.branches:
                if truthy(self.evaluate(condition, scope)):
                    self.execute_block(body, Scope(scope))
                    return
            if node.otherwise is not None:
                self.execute_block(node.otherwise, Scope(scope))
        elif kind == "while":
            body_scope = Scope(scope)
            while truthy(self.evaluate(node.condition, scope)):
                try:
                    self.execute_block(node.body, body_scope)
                except _Break:
                    break
                except _Continue:
                    continue
        elif kind == "return":
            raise _Return(None if node.value is None else self.evaluate(node.value, scope))
        elif kind == "break":
            raise _Break()
        elif kind == "continue":
            raise _Continue()
        else:
            raise AssertionError(f"unknown statement {kind}")

    def assign(self, target, value_node, scope):
        if target.kind == "name":
            owner = scope.owner(target.name, target.token)
            owner.values[target.name] = self.evaluate(value_node, scope)
            return
        value = self.evaluate(value_node, scope)
        container = self.evaluate(target.target, scope)
        index = self.evaluate(target.index, scope)
        self.checked_index(container, index, target.token)
        container[index] = value

    def checked_index(self, container, index, token):
        if not isinstance(container, (list, str)):
            raise at(MiniTypeError, token, "value is not indexable")
        if not isinstance(index, int):
            raise at(MiniTypeError, token, "index must be an integer")
        if index >= len(container):
            raise at(MiniRuntimeError, token, "index out of range")

    def evaluate(self, node, scope):
        kind = node.kind
        if kind == "literal":
            return node.value
        if kind == "name":
            return scope.lookup(node.name, node.token)
        if kind == "list":
            return [self.evaluate(item, scope) for item in node.items]
        if kind == "function":
            return Function(None, node.params, node.body, scope)
        if kind == "not":
            return not truthy(self.evaluate(node.operand, scope))
        if kind == "negate":
            operand = self.evaluate(node.operand, scope)
            if not isinstance(operand, int):
                raise at(MiniTypeError, node.operand.token, "unary '-' needs an integer")
            return -operand
        if kind == "logical":
            left = truthy(self.evaluate(node.left, scope))
            if left == (node.op == "or"):
                return left
            return truthy(self.evaluate(node.right, scope))
        if kind == "binary":
            left = self.evaluate(node.left, scope)
            return _binary(node.op, left, self.evaluate(node.right, scope), node.left.token)
        if kind == "index":
            container = self.evaluate(node.target, scope)
            index = self.evaluate(node.index, scope)
            self.checked_index(container, index, node.token)
            return container[index]
        if kind == "call":
            callee = self.evaluate(node.callee, scope)
            args = [self.evaluate(arg, scope) for arg in node.args]
            return self.call(callee, args, node.token)
        raise AssertionError(f"unknown expression {kind}")

    def call(self, callee, args, token):
        if isinstance(callee, Builtin):
            return callee.implementation(args, token)
        if not isinstance(callee, Function):
            raise at(MiniTypeError, token, "value is not callable")
        scope = Scope(callee.closure, call=True)
        for position, name in enumerate(callee.params):
            scope.values[name] = args[position] if position < len(args) else None
        try:
            self.execute_block(callee.body, scope)
        except _Return as result:
            return result.value
        return None


def _binary(op, left, right, token):
    if op == "==":
        return left == right
    if op == "!=":
        return left != right
    if op in ("<", "<=", ">", ">="):
        if not ((isinstance(left, int) and isinstance(right, int))
                or (isinstance(left, str) and isinstance(right, str))):
            raise at(MiniTypeError, token, f"cannot compare with {op!r}")
        return {"<": left < right, "<=": left <= right, ">": left > right, ">=": left >= right}[op]
    if op == "+" and type(left) is type(right) and isinstance(left, (str, list)):
        return left + right
    if not (isinstance(left, int) and isinstance(right, int)):
        raise at(MiniTypeError, token, f"unsupported operand types for {op!r}")
    if op == "+":
        return left + right
    if op == "-":
        return left - right
    if op == "*":
        return left * right
    if op in ("/", "%"):
        if right == 0:
            raise at(MiniRuntimeError, token, "division by zero")
        return left // right if op == "/" else left % right
    return left ** right
