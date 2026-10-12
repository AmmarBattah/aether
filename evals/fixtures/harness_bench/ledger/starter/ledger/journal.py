"""Append-only journal persistence. Full contract in ../CONTRACT.md."""
import json


class Journal:
    def __init__(self, path):
        self._path = path
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._handle = open(self._path, "a", encoding="utf-8")

    def append(self, entry):
        # BUG: never flushed, so a separate reader (another Journal/file
        # handle opened on the same path, e.g. during crash recovery)
        # will not see this entry until the writer happens to flush or
        # close for an unrelated reason.
        self._handle.write(json.dumps(entry) + "\n")

    def read_all(self):
        with open(self._path, "r", encoding="utf-8") as handle:
            return [json.loads(line) for line in handle if line.strip()]

    def close(self):
        self._handle.flush()
        self._handle.close()
