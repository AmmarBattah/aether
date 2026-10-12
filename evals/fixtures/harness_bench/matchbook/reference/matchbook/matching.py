"""Price-time matching with self-trade prevention and fill-or-kill."""
import copy

from .state import close, crosses, refill_if_emptied, resting, show_tranche, visible

OPPOSITE = {"buy": "sell", "sell": "buy"}


def _trade(state, incoming, other, now):
    quantity = min(incoming["remaining"], visible(other))
    assert quantity > 0, "a trade needs quantity on both sides"
    assert incoming["owner"] != other["owner"], "self-trade must be prevented before trading"
    buy, sell = (incoming, other) if incoming["side"] == "buy" else (other, incoming)
    trade = {"id": len(state.trades) + 1, "time": now, "price": other["price"], "quantity": quantity,
             "buy": buy["id"], "sell": sell["id"], "taker": incoming["id"]}
    state.trades.append(trade)
    for order in (incoming, other):
        order["remaining"] -= quantity
        order["filled"] += quantity
    if other["remaining"] == 0:
        close(other, "filled")
    refill_if_emptied(state, other)
    return trade


def _prevent_self_trade(state, incoming, other, cancelled):
    """Apply the incoming order's stp; returns False when matching must stop."""
    assert incoming["owner"] == other["owner"], "stp applies only to the same owner"
    assert other["status"] == "open", "stp met an order that is not resting"
    if incoming["stp"] == "cancel_newest":
        close(incoming, "cancelled", "stp")
        return False
    if incoming["stp"] == "cancel_oldest":
        close(other, "cancelled", "stp")
        cancelled.append(other["id"])
        return True
    reduce_by = min(incoming["remaining"], visible(other))
    for order in (incoming, other):
        order["remaining"] -= reduce_by
    if other["remaining"] == 0:
        close(other, "cancelled", "stp")
        cancelled.append(other["id"])
    refill_if_emptied(state, other)
    if incoming["remaining"] == 0:
        close(incoming, "cancelled", "stp")
        return False
    return True


def match(state, incoming, now):
    """Match an order that holds no queue place; returns (trades, cancelled ids)."""
    assert incoming["place"] is None, "an incoming order must not be in the queue"
    assert incoming["hidden"] == 0, "an incoming order matches with its whole quantity"
    assert incoming["status"] == "open" and incoming["remaining"] > 0, "nothing to match"
    trades, cancelled = [], []
    while incoming["status"] == "open" and incoming["remaining"] > 0:
        book = resting(state, OPPOSITE[incoming["side"]])
        if not book or not crosses(incoming["side"], incoming["price"], book[0]["price"]):
            break
        if book[0]["owner"] == incoming["owner"]:
            if not _prevent_self_trade(state, incoming, book[0], cancelled):
                break
            continue
        trades.append(_trade(state, incoming, book[0], now))
    if incoming["status"] == "open" and incoming["remaining"] == 0:
        close(incoming, "filled")
    return trades, cancelled


def would_fill_completely(state, incoming, now):
    """Dry-run the match on a copy: fill-or-kill needs the whole quantity traded."""
    assert incoming["tif"] == "FOK", "only fill-or-kill is judged before it runs"
    trial_state = copy.deepcopy(state)
    trial = trial_state.orders[incoming["id"]]
    wanted = trial["remaining"]
    assert wanted > 0, "a fill-or-kill order needs quantity"
    trades, _ = match(trial_state, trial, now)
    return sum(trade["quantity"] for trade in trades) == wanted


def settle(state, incoming):
    """After matching: rest what is left, or cancel an immediate-or-cancel rest."""
    assert incoming["place"] is None, "settling an order that already rests"
    if incoming["status"] != "open":
        return
    assert incoming["remaining"] > 0, "an open order must have quantity left"
    if incoming["tif"] in ("GTC", "GTD"):
        incoming["place"] = state.take_place()
        if incoming["display"] is not None:
            show_tranche(incoming, incoming["display"])
    else:
        close(incoming, "cancelled", "ioc")
