"""Book state: orders in submission order, trades, clock and queue places."""

VIEW_FIELDS = ("id", "owner", "side", "price", "quantity", "filled", "remaining",
               "tif", "expires", "status", "reason")


class State:
    def __init__(self):
        self.orders = {}
        self.trades = []
        self.clock = 0
        self.sequence = 0
        self.next_place = 1

    def take_place(self):
        place = self.next_place
        self.next_place += 1
        return place


def view(order):
    return {field: order[field] for field in VIEW_FIELDS}


def close(order, status, reason=None):
    assert order["status"] == "open", f"closing an order that is already {order['status']}"
    assert status in ("filled", "cancelled", "expired", "rejected"), status
    order.update(status=status, reason=reason, remaining=0, hidden=0)


def visible(order):
    """Quantity that can be matched now; an iceberg keeps the rest in its hidden reserve."""
    assert 0 <= order["hidden"] <= order["remaining"], "the reserve cannot exceed what remains"
    return order["remaining"] - order["hidden"]


def show_tranche(order, shown):
    """Split what remains into a visible tranche of at most `shown` and the hidden reserve."""
    assert order["status"] == "open" and order["remaining"] > 0, "only a live order shows a tranche"
    assert shown > 0, "a tranche must show something"
    order["hidden"] = order["remaining"] - min(shown, order["remaining"])


def refill_if_emptied(state, order):
    """An iceberg whose visible tranche is gone shows the next one from the back of its level."""
    if order["status"] != "open" or visible(order) > 0:
        return
    assert order["display"] is not None and order["hidden"] > 0, "only an iceberg has a reserve to refill from"
    show_tranche(order, order["display"])
    order["place"] = state.take_place()


def expire_due(state, now):
    """Expire every open order due at `now`; ids come back in submission order."""
    assert now >= state.clock, "expiry must not run backwards in time"
    expired = []
    for order in state.orders.values():
        if order["status"] == "open" and order["expires"] is not None and order["expires"] <= now:
            close(order, "expired")
            expired.append(order["id"])
    return expired


def resting(state, side):
    """Open orders of one side that hold a queue place, best first."""
    assert side in ("buy", "sell"), side
    sign = -1 if side == "buy" else 1
    found = [o for o in state.orders.values()
             if o["status"] == "open" and o["side"] == side and o["place"] is not None]
    return sorted(found, key=lambda o: (sign * o["price"], o["place"]))


def crosses(side, limit, resting_price):
    assert side in ("buy", "sell"), side
    if limit is None:
        return True
    return resting_price <= limit if side == "buy" else resting_price >= limit
