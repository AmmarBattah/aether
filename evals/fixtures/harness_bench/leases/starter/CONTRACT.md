# Durable fenced resource leases

Repair the stdlib-only `leasepool` package. This is a deterministic simulation:
callers interleave operations explicitly; no threads, real clocks, sleeps, or
background workers are needed. Do not modify this contract or visible tests.

## API

`Pool(append, records=())`: `append(record)` durably appends a JSON-compatible
record or raises. The constructor replays `records` without calling `append`.
Methods return defensive copies. `ValueError` denotes invalid input or an
invalid journal; `Conflict` denotes an incompatible reused request ID;
`StaleLease` denotes a missing, queued, cancelled, or expired lease.

* `request(key, resources, priority, ttl, now) -> dict`: submit or replay a
  request. `key` is a nonempty string. `resources` is a nonempty list of distinct
  nonempty strings; order does not affect request identity. `priority` is an
  integer (not bool), with larger values winning. `ttl` is a positive integer
  (not bool). `now` is a nonnegative integer (not bool).
* `advance(now) -> list[dict]`: expire leases and grant eligible queued work;
  return newly granted requests in grant order.
* `renew(key, fences, ttl, now) -> dict`: extend a currently active lease to
  `now + ttl`, after validating all its fences and the new positive TTL.
* `release(key, fences, now) -> dict`: release a currently active lease.
* `cancel(key, now) -> bool`: cancel queued or active work; return false for an
  unknown key or one already terminal. Cancelled requests cannot be resubmitted.
* `get(key) -> dict | None`: inspect a request without advancing time.
* `snapshot() -> dict`: return `{"time": int, "sequence": int,
  "requests": list[dict], "counters": dict[str, int]}`. Requests are in arrival
  order. The initial snapshot is time 0, sequence 0, no requests/counters.

Every request dictionary has exactly these fields: `key`, `resources` (sorted),
`priority`, `ttl` (the originally requested TTL), `arrival` (1-based request
arrival order), `state` (`queued`, `active`, `expired`, `released`, `cancelled`),
`fences` (resource-to-integer dictionary), `deadline` (integer or null).
Queued requests have empty fences and null deadline. Terminal requests retain
their most recent fences/deadline for inspection.

## Time and atomicity

Every mutator validates its arguments and identity/fences before changing
anything. Time may stay equal but never decrease relative to the last successful
mutation. A failed operation changes neither time nor state and appends nothing.

A successful mutator first expires active leases whose deadline is **at or
before** `now`, then applies the requested action, then schedules queued work.
`renew` and `release` targeting a lease expiring at `now` fail atomically, without
expiring any other lease. A false `cancel` is nevertheless a successful mutation
and can expire/grant other work. `get`/`snapshot` never mutate or append.

All successful mutations append exactly one record, even a no-op `advance` or
idempotent `request`. Call `append` before publishing any in-memory change. If
append raises, propagate that exception, leave all state unchanged, and allow a
retry to produce the same record. Assume an append that raises persisted nothing.
The callback may retain or mutate its argument; it must not own internal state.

## Scheduling, fairness, fencing

Scheduling considers queued requests by descending priority, then ascending
arrival. A request needs every resource simultaneously. An earlier considered
request that cannot run reserves **all** its resources against later considered
requests during this scheduling pass. Thus a blocked request prevents conflicting
overtaking, but does not block disjoint work. Already active leases are never
preempted. Allocation and reservation happen atomically within each mutation.

Each resource has a durable fencing counter starting at zero. Every grant
increments every acquired resource's counter by one and records the resulting
values in that request. Releasing, expiring, cancelling, and renewing do not
increment counters. Fences must be a dictionary with exactly the lease's resource
keys and positive integer (not bool) values, all matching the current grant.

Reusing a request key with the same sorted resources, priority, and original TTL
returns its current record (including a terminal record); it never creates a new
arrival or lease. Any different specification raises `Conflict` atomically.
The return value reflects all scheduling performed by that successful call.

## Durable records and replay

A journal record has exactly `{"sequence": n, "operation": op, "arguments": a}`.
Sequence starts at one and is contiguous. Operations match mutator names, and
arguments are dictionaries keyed by the documented parameter names. Resources
are normalized to sorted order in `request` records. Fences are copied as passed.
Replay recomputes the state through the same validation and scheduling rules.
Reject wrong keys/types, unknown operations, gaps, repeated sequences, invalid
operations, backwards time, or any record that would fail when executed.
Replay must not mutate its input records, invoke `append`, or silently accept a
valid prefix of an invalid journal. Noncanonical resource ordering is invalid.

The tests may run long randomized interleavings, replay after each boundary,
inject append failures, mutate returned values/callback records, and exercise
priority inversions, deadline equality, stale fences, and terminal-key reuse.
