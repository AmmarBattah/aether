"""Server: framing, protocol dispatch, journal-before-acknowledge, compaction.

See ../CONTRACT.md.
"""
from .errors import FramingError, ProtocolError
from .framing import FrameDecoder, encode_frame
from .protocol import (MUTATION_OPS, encode_request, encode_response,
                       is_integer_text, parse_request)
from .store import Store, replay
from .wal import WriteAheadLog


class Server:
    """A single connection's worth of protocol handling over one log."""

    def __init__(self, wal_path, *, wal=None):
        self._wal = wal if wal is not None else WriteAheadLog(wal_path)
        self._store = Store()
        self._decoder = FrameDecoder()
        self._next_seq = 1

    @classmethod
    def recover(cls, wal_path, *, wal=None):
        """Rebuild a server from an existing log, appending nothing to it."""
        server = cls(wal_path, wal=wal)
        server._restore()
        return server

    @property
    def next_seq(self):
        return self._next_seq

    def applied_seq(self):
        return self._store.applied_seq()

    def get(self, key):
        return self._store.get(key)

    def keys(self, prefix=""):
        return self._store.keys(prefix)

    def snapshot(self):
        return self._store.snapshot()

    def handle_request(self, payload):
        """Handle one decoded payload and return one response payload."""
        assert isinstance(payload, str), f"payload must be str, got {type(payload).__name__}"
        try:
            request = parse_request(payload)
        except ProtocolError as exc:
            return encode_response("ERR", "protocol", str(exc))
        if request.op in MUTATION_OPS:
            return self._handle_mutation(request)
        return self._handle_read(request)

    def handle_bytes(self, chunk):
        """Frame-level entry point: bytes in, framed response bytes out.

        Every complete frame decoded before a malformed one is still served;
        the malformed frame yields one `ERR framing` response and leaves the
        store untouched.
        """
        responses = []
        try:
            self._decoder.feed(chunk)
            payload = self._decoder.next_frame()
            while payload is not None:
                responses.append(encode_frame(self.handle_request(payload)))
                payload = self._decoder.next_frame()
        except FramingError as exc:
            responses.append(encode_frame(encode_response("ERR", "framing", str(exc))))
        return b"".join(responses)

    def compact(self):
        """Rewrite the log to the smallest record set with the same state."""
        before_state = self._store.snapshot()
        before_last_seq = self._wal.last_seq()
        records = compaction_records(self._wal.records())
        self._wal.replace_with(records)
        assert self._store.snapshot() == before_state, "compaction must preserve observable state"
        assert self._wal.last_seq() == before_last_seq, "compaction must preserve the highest seq"
        return len(records)

    def close(self):
        """Idempotent, see WriteAheadLog.close()."""
        self._wal.close()

    def _restore(self):
        records = self._wal.records()
        for seq, payload in records:
            self._store.apply(seq, payload)
        self._next_seq = (records[-1][0] if records else 0) + 1
        assert self._next_seq > self._store.applied_seq(), "the next seq must lead applied state"

    def _handle_read(self, request):
        op, args = request.op, request.args
        if op == "PING":
            return encode_response("PONG")
        if op == "GET":
            value = self._store.get(args[0])
            if value is None:
                return encode_response("ERR", "notfound", args[0])
            return encode_response("VAL", value)
        assert op == "LIST", f"unhandled read op: {op!r}"
        return encode_response("KEYS", *self._store.keys(args[0]))

    def _handle_mutation(self, request):
        rejection = self._rejection_for(request)
        if rejection is not None:
            return rejection
        payload = encode_request(request.op, *request.args)
        seq = self._next_seq
        self._wal.append(seq, payload)
        applied = self._store.apply(seq, payload)
        assert applied, f"record {seq} must be new to this store"
        self._next_seq = seq + 1
        return encode_response("OK", str(seq))

    def _rejection_for(self, request):
        """The ERR response for a mutation that must not be journaled, else None."""
        op, key = request.op, request.args[0]
        assert op in MUTATION_OPS, f"{op} is not a mutation"
        if op == "SET":
            return None
        current = self._store.get(key)
        if current is None:
            return encode_response("ERR", "notfound", key)
        if op == "INCR" and not is_integer_text(current):
            return encode_response("ERR", "badvalue", key)
        return None


def compaction_records(records):
    """The compacted form of `records`: one SET per live key, at the seq of
    the last record that touched it, plus the log's final record when it is
    the only carrier of the highest sequence number."""
    store = Store()
    last_touch = {}
    for seq, payload in records:
        request = parse_request(payload)
        store.apply(seq, payload)
        last_touch[request.args[0]] = seq
    live = store.snapshot()
    compacted = sorted((last_touch[key], encode_request("SET", key, value))
                       for key, value in live.items())
    if records and (not compacted or compacted[-1][0] < records[-1][0]):
        compacted.append(records[-1])
    assert replay(compacted).snapshot() == live, "compaction must preserve observable state"
    assert len(compacted) <= max(len(records), 1), "compaction must never grow the log"
    return compacted
