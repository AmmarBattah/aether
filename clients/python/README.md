# aether (Python)

A client for the Aether daemon protocol.

`aether/protocol.py` is the public facade for generated types and builders.
Its `_protocol_types/` and `_protocol_commands/` sibling packages are generated
from the Rust protocol types by `scripts/generate_clients.py` too. Do not edit
generated files: change the Rust types and regenerate. Keep the whole `aether/`
package when copying the client; public imports through `protocol` are unchanged.

The generated `TypedDict`s are all `total=False`, so a type checker will not
flag a missing required field: which fields the wire requires is carried as a
`# optional` marker on the ones it does not. `typing.Required` would make it
checkable and is Python 3.11+, which this library does not require. The
command builders do enforce it - a required field is a keyword-only argument
without a default - and building a frame with a builder is what a caller
should be doing anyway. `aether/client.py` is the hand-written connection on top of it -
handshake, request/reply over one socket, subscriptions, keepalive.

```python
from aether import AetherClient, protocol

client = AetherClient.connect_unix()                     # the daemon's socket
client = AetherClient.connect_tcp("127.0.0.1", 7777, token)   # or over TCP

agents = client.request(protocol.list_agents(query={}))["data"]
subscription = client.subscribe(after=0)  # 0 replays the log; None starts live
for event in subscription.events():
    print(event["kind"]["type"])
```

`AetherClient.connect()` picks the transport for you: `AETHER_ADDRESS`
(`host:port`) with `AETHER_TOKEN` if they are set, the Unix socket otherwise.

The TCP transport closes a connection that says nothing for
`idle_timeout_secs`; the client pings for you as long as it is open.

Rust, TypeScript, and Python buffer at most 1024 event/delta frames per
subscription. Overflow stops the affected daemon subscription; the retained
prefix drains before a terminal lag signal. Other subscriptions and requests
continue. Resume events after the last consumed sequence (or the initial
subscription cursor if no event was consumed). Live deltas cannot be replayed.
Rust yields `SubscriptionFrame::Lagged`; TypeScript and Python raise
`SubscriptionLaggedError`.

Python's error carries `subscription_id`, `last_seq`, and `capacity`:

```python
from aether import SubscriptionLaggedError

subscription = client.subscribe(after=last_seq)
try:
    for event in subscription.events():
        process(event)
except SubscriptionLaggedError as lag:
    subscription = client.subscribe(after=lag.last_seq)
```

`subscribe()` returns a `Subscription` handle with `next_frame()`, `events()`,
and `unsubscribe()`. It remains a mapping with the legacy `id` and initial
`last_seq` keys. `client.next_frame()` and `client.events()` merge the same
per-subscription buffers in arrival order and raise the same lag error.
Use either a handle or the merged reader to consume a subscription; they
share its cursor and do not duplicate frames. A merged reader can continue
reading other subscriptions after catching the error.
`client.dropped_frames` is deprecated, emits `DeprecationWarning`, and always
returns zero: overflow is now terminal and observable rather than silently
dropping the oldest frame.

Run the example against a running daemon:

```sh
python3 clients/python/examples/list_and_stream.py
```

No dependencies beyond the standard library.

## Recurring timers

Use the generated builders through the same authenticated connection:

```python
created = client.request(protocol.create_schedule(spec={
    "name": "Read docs",
    "target": agent_id,
    "recurrence": {"kind": "interval", "seconds": 60},
    "payload": "Read documentation and report changes",
    "max_deliveries": 3,
}))
job_id = created["data"]["id"]
history = client.request(protocol.schedule_history(id=job_id, limit=100))
client.request(protocol.update_schedule(id=job_id, action="pause"))
```

`list_schedules`, `get_schedule`, and `update_schedule` also support inspection,
resuming, and cancellation. History records trigger delivery, skipping, or
failure; it does not claim that the model completed the task. Put a `spawn`
template (`role`, `objective`, optional model and scope) in place of `target`
to start a fresh root run on each occurrence instead (protocol 9).
