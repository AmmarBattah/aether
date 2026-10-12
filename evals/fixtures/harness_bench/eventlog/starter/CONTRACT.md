# Durable event stream and consumer groups

Implement the stdlib-only `eventlog` package. Operations are explicitly interleaved; there are no background threads or real clocks. `append(record)` durably appends a JSON-compatible record or raises. Constructor replay never calls append. Public methods return defensive copies.

## API

- `Hub(append, records=())`
- `publish(key, payload) -> dict`: append one event and return `{"offset": int, "key": str, "payload": JSON value}`. A key is a nonempty string. Payload is a finite JSON value (null/bool/int/finite float/string/list/dict with string keys); reject cycles and non-finite floats. Re-publishing a key with a deeply equal payload returns its original event without adding an event. Reusing the key with any type-distinct or otherwise different payload raises `Conflict`.
- `create_group(name, start="earliest") -> dict`: create a consumer group, where start is `earliest` (committed offset 0) or `latest` (current last event offset). Repeating the same creation returns the existing group; a different start raises `Conflict`.
- `claim(group, consumer, now, ttl) -> dict`: acquire or renew group ownership. Names are nonempty strings, now is a nonnegative integer (not bool), ttl is a positive integer (not bool). Time cannot go backwards relative to the last successful time-bearing operation. If another owner's lease has deadline strictly greater than now, raise `Busy`. Same-owner renewal keeps generation. A new owner, including reclaim after expiry, increments generation exactly once. Result is `{group, consumer, generation, lease_until}`.
- `poll(group, consumer, generation, limit, now) -> list[dict]`: return up to limit events after the group's committed offset, in offset order. This is a read: it never appends or changes state/time. It requires the current owner, matching generation, and lease deadline strictly greater than now. Limit is a positive integer (not bool); now is valid and cannot precede the last successful time-bearing operation.
- `ack(group, consumer, generation, through, now) -> dict`: atomically move committed offset to `through`. The value must be an integer (not bool), cannot move backwards, and cannot exceed the log tail. An equal-offset acknowledgment is a successful no-op that still appends one journal record. It is fenced by the active, unexpired claim.
- `release(group, consumer, generation, now) -> dict`: release current ownership. It is fenced by the active, unexpired claim.
- `get_group(name) -> dict | None`: returns `{name, committed, start, owner, generation, lease_until}`; unowned groups have owner and lease_until null.
- `snapshot() -> dict`: `{sequence, time, events, groups}`. Events and groups are in creation order. Initial state is sequence/time 0 and empty lists.

Errors: `Invalid` subclasses `ValueError` for malformed arguments/journal, `Conflict` for incompatible reuse, `Busy` when another unexpired owner holds a group, and `StaleClaim` for missing, expired, wrong-owner, or wrong-generation claims. Failed operations must leave time, sequence, events, groups, and append callback untouched.

## Durability and replay

Each successful mutator (`publish`, `create_group`, `claim`, `ack`, `release`) appends exactly one record before publishing in-memory changes. Even idempotent publish/group creation and equal-offset ack append once. Records have exactly `{"sequence": n, "operation": op, "arguments": dict}`; sequence is contiguous from one. Operation names are the method names and arguments use their public parameter names exactly. Replay must execute each record through the same validation and state transitions, reject unknown keys/types/operations, gaps, noncanonical or invalid records, and the entire invalid journal (never accept only a prefix). It must not mutate input records or call append.

The callback may retain or mutate its argument. Internal state and returned values must not be aliased to callback-owned or caller-owned objects. If append raises, propagate the same exception and publish no state; retrying the operation must produce the same record. Assume a raising append persisted nothing.

Payload equality is type-sensitive recursively (`true` differs from `1`). Offsets are one-based and never reused. Consumer acknowledgments may skip unseen events but may never create a gap in the committed cursor: one group cursor represents all events through that offset. Poll is repeatable until ack. Expiry is checked at equality (`lease_until <= now` is expired). A claim at the deadline may be taken by a new owner; the expired generation can never ack or release.
