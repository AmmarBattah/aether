from collections import namedtuple

from .errors import MiniSyntaxError

Token = namedtuple("Token", "kind value line column")

KEYWORDS = {"let", "fn", "if", "elif", "else", "while", "break", "continue", "return",
            "true", "false", "nil", "and", "or", "not"}
_OPERATORS = ("==", "!=", "<=", ">=", "**", "<", ">", "+", "-", "*", "/", "%", "=",
              "(", ")", "[", "]", "{", "}", ",", ";")
_ESCAPES = {"n": "\n", "t": "\t", '"': '"', "\\": "\\"}


def _is_ident_start(char):
    return char.isascii() and (char.isalpha() or char == "_")


def _is_ident_char(char):
    return char.isascii() and (char.isalnum() or char == "_")


def tokenize(source):
    tokens, index, line, line_start = [], 0, 1, 0
    while index < len(source):
        char = source[index]
        column = index - line_start + 1
        if char == "\n":
            index, line, line_start = index + 1, line + 1, index + 1
        elif char in " \t\r":
            index += 1
        elif char == "#":
            while index < len(source) and source[index] != "\n":
                index += 1
        elif "0" <= char <= "9":
            end = index
            while end < len(source) and "0" <= source[end] <= "9":
                end += 1
            text = source[index:end]
            if len(text) > 1 and text[0] == "0":
                raise MiniSyntaxError("integer literal has a leading zero", line, column)
            tokens.append(Token("int", int(text), line, column))
            index = end
        elif _is_ident_start(char):
            end = index
            while end < len(source) and _is_ident_char(source[end]):
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
        if char == "\\":
            escape = source[index + 1] if index + 1 < len(source) else ""
            if escape not in _ESCAPES:
                raise MiniSyntaxError("invalid escape sequence", line, index - line_start + 1)
            chars.append(_ESCAPES[escape])
            index += 2
        else:
            chars.append(char)
            index += 1
    raise MiniSyntaxError("unterminated string", line, start - line_start + 1)
