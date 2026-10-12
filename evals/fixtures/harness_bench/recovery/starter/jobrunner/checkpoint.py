"""Durable, self-repairing job checkpoints. See ../CONTRACT.md."""
import json
from pathlib import Path
from zlib import crc32

from .errors import CheckpointError

_CHECKSUM_DIGITS = 8


class CheckpointStore:
    """Versioned checkpoint with an atomic write and a one-deep backup.

    `save()` never overwrites the live file in place: it writes a temp file,
    demotes the current checkpoint to `<name>.bak`, then renames the temp
    over `<name>`. So at most one of the two files can be damaged by a crash,
    and `load()` can always repair from the other.
    """

    def __init__(self, path):
        self._path = Path(path)
        self._backup_path = self._path.with_name(self._path.name + ".bak")
        self._temp_path = self._path.with_name(self._path.name + ".tmp")
        self._path.parent.mkdir(parents=True, exist_ok=True)

    @property
    def path(self):
        return self._path

    @property
    def backup_path(self):
        return self._backup_path

    def load(self):
        """The newest valid checkpoint payload, or None when none was written.

        Repairs the live file from the backup when the live file is damaged,
        and always discards a leftover temp file (a half-written record is
        never adopted). Raises CheckpointError when a checkpoint exists but
        no copy of it is readable.
        """
        live = _read_record(self._path)
        if live is None:
            if self._path.exists():
                raise CheckpointError(f"no readable checkpoint at {self._path}")
            return None
        return live["payload"]

    def save(self, payload):
        """Atomically store `payload` as the next checkpoint version."""
        if not isinstance(payload, dict):
            raise CheckpointError(f"checkpoint payload must be a dict, got {type(payload).__name__}")
        record = {"version": self._next_version(), "payload": payload}
        record["checksum"] = _checksum(record["payload"], record["version"])
        self._path.write_text(_dump(record), encoding="utf-8")
        assert _read_record(self._path) is not None, "a saved checkpoint must read back"
        return record

    def _next_version(self):
        live = _read_record(self._path)
        backup = _read_record(self._backup_path)
        best = _newest(live, backup)
        return 1 if best is None else best["version"] + 1


def _dump(record):
    try:
        return json.dumps(record, sort_keys=True)
    except TypeError as exc:
        raise CheckpointError(f"checkpoint payload is not JSON serializable: {exc}") from exc


def _checksum(payload, version):
    body = _dump({"version": version, "payload": payload})
    return f"{crc32(body.encode('utf-8')):0{_CHECKSUM_DIGITS}x}"


def _read_record(path):
    """The record stored at `path`, or None when it is missing or damaged."""
    if not path.exists():
        return None
    try:
        record = json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, UnicodeDecodeError):
        return None
    if not isinstance(record, dict) or set(record) != {"version", "payload", "checksum"}:
        return None
    if not isinstance(record["version"], int) or not isinstance(record["payload"], dict):
        return None
    return record


def _newest(live, backup):
    candidates = [record for record in (live, backup) if record is not None]
    if not candidates:
        return None
    if len(candidates) == 1:
        return candidates[0]
    return live if live["version"] >= backup["version"] else backup
