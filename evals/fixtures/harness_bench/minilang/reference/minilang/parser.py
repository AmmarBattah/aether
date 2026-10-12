from .errors import MiniSyntaxError, at
from .lexer import tokenize
from .nodes import Node

COMPARISONS = ("==", "!=", "<", "<=", ">", ">=")


def parse(source):
    return _Parser(tokenize(source)).program()


class _Parser:
    def __init__(self, tokens):
        self.tokens, self.index = tokens, 0
        self.in_function, self.in_loop = False, False

    def peek(self, offset=0):
        return self.tokens[min(self.index + offset, len(self.tokens) - 1)]

    def advance(self):
        token = self.peek()
        self.index += 1
        return token

    def at_op(self, *values):
        token = self.peek()
        return token.kind == "op" and token.value in values

    def at_kw(self, *values):
        token = self.peek()
        return token.kind == "kw" and token.value in values

    def expect_op(self, value):
        if not self.at_op(value):
            raise self.unexpected(f"expected {value!r}")
        return self.advance()

    def expect_ident(self):
        if self.peek().kind != "ident":
            raise self.unexpected("expected an identifier")
        return self.advance()

    def unexpected(self, message):
        return at(MiniSyntaxError, self.peek(), message)

    def program(self):
        statements = []
        while self.peek().kind != "eof":
            statements.append(self.statement())
        return statements

    def block(self):
        self.expect_op("{")
        statements = []
        while not self.at_op("}"):
            if self.peek().kind == "eof":
                raise self.unexpected("expected '}'")
            statements.append(self.statement())
        self.advance()
        return statements

    def statement(self):
        token = self.peek()
        if self.at_kw("let"):
            self.advance()
            name = self.expect_ident()
            self.expect_op("=")
            value = self.expression()
            self.expect_op(";")
            return Node("let", name, name=name.value, value=value)
        if self.at_kw("fn") and self.peek(1).kind == "ident":
            self.advance()
            name = self.advance()
            return Node("fndecl", name, name=name.value, function=self.function_rest(name.value, token))
        if self.at_kw("if"):
            return self.if_statement()
        if self.at_kw("while"):
            self.advance()
            condition = self.expression()
            outer, self.in_loop = self.in_loop, True
            try:
                body = self.block()
            finally:
                self.in_loop = outer
            return Node("while", token, condition=condition, body=body)
        if self.at_kw("break", "continue"):
            if not self.in_loop:
                raise at(MiniSyntaxError, token, f"{token.value} outside a loop")
            self.advance()
            self.expect_op(";")
            return Node(token.value, token)
        if self.at_kw("return"):
            if not self.in_function:
                raise at(MiniSyntaxError, token, "return outside a function")
            self.advance()
            value = None if self.at_op(";") else self.expression()
            self.expect_op(";")
            return Node("return", token, value=value)
        expression = self.expression()
        if self.at_op("="):
            equals = self.advance()
            if expression.kind not in ("name", "index"):
                raise at(MiniSyntaxError, equals, "invalid assignment target")
            value = self.expression()
            self.expect_op(";")
            return Node("assign", equals, target=expression, value=value)
        self.expect_op(";")
        return Node("expr", token, value=expression)

    def if_statement(self):
        token = self.advance()
        branches = [(self.expression(), self.block())]
        otherwise = None
        while self.at_kw("elif"):
            self.advance()
            branches.append((self.expression(), self.block()))
        if self.at_kw("else"):
            self.advance()
            otherwise = self.block()
        return Node("if", token, branches=branches, otherwise=otherwise)

    def function_rest(self, name, token):
        self.expect_op("(")
        params, seen = [], set()
        if not self.at_op(")"):
            while True:
                param = self.expect_ident()
                if param.value in seen:
                    raise at(MiniSyntaxError, param, "duplicate parameter")
                seen.add(param.value)
                params.append(param.value)
                if not self.at_op(","):
                    break
                self.advance()
        self.expect_op(")")
        outer = (self.in_function, self.in_loop)
        self.in_function, self.in_loop = True, False
        try:
            body = self.block()
        finally:
            self.in_function, self.in_loop = outer
        return Node("function", token, name=name, params=params, body=body)

    def expression(self):
        return self.or_expression()

    def or_expression(self):
        left = self.and_expression()
        while self.at_kw("or"):
            token = self.advance()
            left = Node("logical", token, op="or", left=left, right=self.and_expression())
        return left

    def and_expression(self):
        left = self.not_expression()
        while self.at_kw("and"):
            token = self.advance()
            left = Node("logical", token, op="and", left=left, right=self.not_expression())
        return left

    def not_expression(self):
        if self.at_kw("not"):
            token = self.advance()
            return Node("not", token, operand=self.not_expression())
        return self.comparison()

    def comparison(self):
        left = self.additive()
        if self.at_op(*COMPARISONS):
            token = self.advance()
            left = Node("binary", token, op=token.value, left=left, right=self.additive())
            if self.at_op(*COMPARISONS):
                raise self.unexpected("comparison operators cannot be chained")
        return left

    def additive(self):
        left = self.multiplicative()
        while self.at_op("+", "-"):
            token = self.advance()
            left = Node("binary", token, op=token.value, left=left, right=self.multiplicative())
        return left

    def multiplicative(self):
        left = self.unary()
        while self.at_op("*", "/", "%"):
            token = self.advance()
            left = Node("binary", token, op=token.value, left=left, right=self.unary())
        return left

    def unary(self):
        if self.at_op("-"):
            token = self.advance()
            return Node("negate", token, operand=self.unary())
        return self.power()

    def power(self):
        base = self.postfix()
        if self.at_op("**"):
            token = self.advance()
            return Node("binary", token, op="**", left=base, right=self.unary())
        return base

    def postfix(self):
        expression = self.primary()
        while True:
            if self.at_op("("):
                token = self.advance()
                expression = Node("call", token, callee=expression, args=self.arguments(")"))
            elif self.at_op("["):
                token = self.advance()
                index = self.expression()
                self.expect_op("]")
                expression = Node("index", token, target=expression, index=index)
            else:
                return expression

    def arguments(self, closer):
        items = []
        if not self.at_op(closer):
            while True:
                items.append(self.expression())
                if not self.at_op(","):
                    break
                self.advance()
        self.expect_op(closer)
        return items

    def primary(self):
        token = self.peek()
        if token.kind in ("int", "str"):
            self.advance()
            return Node("literal", token, value=token.value)
        if token.kind == "ident":
            self.advance()
            return Node("name", token, name=token.value)
        if self.at_kw("true", "false", "nil"):
            self.advance()
            return Node("literal", token, value={"true": True, "false": False, "nil": None}[token.value])
        if self.at_kw("fn"):
            self.advance()
            return self.function_rest(None, token)
        if self.at_op("("):
            self.advance()
            expression = self.expression()
            self.expect_op(")")
            return expression
        if self.at_op("["):
            self.advance()
            return Node("list", token, items=self.arguments("]"))
        raise self.unexpected("expected an expression")
