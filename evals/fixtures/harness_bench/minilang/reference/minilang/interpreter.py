import sys

from .errors import MiniError, MiniNameError, MiniRuntimeError, MiniTypeError, at
from .parser import parse
from .values import Builtin, Function, display, equal, is_int, truthy

MAX_CALL_DEPTH = 100
_RECURSION_LIMIT = 20000


class _Return(Exception):
    def __init__(self, value):
        self.value = value


class _Break(Exception):
    pass


class _Continue(Exception):
    pass


class Scope:
    def __init__(self, parent=None):
        self.values, self.parent = {}, parent

    def declare(self, name, value, token):
        if name in self.values:
            raise at(MiniNameError, token, f"{name!r} is already declared in this scope")
        self.values[name] = value

    def _owner(self, name, token):
        scope = self
        while scope is not None:
            if name in scope.values:
                return scope
            scope = scope.parent
        raise at(MiniNameError, token, f"undeclared name {name!r}")

    def lookup(self, name, token):
        return self._owner(name, token).values[name]

    def assign(self, name, value, token):
        self._owner(name, token).values[name] = value


def run(source):
    output = []
    previous_limit = sys.getrecursionlimit()
    sys.setrecursionlimit(max(previous_limit, _RECURSION_LIMIT))
    try:
        program = parse(source)
        _Interpreter(output).execute_block(program, Scope(_builtins(output)))
    except MiniError as error:
        error.output = list(output)
        raise
    finally:
        sys.setrecursionlimit(previous_limit)
    return output


def _builtins(output):
    def print_(args, token):
        output.append(" ".join(display(arg) for arg in args))

    def len_(args, token):
        if not isinstance(args[0], (str, list)):
            raise at(MiniTypeError, token, "len() needs a string or list")
        return len(args[0])

    def push(args, token):
        if not isinstance(args[0], list):
            raise at(MiniTypeError, token, "push() needs a list")
        args[0].append(args[1])

    scope = Scope()
    for name, arity, implementation in (("print", None, print_), ("len", 1, len_),
                                        ("str", 1, lambda args, token: display(args[0])), ("push", 2, push)):
        scope.values[name] = Builtin(name, arity, implementation)
    return scope


class _Interpreter:
    def __init__(self, output):
        self.output, self.depth = output, 0

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
            while truthy(self.evaluate(node.condition, scope)):
                try:
                    self.execute_block(node.body, Scope(scope))
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
            scope.assign(target.name, self.evaluate(value_node, scope), target.token)
            return
        container = self.evaluate(target.target, scope)
        index = self.evaluate(target.index, scope)
        value = self.evaluate(value_node, scope)
        if not isinstance(container, list):
            raise at(MiniTypeError, target.token, "only lists support index assignment")
        container[self.checked_index(container, index, target.token)] = value

    def checked_index(self, container, index, token):
        if not is_int(index):
            raise at(MiniTypeError, token, "index must be an integer")
        if not 0 <= index < len(container):
            raise at(MiniRuntimeError, token, "index out of range")
        return index

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
            if not is_int(operand):
                raise at(MiniTypeError, node.token, "unary '-' needs an integer")
            return -operand
        if kind == "logical":
            left = self.evaluate(node.left, scope)
            if truthy(left) == (node.op == "or"):
                return left
            return self.evaluate(node.right, scope)
        if kind == "binary":
            left = self.evaluate(node.left, scope)
            return _binary(node.op, left, self.evaluate(node.right, scope), node.token)
        if kind == "index":
            container = self.evaluate(node.target, scope)
            index = self.evaluate(node.index, scope)
            if not isinstance(container, (list, str)):
                raise at(MiniTypeError, node.token, "value is not indexable")
            return container[self.checked_index(container, index, node.token)]
        if kind == "call":
            callee = self.evaluate(node.callee, scope)
            args = [self.evaluate(arg, scope) for arg in node.args]
            return self.call(callee, args, node.token)
        raise AssertionError(f"unknown expression {kind}")

    def call(self, callee, args, token):
        if isinstance(callee, Builtin):
            if callee.arity is not None and len(args) != callee.arity:
                raise at(MiniTypeError, token, f"{callee.name}() takes {callee.arity} argument(s)")
            return callee.implementation(args, token)
        if not isinstance(callee, Function):
            raise at(MiniTypeError, token, "value is not callable")
        if len(args) != len(callee.params):
            raise at(MiniTypeError, token, f"expected {len(callee.params)} argument(s), got {len(args)}")
        if self.depth >= MAX_CALL_DEPTH:
            raise at(MiniRuntimeError, token, "stack overflow")
        scope = Scope(callee.closure)
        for name, value in zip(callee.params, args):
            scope.values[name] = value
        self.depth += 1
        try:
            self.execute_block(callee.body, scope)
        except _Return as result:
            return result.value
        finally:
            self.depth -= 1
        return None


def _binary(op, left, right, token):
    if op == "==":
        return equal(left, right)
    if op == "!=":
        return not equal(left, right)
    if op in ("<", "<=", ">", ">="):
        if not ((is_int(left) and is_int(right)) or (isinstance(left, str) and isinstance(right, str))):
            raise at(MiniTypeError, token, f"cannot compare with {op!r}")
        return {"<": left < right, "<=": left <= right, ">": left > right, ">=": left >= right}[op]
    if op == "+" and type(left) is type(right) and isinstance(left, (str, list)):
        return left + right
    if not (is_int(left) and is_int(right)):
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
        quotient = abs(left) // abs(right)
        quotient = quotient if (left < 0) == (right < 0) else -quotient
        return quotient if op == "/" else left - right * quotient
    if right < 0:
        raise at(MiniRuntimeError, token, "negative exponent")
    return left ** right
