from collections import namedtuple

from .errors import MiniSyntaxError

Token = namedtuple("Token", "kind value line column")

KEYWORDS = {"let", "fn", "if", "elif", "else", "while", "break", "continue", "return",
            "true", "false", "nil", "and", "or", "not"}
_OPERATORS = ("==", "!=", "<=", ">=", "**", "<", ">", "+", "-", "*", "/", "%", "=",
              "(", ")", "[", "]", "{", "}", ",", ";")
_ESCAPES = {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}


def tokenize(source):
    tokens, index, line, line_start = [], 0, 1, 0
    while index < len(source):
        char = source[index]
        column = index - line_start + 1
        if char == "\n":
            index, line, line_start = index + 1, line + 1, index + 1
        elif char.isspace():
            index += 1
        elif char == "#":
            while index < len(source) and source[index] != "\n":
                index += 1
        elif char.isdigit():
            end = index
            while end < len(source) and source[end].isdigit():
                end += 1
            tokens.append(Token("int", int(source[index:end]), line, column))
            index = end
        elif char.isalpha() or char == "_":
            end = index
            while end < len(source) and (source[end].isalnum() or source[end] == "_"):
                end += 1
            word = source[index:end]
            tokens.append(Token("kw" if word in KEYWORDS else "ident", word, line, column))
            index = end
        elif char == '"':
            value, index = _string(source, index, line, line_start)
            tokens.append(Token("str", value, line, column))
        else:
            operator = next((op for op in _OPERATORS if source.startswith(op, index)), None)
            if operator is None:
                raise MiniSyntaxError(f"unexpected character {char!r}", line, column)
            tokens.append(Token("op", operator, line, column))
            index += len(operator)
    tokens.append(Token("eof", None, line, len(source) - line_start + 1))
    return tokens


def _string(source, start, line, line_start):
    chars, index = [], start + 1
    while index < len(source) and source[index] != "\n":
        char = source[index]
        if char == '"':
            return "".join(chars), index + 1
        if char == "\\" and index + 1 < len(source):
            escape = source[index + 1]
            chars.append(_ESCAPES.get(escape, escape))
            index += 2
        else:
            chars.append(char)
            index += 1
    raise MiniSyntaxError("unterminated string", line, index - line_start + 1)
