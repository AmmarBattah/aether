# Serializable transactional store

Repair `txstore/`. Standard library only. The public API below is complete;
visible smoke tests are intentionally not comprehensive. Operations are
deterministic, single-threaded, but transactions can be interleaved arbitrarily.
Do not change API signatures, this contract, or existing visible tests.

## Values, snapshots, and errors

`from txstore import Store, Conflict, Closed, Invalid, Corrupt`

All four errors subclass `Exception`. Keys and transaction IDs are nonempty
strings; prefixes may be empty strings. Strings must be UTF-8 encodable.
Values are JSON trees containing only
null, bool, int, finite float, str, list, and dict with string keys. Unsupported
types and non-finite numbers raise `Invalid`, including in nested containers.
Every accepted value is deep-copied on entry and on return. No caller mutation
may alter store state, pending writes, receipts, or savepoints. Returned `None`
means either a missing key or a stored JSON null; `scan` distinguishes them.

`Store(journal=None)` starts at revision 0 and optionally recovers from journal.
`store.revision` is the latest committed revision. `store.snapshot()` returns a
copy of all currently present key/value pairs. `store.begin(txid)` returns a
new transaction with a snapshot of the store at that moment. A transaction ID
can be reused; it is an idempotency key at commit, not a lock at begin.

## Transaction API

* `get(key)` returns the transaction's pending value/deletion if any, otherwise
  its begin-time value. Record a point-read dependency even on a missing key
  or a key supplied by a pending write.
* `scan(prefix="")` returns a sorted list of `(key, value)` pairs whose keys
  start with prefix, with pending writes/deletions overlaid on the begin-time
  snapshot. Record the prefix as a range-read dependency, including empty
  results. Repeated reads keep the same snapshot, not the latest store state.
* `put(key, value)` stages a defensive copy. `delete(key)` stages a deletion,
  including for an absent key. Last write to a key wins. A write alone does
  not create a point-read dependency but has a write-write conflict check.
* `savepoint(name)` records a copy of the current write set. Name is a nonempty
  string unique among active savepoints; duplicates raise `Invalid`.
* `rollback(name)` restores the writes recorded at that savepoint and discards
  later savepoints. The named savepoint remains reusable. All reads performed
  before OR after the savepoint remain dependencies: rollback does not erase
  observations. Unknown names raise `Invalid` without modifying anything.
* `commit()` returns a defensive copy of the receipt described below. A
  successful commit closes the transaction. Every subsequent operation on a
  closed transaction, including another commit or rollback, raises `Closed`.
  Check closed status before validating any arguments. Conflicts, invalid
  commit replay, and journal failures do NOT close the transaction.
* `abort()` closes the transaction without changing store state or journal;
  aborting an already closed transaction raises `Closed` too.

Validate every argument before changing writes, reads, savepoints, or state.
Invalid operations must be side-effect free. Transactions may read new values
only via their own writes, never by silently refreshing their snapshots.

## Serializable commit and idempotency

Maintain each key's last mutation revision, INCLUDING tombstones. An identical
put and an absent-key delete are still mutations. For a new transaction ID,
raise `Conflict` if, since this transaction began:

1. Any point-read key or final write-set key was mutated; or
2. ANY mutated key matches a scanned prefix, even if it was subsequently
   deleted, or restored to its old value. This includes phantom insert/delete
   and ABA changes invisible to snapshot-value comparison.

Check only the final writes after rollbacks, but ALL recorded reads. Conflicts
must leave store, journal, and transaction unchanged. Disjoint blind writes
may commit despite intervening commits. Read-only transactions also validate
reads, consume one new revision, and persist a receipt.

A receipt is `{"txid": id, "revision": n, "writes": operations}`. Operations
are sorted by key, exactly `[["put", key, value], ["delete", key], ...]`, one
per final write-set key. Do not elide no-op puts or absent-key deletes.

If txid already committed, compare its stored operations with the final write
set using canonical JSON representation (so bool/int/float remain distinct).
An exact match returns its ORIGINAL receipt and closes the transaction, with
no conflict checks, append, mutations, or revision bump. A mismatch raises
`Invalid` and leaves the transaction open. This applies even after restart.

## Journal format, append failure, and recovery

A journal has `read() -> bytes` and `append(record: bytes) -> None`. Neither
method is called when no journal was supplied. `append` is all-or-nothing: an
exception means it did not write any bytes. Propagate that same exception;
never publish a commit or receipt before append succeeds. Retrying the SAME
transaction after an append failure must work, subject to normal conflicts.

Every successful NEW commit appends exactly one complete record. A record is:

`8 ASCII lowercase hex digits of payload byte length + b":" + payload + b":"
+ 64 ASCII lowercase hex digits of SHA-256(payload) + b"\n"`

Payload is UTF-8 JSON using `sort_keys=True, separators=(",", ":"),
ensure_ascii=False, allow_nan=False`. It is exactly the receipt object.
Length counts bytes, not Unicode characters. Export helpers:

* `txstore.codec.encode(receipt) -> bytes`: encode this framing/canonical JSON.
* `txstore.codec.decode(data: bytes) -> list[dict]`: validate and decode frames.
  Ignore only a syntactically valid but incomplete FINAL frame. Validate every
  available header/delimiter/checksum byte; a bad byte is `Corrupt`, even in
  a final short frame. A full invalid checksum, UTF-8, JSON, noncanonical JSON,
  or invalid receipt is `Corrupt`, never silently truncated.

Receipt validation: exact object keys txid/revision/writes; nonempty string
txid; positive int revision (not bool); list of valid put/delete operations
with valid keys and JSON values; strictly ascending distinct keys. No extra
fields or operation elements. `encode` raises `Invalid` on invalid receipts.

`Store(journal)` calls read once and replays decoded receipts. Unique new
receipts must have revisions 1,2,... without gaps. An EXACT repeated receipt
(same canonical JSON, including revision) is accepted anywhere after its
original and ignored. Reusing txid with any different receipt, or reusing a
revision for a different txid, is `Corrupt`. Recovery restores key tombstones,
revision and idempotency receipts, and must not append any bytes.

Journal objects in this exercise permit continued appending only after their
owner has removed any incomplete tail; automatic tail truncation is NOT part
of this API. No filesystem durability or actual multithreading is required.
