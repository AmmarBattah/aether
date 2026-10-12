# Limit order book with price-time priority

Implement the stdlib-only `matchbook` package: a single-instrument matching
engine with a durable journal. Everything is single-threaded and deterministic;
there are no real clocks. This file is the complete public contract.

```python
from matchbook import Engine, Invalid, Conflict, NotFound, Forbidden, Closed
```

## Errors

- `Invalid` (subclasses `ValueError`): malformed arguments or a malformed journal.
- `Conflict`: an order id that was already used.
- `NotFound`: an order id that was never accepted.
- `Forbidden`: the caller does not own the order.
- `Closed`: the order is not open at the operation's time.

When several apply, the first in this order is raised: `Invalid`, `Conflict`,
`NotFound`, `Forbidden`, `Closed`. An operation that raises is a *failed*
operation: it changes nothing at all (no state, no clock, no expiry, no journal
record). An operation that returns is an *accepted* operation.

## Engine

`Engine(append=None, records=())`

`append(record)` durably appends one JSON-compatible record or raises; `None`
means no journal. `records` is a previously written journal to replay (see
"Journal"). The engine clock starts at 0.

### Mutators

Every mutator returns a *report*:

```python
{"order": <order view or None>, "trades": [<trade>, ...],
 "cancelled": [<order id>, ...], "expired": [<order id>, ...]}
```

- `submit(order_id, owner, side, price, quantity, now, tif="GTC", expires=None, post_only=False, stp="cancel_newest")`
- `submit_iceberg(order_id, owner, side, price, quantity, display, now, stp="cancel_newest")` (see "Iceberg orders")
- `amend(order_id, owner, now, price=None, quantity=None)`
- `cancel(order_id, owner, now)`
- `advance(now)`: only moves the clock (and so expires orders); its report has `order` `None`.

Arguments (anything else is `Invalid`):

- `order_id`, `owner`: nonempty `str`.
- `side`: `"buy"` or `"sell"`.
- `price`: a positive `int`, or `None` for a market order (`submit` only). `bool` is never an `int` here.
- `quantity`: a positive `int` (not `bool`).
- `now`: a nonnegative `int` (not `bool`), not less than the engine clock. Equal is allowed.
- `tif`: `"GTC"`, `"IOC"`, `"FOK"` or `"GTD"`.
- `expires`: an `int` (not `bool`) strictly greater than `now` when `tif` is `"GTD"`; must be `None` for every other `tif`.
- `post_only`: a `bool`. `True` requires a limit price and `tif` `"GTC"` or `"GTD"`.
- `stp`: `"cancel_newest"`, `"cancel_oldest"` or `"decrement"`.
- A market order (`price=None`) requires `tif` `"IOC"` or `"FOK"`.
- `amend`: `price` is `None` (keep) or a positive `int`; `quantity` is `None` (keep) or a positive `int`; they may not both be `None`.

### Time and expiry

An accepted operation with time `now` first sets the clock to `now` and then
expires every open order whose `expires` is not `None` and `expires <= now`
(expiry is checked at equality), before the operation's own effect. Expired
orders never trade. The report's `expired` lists the ids this operation
expired, in the order those orders were submitted.

`amend` and `cancel` of an order that this operation's `now` would expire
raise `Closed`; being a failed operation, that leaves the order `"open"` until
some later accepted operation expires it.

### Order view

```python
{"id", "owner", "side", "price", "quantity", "filled", "remaining",
 "tif", "expires", "status", "reason"}
```

- `filled`: total quantity traded so far.
- `remaining`: quantity still resting in the book; `0` whenever `status` is not `"open"`.
- `quantity`: the submitted quantity; after an accepted `amend` it is `filled + remaining`.
- `status`: `"open"`, `"filled"`, `"cancelled"`, `"expired"` or `"rejected"`.
- `reason`: `None` for open, filled and expired orders. For `"cancelled"`: `"user"` (by `cancel`), `"ioc"` (unfilled remainder of an IOC order) or `"stp"` (self-trade prevention). For `"rejected"`: `"post_only"` or `"fok"`.
- `price` is `None` for a market order.

### Matching

An incoming buy can match an open sell whose price is at or below the buy's
limit; an incoming sell can match an open buy whose price is at or above the
sell's limit; a market order matches at any price. The best opposite order is
taken first: lowest price for sells, highest price for buys, and at equal
prices the order that has held its place in the queue longest. Each match
trades `min(incoming remaining, resting remaining)` at the **resting** order's
price. Matching repeats until the incoming order has nothing left or no
matching order remains.

A trade is

```python
{"id": n, "time": now, "price": p, "quantity": q, "buy": <order id>, "sell": <order id>, "taker": <incoming order id>}
```

with `id` contiguous from 1 across the engine's whole life.

After matching, an incoming order with nothing left is `"filled"`. Otherwise:
`"GTC"`/`"GTD"` rest in the book as `"open"` at the back of their price level;
`"IOC"` is `"cancelled"` with reason `"ioc"` (also when nothing traded).

### Post-only

