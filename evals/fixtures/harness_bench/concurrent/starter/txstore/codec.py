"""Framed journal receipts."""
import hashlib
import json


def encode(receipt):
    payload = json.dumps(receipt, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=False, allow_nan=False).encode()
    return f"{len(payload):08x}:".encode() + payload + b":" + hashlib.sha256(payload).hexdigest().encode() + b"\n"


def decode(data):
    receipts = []
    for line in data.splitlines():
        try:
            size = int(line[:8], 16)
            receipts.append(json.loads(line[9:9 + size]))
        except (ValueError, UnicodeError):
            break
    return receipts
