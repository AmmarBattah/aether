"""Append-only journal persistence. Full contract in ../../starter/CONTRACT.md."""
import json

from .errors import LedgerError


class Journal:
    def __init__(self, path):
        self._path = path
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._handle = open(self._path, "a", encoding="utf-8")

    def append(self, entry):
        self._handle.write(json.dumps(entry) + "\n")
        self._handle.flush()

    def read_all(self):
        """Every complete (newline-terminated) record, in write order.

        A crash mid-append can leave an unterminated final line on disk; that
        is dropped silently rather than raising. Any *complete* line that
        still fails to parse is real corruption and raises LedgerError.
        """
        with open(self._path, "r", encoding="utf-8") as handle:
            raw = handle.read()
        if raw == "" or raw.endswith("\n"):
            complete_lines = raw.split("\n")[:-1]
        else:
            *complete_lines, _truncated_tail = raw.split("\n")
        return [self._parse_line(line) for line in complete_lines if line.strip()]

    def _parse_line(self, line):
        try:
            return json.loads(line)
        except json.JSONDecodeError as exc:
            raise LedgerError(f"corrupt journal record: {exc}") from exc

    def close(self):
        # Idempotent: replay/inspection code may legitimately close a
        # Journal it does not know has already been closed elsewhere.
        if self._handle.closed:
            return
        self._handle.flush()
        self._handle.close()
