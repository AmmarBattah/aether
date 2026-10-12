# Linestore contract

`linestore/` is a line-oriented key/value protocol server with a write-ahead
log (WAL), log replay, and log compaction. Fix the bugs in `linestore/` so it
satisfies this contract exactly. Run `python3 -m unittest discover -s tests`
for the visible smoke tests. Do not weaken or delete any check described here,
and do not change any public name/signature listed below. Standard library
only; no threads, no sockets, no wall-clock or random behaviour.

Four cross-cutting invariants hold across every module and are what the graded
oracle checks hardest:

1. **Journal before acknowledge.** A mutation is durable in the WAL *before*
   the server applies it in memory and *before* it answers `OK`.
2. **Replay is idempotent.** Applying the same records again (in one pass or
   in repeated passes) never changes the resulting state.
3. **Compaction preserves observable state.** After `compact()`, every read
   answers exactly as before, and the highest sequence number in the log is
   unchanged, so sequence numbers never go backwards after a recovery.
4. **Malformed input never corrupts state.** A bad frame or a bad request
   produces an error *response*; it never mutates the store and never writes
   to the log.

## `linestore.errors`

- `LineStoreError(Exception)` — base class.
- `FramingError(LineStoreError)` — the byte stream is not valid frames.
- `ProtocolError(LineStoreError)` — a payload is not a valid message.
- `StorageError(LineStoreError)` — the log is corrupt or misused.

## `linestore.framing`

A frame is `<n>|<payload>\n`, where `<n>` is the **UTF-8 byte length** of
`<payload>` in decimal ASCII digits. `MAX_PAYLOAD_BYTES = 4096` and
`MAX_HEADER_DIGITS = 8`.

- `encode_frame(payload: str) -> bytes` — raises `FramingError` if `payload`
  contains a newline or encodes to more than `MAX_PAYLOAD_BYTES` bytes. The
  header counts bytes, not characters, so non-ASCII payloads round-trip.
- `FrameDecoder()`
  - `feed(chunk: bytes) -> None` — buffers bytes; raises `FramingError` if
    `chunk` is not `bytes`/`bytearray`.
  - `next_frame() -> str | None` — pops the next complete payload, or returns
    `None` while the buffer holds only part of a frame. A frame may arrive
    split across any number of `feed()` calls, and one `feed()` may carry many
    frames.
  - `pending_bytes() -> int`, `is_poisoned() -> bool`.
  - **Poisoning**: malformed input raises `FramingError` and permanently
    poisons the decoder — a stream whose framing is broken cannot be
    resynchronised. After that, `pending_bytes()` is `0`, `is_poisoned()` is
    `True`, and *every* later `feed()`/`next_frame()` raises `FramingError`.
    Malformed means: a header byte that is neither a digit nor `|`; an empty
    length; more than `MAX_HEADER_DIGITS` digits before any `|`; a declared
    length above `MAX_PAYLOAD_BYTES`; a frame not terminated by `\n` at the
    declared length; a payload containing a newline; a payload that is not
    valid UTF-8.

## `linestore.protocol`

Payload grammar: space-separated tokens, each percent-escaped with
`urllib.parse.quote(token, safe="")`, so a token may contain spaces or be
empty and still survive the round trip. Tokens are split on **single**
spaces, so an empty trailing token is still a token.

- `REQUEST_ARITY = {"PING": 0, "GET": 1, "LIST": 1, "SET": 2, "DEL": 1, "INCR": 2}`
- `MUTATION_OPS = {"SET", "DEL", "INCR"}`
- `RESPONSE_ARITY = {"PONG": 0, "OK": 1, "VAL": 1, "ERR": 2, "KEYS": None}`
  (`None` means variable arity.)
- `ERROR_CODES = {"protocol", "framing", "notfound", "badvalue", "storage"}`
- `MAX_KEY_BYTES = 64`
- `Request(op, args)` / `Response(kind, args)` — `args` is a tuple.
- `encode_request(op, *args) -> str`, `encode_response(kind, *args) -> str`.
- `parse_request(payload) -> Request` — raises `ProtocolError` for an empty
  payload, an unknown op, wrong arity, an empty key, a key over
  `MAX_KEY_BYTES` UTF-8 bytes, a key containing whitespace or control
  characters, or a non-integer `INCR` delta. A `LIST` prefix may be empty and
  is not validated as a key.
- `parse_response(payload) -> Response` — raises `ProtocolError` for an
  unknown kind, wrong arity, a non-numeric `OK` sequence, or an error code
  outside `ERROR_CODES`.
- `is_integer_text(text) -> bool` — decimal integer literal, optional `-`.

## `linestore.wal`

