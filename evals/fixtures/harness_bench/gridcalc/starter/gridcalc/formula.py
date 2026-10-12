import re

from .errors import CellError, FormulaError
from .refs import parse_cell

TOKEN = re.compile(r"\s*([0-9]+(?:\.[0-9]+)?|[A-Za-z]+[0-9]*(?::[A-Za-z]+[0-9]+)?|[-+*/(),])")


def tokenize(text):
    tokens, pos = [], 0
    while pos < len(text):
        match = TOKEN.match(text, pos)
        if match is None:
            raise FormulaError(f"unexpected text at {pos}")
        tokens.append(match[1])
        pos = match.end()
    return tokens


class Evaluator:
    def __init__(self, tokens, lookup):
        self.tokens = tokens
        self.lookup = lookup
        self.pos = 0

    def peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else None

    def expr(self):
        value = self.term()
        while self.peek() in ("+", "-"):
            operator = self.tokens[self.pos]
            self.pos += 1
            other = self.term()
            value = value + other if operator == "+" else value - other
        return value

    def term(self):
        value = self.primary()
        while self.peek() in ("*", "/"):
            operator = self.tokens[self.pos]
            self.pos += 1
            other = self.primary()
            if operator == "/" and other == 0:
                return CellError("#DIV/0!")
            value = value * other if operator == "*" else value / other
        return value

    def primary(self):
        token = self.peek()
        if token is None:
            raise FormulaError("missing operand")
        self.pos += 1
        if token == "(":
            value = self.expr()
            self.pos += 1
            return value
        if token[0].isdigit():
            return float(token) if "." in token else int(token)
        if token.upper() == "SUM":
            return self.total()
        return self.lookup(*parse_cell(token)) or 0

    def total(self):
        self.pos += 1
        first, _, last = self.tokens[self.pos].partition(":")
        self.pos += 2
        top, left = parse_cell(first)
        bottom, right = parse_cell(last or first)
        cells = [self.lookup(row, col) for row in range(top, bottom + 1) for col in range(left, right + 1)]
        return sum(cell for cell in cells if isinstance(cell, (int, float)))


def evaluate(text, lookup):
    return Evaluator(tokenize(text), lookup).expr()
