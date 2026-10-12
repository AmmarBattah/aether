"""Write-ahead log persistence and compaction rewrite. See ../CONTRACT.md."""
import os
from pathlib import Path
from zlib import crc32

from .errors import StorageError

_CHECKSUM_DIGITS = 8


class WriteAheadLog:
    """Append-only log of `(seq, payload)` records, one text line each."""

    def __init__(self, path):
        self._path = Path(path)
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._handle = open(self._path, "a", encoding="utf-8")
        self._closed = False

    @property
    def path(self):
        return self._path

    def append(self, seq, payload):
        """Append one record and flush it so a separate reader can see it."""
        assert isinstance(seq, int) and seq > 0, f"seq must be a positive int, got {seq!r}"
        assert isinstance(payload, str) and "\n" not in payload, "payload must be one line"
        if self._closed:
            raise StorageError("cannot append to a closed write-ahead log")
        self._handle.write(encode_record(seq, payload))

    def records(self):
        """Every complete, checksum-valid record on disk, in write order."""
        if not self._path.exists():
            return []
        text = self._path.read_text(encoding="utf-8")
        lines = text.splitlines()
        records = [_decode_record(index, line) for index, line in enumerate(lines)]
        require_increasing(records)
        return records

    def last_seq(self):
        records = self.records()
        return records[-1][0] if records else 0

    def replace_with(self, records):
        """Atomically replace the whole log with `records`. Returns its length."""
        require_increasing(records)
        if self._closed:
            raise StorageError("cannot rewrite a closed write-ahead log")
        temp_path = self._path.with_name(self._path.name + ".compact")
        body = "".join(encode_record(seq, payload) for seq, payload in records)
        temp_path.write_text(body, encoding="utf-8")
        self._swap_in(temp_path)
        assert not temp_path.exists(), "the compaction temp file must not survive the swap"
        assert len(self.records()) == len(records), "every compacted record must land on disk"
        return len(records)

    def close(self):
        """Flush and close. Idempotent."""
        if self._closed:
            return
        self._handle.flush()
        self._handle.close()
        self._closed = True

    def _swap_in(self, temp_path):
        assert temp_path.exists(), "the compaction temp file must exist before the swap"
        self._handle.close()
        os.replace(temp_path, self._path)
        self._handle = open(self._path, "a", encoding="utf-8")


def encode_record(seq, payload):
    """Serialize one record as `<seq> <crc32> <payload>\\n`."""
    assert isinstance(seq, int) and seq > 0, f"seq must be a positive int, got {seq!r}"
    checksum = crc32(payload.encode("utf-8"))
    return f"{seq} {checksum:0{_CHECKSUM_DIGITS}x} {payload}\n"


def require_increasing(records):
    """Raise StorageError unless `records` has strictly increasing seqs."""
    previous = 0
    for seq, payload in records:
        if not isinstance(seq, int) or seq <= previous:
            raise StorageError(f"record seq {seq!r} does not follow {previous}")
        if not isinstance(payload, str) or "\n" in payload:
            raise StorageError(f"record {seq} payload is not a single line")
        previous = seq


def _decode_record(index, line):
    if not line:
        raise StorageError(f"log line {index} is blank")
    parts = line.split(" ", 2)
    if len(parts) != 3:
        raise StorageError(f"log line {index} is malformed: {line!r}")
    seq_text, checksum_text, payload = parts
    if not seq_text.isdigit():
        raise StorageError(f"log line {index} has a non-numeric seq: {seq_text!r}")
    if len(checksum_text) != _CHECKSUM_DIGITS:
        raise StorageError(f"log line {index} has a malformed checksum: {checksum_text!r}")
    return int(seq_text), payload
