"""Request/response messages carried inside one frame. See ../CONTRACT.md."""
from collections import namedtuple
from urllib.parse import quote, unquote

from .errors import ProtocolError

REQUEST_ARITY = {"PING": 0, "GET": 1, "LIST": 1, "SET": 2, "DEL": 1, "INCR": 2}
MUTATION_OPS = frozenset({"SET", "DEL", "INCR"})
RESPONSE_ARITY = {"PONG": 0, "OK": 1, "VAL": 1, "ERR": 2, "KEYS": None}
ERROR_CODES = frozenset({"protocol", "framing", "notfound", "badvalue", "storage"})
MAX_KEY_BYTES = 64

Request = namedtuple("Request", ("op", "args"))
Response = namedtuple("Response", ("kind", "args"))


def encode_request(op, *args):
    """Encode one request as a payload string, percent-escaping every token."""
    assert op in REQUEST_ARITY, f"unknown op: {op!r}"
    assert len(args) == REQUEST_ARITY[op], f"{op} takes {REQUEST_ARITY[op]} argument(s)"
    return _encode_tokens(op, args)


def encode_response(kind, *args):
    """Encode one response as a payload string, percent-escaping every token."""
    assert kind in RESPONSE_ARITY, f"unknown response kind: {kind!r}"
    expected = RESPONSE_ARITY[kind]
    assert expected is None or len(args) == expected, f"{kind} takes {expected} argument(s)"
    return _encode_tokens(kind, [str(arg) for arg in args])


def parse_request(payload):
    """Parse a request payload, raising ProtocolError for anything invalid."""
    op, args = _split_payload(payload)
    if op not in REQUEST_ARITY:
        raise ProtocolError(f"unknown op: {op!r}")
    if len(args) != REQUEST_ARITY[op]:
        raise ProtocolError(f"{op} takes {REQUEST_ARITY[op]} argument(s), got {len(args)}")
    _validate_request_args(op, args)
    return Request(op, args)


def parse_response(payload):
    """Parse a response payload, raising ProtocolError for anything invalid."""
    kind, args = _split_payload(payload)
    if kind not in RESPONSE_ARITY:
        raise ProtocolError(f"unknown response kind: {kind!r}")
    expected = RESPONSE_ARITY[kind]
    if expected is not None and len(args) != expected:
        raise ProtocolError(f"{kind} takes {expected} argument(s), got {len(args)}")
    if kind == "OK" and not args[0].isdigit():
        raise ProtocolError(f"OK carries a sequence number, got {args[0]!r}")
    if kind == "ERR" and args[0] not in ERROR_CODES:
        raise ProtocolError(f"unknown error code: {args[0]!r}")
    return Response(kind, args)


def is_integer_text(text):
    """True when `text` is a decimal integer literal, optionally signed."""
    body = text[1:] if text.startswith("-") else text
    return bool(body) and body.isdigit()


def _encode_tokens(head, args):
    tokens = [head] + [quote(arg, safe="") for arg in args]
    payload = " ".join(tokens)
    assert "\n" not in payload, "an encoded payload must stay on one line"
    return payload


def _split_payload(payload):
    if not isinstance(payload, str) or not payload:
        raise ProtocolError("payload must be a non-empty string")
    tokens = [unquote(token) for token in payload.split()]
    assert tokens, "splitting a non-empty payload always yields a token"
    return tokens[0], tuple(tokens[1:])


def _validate_request_args(op, args):
    assert op in REQUEST_ARITY, f"unknown op: {op!r}"
    if op == "LIST" or op == "PING":
        return
    _validate_key(args[0])
    if op == "INCR" and not is_integer_text(args[1]):
        raise ProtocolError(f"INCR delta must be an integer, got {args[1]!r}")


def _validate_key(key):
    if not key:
        raise ProtocolError("key must not be empty")
    if len(key.encode("utf-8")) > MAX_KEY_BYTES:
        raise ProtocolError(f"key exceeds {MAX_KEY_BYTES} bytes: {key!r}")
    if any(character.isspace() or ord(character) < 0x20 for character in key):
        raise ProtocolError(f"key must not contain whitespace or control characters: {key!r}")
