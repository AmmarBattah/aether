import re
from collections import namedtuple

from .errors import FormulaError
from .refs import Corner, column_number, order_corners

Token = namedtuple("Token", "kind value start end")
NUMBER = re.compile(r"[0-9]+(\.[0-9]+)?")
WORD = re.compile(r"[A-Za-z0-9_$]+")
NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
CORNER = re.compile(r"(\$?)([A-Za-z]{1,2})(\$?)([1-9][0-9]{0,3})")
OPERATORS = ("<>", "<=", ">=", "+", "-", "*", "/", "^", "&", "%", "=", "<", ">")
PUNCTUATION = {"(": "lparen", ")": "rparen", ",": "comma"}
REF_ERROR = "#REF!"


def _corner(word):
    match = CORNER.fullmatch(word)
    if match is None:
        return None
    return Corner(int(match[4]), column_number(match[2]), bool(match[3]), bool(match[1]))


def _string(text, pos):
    chars, cursor = [], pos + 1
    while cursor < len(text):
        if text[cursor] != '"':
            chars.append(text[cursor])
            cursor += 1
        elif text[cursor + 1:cursor + 2] == '"':
            chars.append('"')
            cursor += 2
        else:
            return Token("str", "".join(chars), pos, cursor + 1)
    raise FormulaError("unterminated string")


def _word(text, pos):
    end = WORD.match(text, pos).end()
    word = text[pos:end]
    if text[end:].lstrip(" ").startswith("("):
        if not NAME.fullmatch(word):
            raise FormulaError(f"bad function name {word!r}")
        return Token("func", word.upper(), pos, end)
    if word.upper() in ("TRUE", "FALSE"):
        return Token("bool", word.upper() == "TRUE", pos, end)
    first = _corner(word)
    if first is None:
        raise FormulaError(f"unknown name {word!r}")
    if text[end:end + 1] != ":":
        return Token("ref", (first, None), pos, end)
    tail = WORD.match(text, end + 1)
    second = _corner(tail.group()) if tail else None
    if second is None:
        raise FormulaError("a range needs two cells")
    return Token("ref", order_corners(first, second), pos, tail.end())


def _next_token(text, pos):
    char = text[pos]
    if char == '"':
        return _string(text, pos)
    if char.isascii() and char.isdigit():
        return Token("num", NUMBER.match(text, pos).group(), pos, NUMBER.match(text, pos).end())
    if char == "$" or char == "_" or (char.isascii() and char.isalpha()):
        return _word(text, pos)
    if text[pos:pos + len(REF_ERROR)].upper() == REF_ERROR:
        return Token("referr", REF_ERROR, pos, pos + len(REF_ERROR))
    if char in PUNCTUATION:
        return Token(PUNCTUATION[char], char, pos, pos + 1)
    for operator in OPERATORS:
        if text.startswith(operator, pos):
            return Token("op", operator, pos, pos + len(operator))
    raise FormulaError(f"unexpected character {char!r}")


def tokenize(text):
    """Tokens of the text after `=`, each with its exact span in that text."""
    assert isinstance(text, str), "formula text must be a string"
    tokens, pos = [], 0
    while pos < len(text):
        if text[pos] == " ":
            pos += 1
            continue
        token = _next_token(text, pos)
        assert token.start == pos and token.end > pos, "a token must consume input"
        tokens.append(token)
        pos = token.end
    return tokens
