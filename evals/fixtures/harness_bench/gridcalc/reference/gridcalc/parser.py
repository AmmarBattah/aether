from .errors import FormulaError
from .functions import ARITY
from .values import normalize

COMPARISONS = ("=", "<>", "<", "<=", ">", ">=")
MAX_DEPTH = 200


class _Parser:
    def __init__(self, tokens):
        self.tokens = tokens
        self.pos = 0
        self.depth = 0

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def take(self, kind, *values):
        token = self.peek()
        if token is None or token.kind != kind or (values and token.value not in values):
            return None
        self.pos += 1
        return token

    def left_associative(self, operators, operand):
        node = operand()
        while True:
            token = self.take("op", *operators)
            if token is None:
                return node
            node = ("binary", token.value, node, operand())

    def expr(self):
        self.depth += 1
        if self.depth > MAX_DEPTH:
            raise FormulaError("formula is nested too deeply")
        node = self.left_associative(COMPARISONS, self.concat)
        self.depth -= 1
        return node

    def concat(self):
        return self.left_associative(("&",), self.additive)

    def additive(self):
        return self.left_associative(("+", "-"), self.term)

    def term(self):
        return self.left_associative(("*", "/"), self.unary)

    def unary(self):
        token = self.take("op", "-", "+")
        if token is not None:
            return ("unary", token.value, self.unary())
        base = self.postfix()
        if self.take("op", "^") is None:
            return base
        return ("binary", "^", base, self.unary())

    def postfix(self):
        node = self.primary()
        while self.take("op", "%") is not None:
            node = ("percent", node)
        return node

    def primary(self):
        token = self.peek()
        if token is None:
            raise FormulaError("missing operand")
        self.pos += 1
        if token.kind == "num":
            return ("num", normalize(float(token.value)) if "." in token.value else int(token.value))
        if token.kind in ("str", "bool"):
            return (token.kind, token.value)
        if token.kind == "referr":
            return ("err", token.value)
        if token.kind == "ref":
            return _reference(*token.value)
        if token.kind == "func":
            return self.call(token.value)
        if token.kind == "lparen":
            node = self.expr()
            self.close()
            return node
        raise FormulaError(f"unexpected {token.value!r}")

    def close(self):
        if self.take("rparen") is None:
            raise FormulaError("missing )")

    def call(self, name):
        self.take("lparen")
        arguments = []
        if self.take("rparen") is None:
            arguments.append(self.expr())
            while self.take("comma") is not None:
                arguments.append(self.expr())
            self.close()
        low, high = ARITY.get(name, (0, None))
        if len(arguments) < low or (high is not None and len(arguments) > high):
            raise FormulaError(f"{name} takes a different number of arguments")
        return ("call", name, tuple(arguments))


def _reference(first, second):
    if second is None:
        return ("cell", first.row, first.col)
    return ("range", first.row, first.col, second.row, second.col)


def parse(tokens):
    """AST of one complete expression; trailing tokens are an error."""
    parser = _Parser(tokens)
    node = parser.expr()
    if parser.peek() is not None:
        raise FormulaError(f"unexpected {parser.peek().value!r} after the expression")
    assert parser.depth == 0, "unbalanced expression depth"
    return node


def references(node):
    """Every cell a node references statically, as (row, col), ranges expanded."""
    kind = node[0]
    if kind == "cell":
        return {(node[1], node[2])}
    if kind == "range":
        return {(row, col) for row in range(node[1], node[3] + 1) for col in range(node[2], node[4] + 1)}
    children = node[2] if kind == "call" else [part for part in node[1:] if isinstance(part, tuple)]
    found = set()
    for child in children:
        found |= references(child)
    return found
