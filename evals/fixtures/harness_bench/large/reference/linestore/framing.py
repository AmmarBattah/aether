"""Byte framing for the line protocol. See ../CONTRACT.md."""
from .errors import FramingError

MAX_PAYLOAD_BYTES = 4096
MAX_HEADER_DIGITS = 8
_SEPARATOR = 0x7C  # "|"
_TERMINATOR = 0x0A  # "\n"


def encode_frame(payload):
    """Encode one payload string as `<utf8-byte-length>|<payload>\\n` bytes."""
    assert isinstance(payload, str), f"payload must be str, got {type(payload).__name__}"
    body = payload.encode("utf-8")
    if _TERMINATOR in body:
        raise FramingError("payload must not contain a newline")
    if len(body) > MAX_PAYLOAD_BYTES:
        raise FramingError(f"payload of {len(body)} bytes exceeds {MAX_PAYLOAD_BYTES}")
    header = str(len(body)).encode("ascii")
    assert len(header) <= MAX_HEADER_DIGITS, "payload limit must fit the header digit budget"
    return header + b"|" + body + b"\n"


class FrameDecoder:
    """Incremental frame decoder over an untrusted byte stream.

    `feed()` only buffers; `next_frame()` pops one complete payload. A
    malformed frame poisons the decoder permanently: a stream whose framing
    is broken cannot be resynchronised, so every later call raises.
    """

    def __init__(self):
        self._buffer = bytearray()
        self._poison_reason = None

    def is_poisoned(self):
        return self._poison_reason is not None

    def pending_bytes(self):
        return len(self._buffer)

    def feed(self, chunk):
        self._reject_if_poisoned()
        if not isinstance(chunk, (bytes, bytearray)):
            raise FramingError(f"chunk must be bytes, got {type(chunk).__name__}")
        self._buffer.extend(chunk)

    def next_frame(self):
        """Next complete payload, or None while the buffer holds a partial frame."""
        self._reject_if_poisoned()
        header = self._peek_header()
        if header is None:
            return None
        length, body_start = header
        end = body_start + length
        if len(self._buffer) <= end:
            return None
        if self._buffer[end] != _TERMINATOR:
            self._poison(f"frame of {length} bytes is not newline terminated")
        body = bytes(self._buffer[body_start:end])
        del self._buffer[:end + 1]
        return self._decode_body(body)

    def _peek_header(self):
        """`(payload_length, body_start)` once the header is complete, else None."""
        for index, byte in enumerate(self._buffer):
            if 0x30 <= byte <= 0x39:
                continue
            if byte != _SEPARATOR:
                self._poison(f"frame header byte {index} is not a length digit")
            if index == 0:
                self._poison("frame header carries no length digits")
            length = int(self._buffer[:index])
            if length > MAX_PAYLOAD_BYTES:
                self._poison(f"declared payload of {length} bytes exceeds {MAX_PAYLOAD_BYTES}")
            return length, index + 1
        if len(self._buffer) > MAX_HEADER_DIGITS:
            self._poison("frame header has too many length digits")
        return None

    def _decode_body(self, body):
        if _TERMINATOR in body:
            self._poison("payload contains a newline")
        try:
            return body.decode("utf-8")
        except UnicodeDecodeError as exc:
            self._poison(f"payload is not valid utf-8: {exc}")

    def _poison(self, reason):
        assert reason, "a poison reason must explain why the stream is unusable"
        self._poison_reason = reason
        self._buffer.clear()
        raise FramingError(reason)

    def _reject_if_poisoned(self):
        if self._poison_reason is not None:
            raise FramingError(f"decoder is unusable: {self._poison_reason}")
