import hashlib
import json
import math

from .errors import Corrupt, Invalid


def valid_name(value, empty=False):
    if not isinstance(value, str) or (not empty and not value):
        raise Invalid("expected string" if empty else "expected nonempty string")
    try:
        value.encode("utf-8")
    except UnicodeError as error:
        raise Invalid("string cannot be encoded") from error


def validate_value(value, active=None):
    active = set() if active is None else active
    if type(value) in (type(None), bool, int, str):
        return
    if type(value) is float:
        if not math.isfinite(value):
            raise Invalid("non-finite number")
        return
    if type(value) not in (list, dict) or id(value) in active:
        raise Invalid("unsupported or cyclic value")
    active.add(id(value))
    try:
        if isinstance(value, dict):
            if any(not isinstance(key, str) for key in value):
                raise Invalid("object keys must be strings")
            entries = value.values()
        else:
            entries = value
        for item in entries:
            validate_value(item, active)
    finally:
        active.remove(id(value))


def canonical(value):
    validate_value(value)
    try:
        return json.dumps(value, sort_keys=True, separators=(",", ":"),
                          ensure_ascii=False, allow_nan=False).encode("utf-8")
    except (ValueError, UnicodeError) as error:
        raise Invalid("value cannot be encoded") from error


def clone(value):
    return json.loads(canonical(value))


def validate_receipt(receipt):
    if type(receipt) is not dict or set(receipt) != {"txid", "revision", "writes"}:
        raise Invalid("receipt fields")
    valid_name(receipt["txid"])
    if type(receipt["revision"]) is not int or receipt["revision"] < 1:
        raise Invalid("revision")
    if type(receipt["writes"]) is not list:
        raise Invalid("writes")
    previous = None
    for operation in receipt["writes"]:
        if type(operation) is not list or not operation:
            raise Invalid("operation")
        if (operation[0] == "put" and len(operation) == 3):
            validate_value(operation[2])
        elif not (operation[0] == "delete" and len(operation) == 2):
            raise Invalid("operation shape")
        valid_name(operation[1])
        if previous is not None and operation[1] <= previous:
            raise Invalid("write order")
        previous = operation[1]


def encode(receipt):
    validate_receipt(receipt)
    payload = canonical(receipt)
    return (f"{len(payload):08x}:".encode() + payload + b":"
            + hashlib.sha256(payload).hexdigest().encode() + b"\n")


def decode(data):
    receipts = []
    offset = 0
    hex_bytes = b"0123456789abcdef"
    while offset < len(data):
        tail = data[offset:]
        if any(byte not in hex_bytes for byte in tail[:8]):
            raise Corrupt("length header")
        if len(tail) < 9:
            break
        if tail[8:9] != b":":
            raise Corrupt("header delimiter")
        size = int(tail[:8], 16)
        end = 9 + size
        if len(tail) < end:
            break
        payload = tail[9:end]
        try:
            receipt = json.loads(payload.decode("utf-8"))
            validate_receipt(receipt)
            if canonical(receipt) != payload:
                raise Invalid("noncanonical payload")
        except (ValueError, UnicodeError, Invalid) as error:
            raise Corrupt("invalid receipt payload") from error
        trailer = tail[end:]
        if trailer and trailer[:1] != b":":
            raise Corrupt("payload delimiter")
        digest = trailer[1:65]
        if any(byte not in hex_bytes for byte in digest):
            raise Corrupt("checksum syntax")
        if len(digest) == 64 and digest != hashlib.sha256(payload).hexdigest().encode():
            raise Corrupt("checksum mismatch")
        if len(trailer) < 66:
            break
        if trailer[65:66] != b"\n":
            raise Corrupt("terminator")
        receipts.append(receipt)
        offset += end + 66
    return receipts
