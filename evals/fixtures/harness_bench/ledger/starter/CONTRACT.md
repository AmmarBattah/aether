# Ledger contract

Implement a transactional, idempotent, journaled account ledger. Fix the bugs
in `ledger/` so it satisfies this contract exactly (`python3 -m ledger.errors`
imports cleanly; run `python3 -m unittest discover -s tests` for the visible
smoke tests). Do not weaken or delete any check described here, and do not
change any public name/signature listed below.

## `ledger.errors.LedgerError(Exception)`

Raised for invalid ledger usage: unknown op names, missing/invalid kwargs,
and any other contract violation described below as "raises LedgerError".

## `ledger.accounts.Accounts`

- `exists(account_id) -> bool`
- `open(account_id, opening_balance=0) -> None` — raises `LedgerError` if the
  account already exists or `opening_balance < 0`.
- `balance(account_id) -> int` — raises `LedgerError` if unknown.
- `credit(account_id, amount) -> None` — raises `LedgerError` if `amount <= 0`
  or the account is unknown.
- `debit(account_id, amount) -> None` — raises `LedgerError` if `amount <= 0`,
  the account is unknown, **or the resulting balance would be negative**.
  Must not partially apply (balance stays unchanged when it raises).
- `snapshot() -> dict` — a point-in-time copy of all balances.
- `restore(snapshot: dict) -> None` — replaces current balances with a copy
  of `snapshot`. Used by `LedgerEngine` to roll back a partially-applied
  operation; mutating the dict passed to or returned by these two methods
  must never affect `Accounts`' own internal state or vice versa.

## `ledger.journal.Journal`

- `Journal(path)` — opens (creating if needed) an append-only journal file.
- `append(entry: dict) -> None` — serializes `entry` as one JSON line and
  **durably** appends it (a separate `Journal`/file handle opened on the same
  path afterward must be able to read it back, even before `close()`).
- `read_all() -> list[dict]` — every **complete** entry ever appended, in
  write order, read directly from disk. A JSON-parse failure on a line that
  ends with a newline is real corruption and raises `LedgerError`. A final
  line with **no** trailing newline is treated as a truncated write left
  behind by a crash mid-`append()`; it is silently dropped (not returned,
  never raises), whether or not it happens to parse as JSON.
- `close() -> None` — flushes and closes the file handle. **Idempotent**: a
  second `close()` call (on this `Journal` or, transitively, on the
  `LedgerEngine` that owns it) is a safe no-op, not an error.

## `ledger.engine.LedgerEngine`

- `LedgerEngine(journal_path, *, journal=None)` — does **not** read an
  existing journal; use `replay()` for that. If `journal` is given, it is
  used instead of opening `journal_path` (dependency injection, e.g. a test
  double that raises on `append()` to simulate a write failure); otherwise a
  real `Journal(journal_path)` is opened.
- `balance(account_id) -> int`
- `history(account_id) -> list[dict]` — every journaled entry that mentions
  `account_id` (as `account`, `src`, or `dst`), in journal order, including
  rejected entries (for audit). Each returned dict is an independent copy:
  mutating it (or a dict previously returned by `apply()`) must never affect
  the engine's own state or any later `apply()`/`history()` result.
- `apply(request_id, op, **kwargs) -> dict` — `op` is one of:
  - `"open_account"`: kwargs `{account, opening_balance=0}`. A negative
    `opening_balance` raises `LedgerError` synchronously (like `Accounts.open`)
    rather than being journaled as a routine rejection.
  - `"deposit"`: kwargs `{account, amount}`
  - `"withdraw"`: kwargs `{account, amount}`
  - `"transfer"`: kwargs `{src, dst, amount}`. `src == dst` is legal: if the
    account has at least `amount`, the transfer is `"applied"` and the
    balance is unchanged (debit then credit of the same account nets to
    zero); if it does not, the transfer is `"rejected"` with reason
    `"insufficient funds"` and the balance is unchanged either way.

  A successful (non-exception-raising) call appends exactly one entry to the
  journal and returns `{"request_id", "status": "applied"|"rejected",
  "reason": str|None, "op", **kwargs}`.
  **Idempotent**: a second `apply()` with a `request_id` already seen (i.e.
  its journal append previously succeeded) returns a copy of the exact first
  result and has no further effect on balances, journal, or history,
  regardless of what `op`/kwargs are passed the second time.
  A `transfer` is atomic: if it is rejected, neither account's balance has
  changed. An unknown `op` name raises `LedgerError` (not journaled).
  **Append-failure rollback**: if the underlying `Journal.append()` raises
  (e.g. a failing injected `journal`), any in-memory balance mutation already
  made by this `apply()` call is rolled back via `Accounts.restore()` before
  the exception propagates; the operation is not recorded in `history()` or
  the idempotency cache, so a later `apply()` with the same `request_id`
  retries the operation from scratch rather than treating it as already seen.
- `LedgerEngine.replay(journal_path) -> LedgerEngine` (classmethod) —
  reconstructs an engine purely from the entries already on disk (accounts,
  balances, history, and the idempotency cache), **without appending anything
  new to the journal**. Applies `Journal.read_all()`'s truncation/corruption
  policy (see above). The rebuilt engine's `balance()` and `history()` must
  match the original engine's, entry for entry.
- `close() -> None` — idempotent, see `Journal.close()`.