Record line format: `<seq> <crc32-of-payload, 8 lowercase hex digits> <payload>\n`.

- `WriteAheadLog(path)` — opens/creates the log for appending; `path`
  property.
- `append(seq: int, payload: str) -> None` — appends one record and flushes,
  so a *separate* reader opened on the same path sees it before `close()`.
  Raises `StorageError` if the log is closed.
- `records() -> list[(int, str)]` — every complete record on disk, in write
  order, read directly from the file. A final line with **no** trailing
  newline is a torn write left by a crash and is dropped silently. Any other
  malformed line — blank, missing fields, non-numeric seq, malformed or
  mismatched checksum — is corruption and raises `StorageError`, as does a
  sequence number that does not strictly increase.
- `last_seq() -> int` — the highest seq on disk, or `0` for an empty log.
- `replace_with(records) -> int` — **atomically** replaces the entire log with
  `records` (write a `<name>.compact` temp file, then `os.replace` it over the
  log, then reopen the append handle; the temp file must not survive). Raises
  `StorageError` for non-increasing seqs or a closed log. Returns the count.
- `close() -> None` — flush and close, **idempotent**.
- Module helpers: `encode_record(seq, payload) -> str`,
  `require_increasing(records) -> None`.

## `linestore.store`

- `Store()` — `get(key) -> str | None`, `keys(prefix="") -> list[str]`
  (sorted), `snapshot() -> dict` (a copy), `applied_seq() -> int`.
- `apply(seq, payload) -> bool` — applies one journaled mutation and returns
  whether it advanced the store. **Idempotent by sequence number**: a record
  whose `seq <= applied_seq()` is ignored and returns `False`. A non-mutation
  op in the log raises `StorageError`, as does an unparseable payload. A
  record whose precondition no longer holds is a **no-op, not an error**
  (compaction can legitimately leave a `DEL` of an already-absent key, and an
  `INCR` of an absent or non-integer key is ignored). Values are strings;
  `INCR` rewrites the value as the decimal sum.
- `replay(records) -> Store` — a fresh `Store` with every record applied.

## `linestore.server`

- `Server(wal_path, *, wal=None)` — does **not** read an existing log; use
  `recover()`. `wal`, when given, is used instead of opening `wal_path`
  (dependency injection, e.g. a double whose `append()` raises).
- `Server.recover(wal_path, *, wal=None) -> Server` (classmethod) — rebuilds
  the store from the records already on disk **without appending anything**,
  and sets `next_seq` to `last_seq() + 1` (not to the record count).
- `next_seq` property, `applied_seq()`, `get(key)`, `keys(prefix="")`,
  `snapshot()`.
- `handle_request(payload: str) -> str` — one decoded payload in, one response
  payload out.
  - `PING` -> `PONG`; `GET k` -> `VAL v` or `ERR notfound k`;
    `LIST prefix` -> `KEYS ...` (sorted).
  - Reads are never journaled.
  - A `ProtocolError` from parsing becomes `ERR protocol <detail>`; nothing is
    journaled and nothing changes.
  - A mutation whose precondition fails — `DEL`/`INCR` of an absent key
    (`ERR notfound k`), `INCR` of a non-integer value (`ERR badvalue k`) — is
    **not** journaled and changes nothing.
  - An accepted mutation journals the **canonically re-encoded** request
    (`encode_request(op, *args)`) at `next_seq`, *then* applies it to the
    store, *then* advances `next_seq`, and answers `OK <seq>`. If
    `WriteAheadLog.append()` raises, the exception propagates, the store is
    unchanged, and the sequence number is **not** consumed: the next accepted
    mutation reuses it.
- `handle_bytes(chunk: bytes) -> bytes` — feeds the decoder, serves every
  complete frame it yields, and returns the concatenated framed responses.
  A `FramingError` is converted to a single framed `ERR framing <detail>`
  response appended after the responses for the frames that decoded cleanly
  before it; the store is untouched. Because the decoder is poisoned, every
  later `handle_bytes()` call returns just that framing error.
- `compact() -> int` — rewrites the log via `WriteAheadLog.replace_with()` to
  `compaction_records()` and returns the record count. Observable state and
  `WriteAheadLog.last_seq()` are unchanged, so `recover()` afterwards yields
  the same snapshot and the same `next_seq`, and it is safe to call twice.
- `close() -> None` — idempotent.
- `compaction_records(records) -> list[(int, str)]` — one `SET` record per
  live key, carrying its current value at the seq of the last record that
  touched that key, sorted by seq; plus the log's final record verbatim when
  it is the only carrier of the highest sequence number (for example a
  trailing `DEL` of a key that is now gone).
