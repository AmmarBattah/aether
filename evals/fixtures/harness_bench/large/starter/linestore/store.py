"""Key/value state machine fed by journal records. See ../CONTRACT.md."""
from .errors import ProtocolError, StorageError
from .protocol import MUTATION_OPS, is_integer_text, parse_request


class Store:
    """Applies journaled mutations. Idempotent by record sequence number."""

    def __init__(self):
        self._data = {}
        self._applied_seq = 0

    def applied_seq(self):
        return self._applied_seq

    def get(self, key):
        return self._data.get(key)

    def keys(self, prefix=""):
        assert isinstance(prefix, str), f"prefix must be str, got {type(prefix).__name__}"
        return sorted(key for key in self._data if key.startswith(prefix))

    def snapshot(self):
        return dict(self._data)

    def apply(self, seq, payload):
        """Apply one journaled mutation; True when it advanced this store.

        Records at or below `applied_seq` are ignored, which is what makes
        replaying a log (or replaying it twice) idempotent. A record whose
        precondition no longer holds is a no-op rather than an error.
        """
        assert isinstance(seq, int) and seq > 0, f"seq must be a positive int, got {seq!r}"
        request = _parse_record(payload)
        if request.op not in MUTATION_OPS:
            raise StorageError(f"{request.op} is not a mutation and cannot be journaled")
        self._mutate(request.op, request.args)
        self._applied_seq = seq
        assert self._applied_seq == seq, "applied_seq must track the last applied record"
        return True

    def _mutate(self, op, args):
        assert op in MUTATION_OPS, f"{op} is not a mutation"
        if op == "SET":
            self._data[args[0]] = args[1]
        elif op == "DEL":
            self._data.pop(args[0], None)
        else:
            self._increment(args[0], int(args[1]))

    def _increment(self, key, delta):
        current = self._data.get(key)
        if current is None or not is_integer_text(current):
            return
        self._data[key] = str(int(current) + delta)


def replay(records):
    """Build a fresh Store from `records`, applying each one in order."""
    store = Store()
    for seq, payload in records:
        store.apply(seq, payload)
    assert not records or store.applied_seq() == records[-1][0], "replay must reach the last seq"
    return store


def _parse_record(payload):
    try:
        return parse_request(payload)
    except ProtocolError as exc:
        raise StorageError(f"journal record is not a valid request: {payload!r}") from exc