A `post_only` order that could match at least one open opposite order (of any
owner, after this operation's expiries) is `"rejected"` with reason
`"post_only"`: no trade, nothing rests. A rejection is still an accepted
operation: it is journaled, moves the clock, and uses up the order id.

### Self-trade prevention

When the best opposite order has the same `owner` as the incoming order, no
trade happens between them; the incoming order's `stp` decides:

- `"cancel_newest"`: the incoming order becomes `"cancelled"` with reason `"stp"` and matching stops. Trades already made stand; the resting order is untouched.
- `"cancel_oldest"`: the resting order becomes `"cancelled"` with reason `"stp"`, and matching continues.
- `"decrement"`: both are reduced by `min(incoming remaining, resting remaining)` without a trade (`filled` does not change). Whichever reaches zero becomes `"cancelled"` with reason `"stp"`; a resting order with quantity left keeps its place; an incoming order with quantity left keeps matching.

The report's `cancelled` lists the ids of resting orders cancelled this way, in
the order it happened (never the incoming order itself). Self-trade prevention
applies only to an own order that is actually reached by matching.

### Fill-or-kill

A `"FOK"` order executes only if its whole quantity would trade right now,
judged by exactly what matching would do, self-trade prevention included:
under `"cancel_newest"` reaching an own order ends matching; under
`"cancel_oldest"` own orders are skipped; under `"decrement"` decremented
quantity is not traded quantity. If the whole quantity would not trade, the
order is `"rejected"` with reason `"fok"` and nothing else changes: no trades,
no cancellations, no decrements (this operation's expiries still apply, and
like every rejection it is an accepted operation). If it would, it executes
normally, including the self-trade cancellations.

### Amend

`quantity` is the order's new `remaining`. If the price is unchanged (`None` or
equal to the current price) and the new remaining is not greater than the
current remaining, the order keeps its place in the queue (an amend that
changes nothing is still accepted and journaled). Otherwise the order loses
its place: it is taken out of the book and re-entered as an incoming order
with the new price and remaining and its original id, owner, side, `tif`,
`expires` and `stp`, so it may trade immediately as the taker and then rests
at the back of its level. `post_only` is a submission-time check only and
does not apply to an amend. `filled` accumulates across amends.

### Cancel

An open order becomes `"cancelled"` with reason `"user"`.

### Iceberg orders

`submit_iceberg` submits a limit order that shows only part of its quantity.
`price`, `quantity` and `display` are positive `int`s (not `bool`) with
`display <= quantity`; `order_id`, `owner`, `side`, `now` and `stp` are as for
`submit`; anything else is `Invalid`. Order ids are one id space: an id used by
either kind of submit is a `Conflict` for both. The order's view has `tif`
`"GTC"` and `expires` `None`, and the call returns an ordinary report.

As the incoming order an iceberg matches like any limit order, with its whole
quantity. What is left rests at the back of its price level, split into a
*visible* tranche of `min(display, remaining)` and a *hidden* reserve holding
the rest. In the order view `remaining` is always visible plus hidden.

While it rests:

- Only the visible tranche can be matched: a match against it trades `min(incoming remaining, visible)`.
- When a trade takes the visible tranche to zero while the reserve is not empty, the order *refills* at once: `min(display, hidden)` moves from the reserve into the visible tranche and the order goes to the back of its price level. Matching then carries on with whichever order is best now, so one incoming order can trade with the same iceberg several times, one trade per tranche, each in its turn in the queue.
- A visible tranche that is only partly taken does not refill and keeps its place.
- An iceberg with nothing visible and nothing hidden left is `"filled"`.
- `book()` adds only the visible tranche to a level's quantity (the order still counts once in the level's count); `queue` lists it at its current place.
- Self-trade prevention reaching a resting iceberg: `"cancel_newest"` leaves it untouched; `"cancel_oldest"` cancels the whole order, reserve included; `"decrement"` reduces the incoming order and the visible tranche by `min(incoming remaining, visible)`, an emptied tranche refills and goes to the back exactly as after a trade, and the iceberg becomes `"cancelled"` with reason `"stp"` (and is listed in `cancelled`) only when nothing at all is left of it.
- Post-only and fill-or-kill are judged by the same rules as always: a post-only order is rejected if it could match a visible tranche, and fill-or-kill counts exactly what matching would trade, refills included.
- `amend`: `quantity` is the new total `remaining`. When the amend keeps its place (the usual rule, comparing total `remaining`), the visible tranche becomes `min(visible, new remaining)` and the reserve holds the rest. When it loses its place, the order re-enters as an incoming order with its whole new remaining and afterwards rests with a fresh tranche of `min(display, remaining)`.
- `cancel` cancels the whole order.

### Reads

Reads take no time, never change state and never journal.

- `order(order_id)`: the order view, or `None` for an unknown id.
- `book()`: `{"bids": [[price, quantity, count], ...], "asks": [...]}`: open orders aggregated per price level, best level first (bids highest first, asks lowest first).
- `queue(side, price)`: ids of the open orders at that level, first in line first.
- `iceberg(order_id)`: `None` for an unknown id or an order that was not submitted by `submit_iceberg`; otherwise `{"display": d, "visible": v, "hidden": h}` (`v` and `h` are `0` once the order is no longer open).
- `trades()`: every trade, in order.
- `snapshot()`: `{"clock": int, "sequence": int, "orders": [<view>, ...], "trades": [...]}` with orders in submission order and `sequence` the number of accepted operations.

Everything returned is a defensive copy: mutating it never changes the engine.

## Journal

Each accepted mutator appends exactly one record, before its effect becomes
visible:

```python
{"sequence": n, "operation": "submit" | "submit_iceberg" | "amend" | "cancel" | "advance", "arguments": {...}}
```

`sequence` is contiguous from 1. `arguments` holds every public parameter of
that method by its exact name, defaults included (for example `advance` has
`{"now": 5}`). Failed operations append nothing.

If `append` raises, the same exception propagates and the operation is a
failed operation; retrying it must append an identical record. The callback
may keep or mutate the record it is given without affecting the engine.

`Engine(append, records)` replays `records` through the same validation and
state transitions, without calling `append` and without mutating `records`.
A journal is rejected as a whole with `Invalid` if any record is not a dict
with exactly those three keys, has a wrong `sequence`, an unknown
`operation`, an `arguments` dict whose keys are not exactly that method's
parameters, or an operation that would fail. After a successful replay the
engine continues from the replayed state, and its `snapshot()` equals the
snapshot of the engine that wrote the journal.
