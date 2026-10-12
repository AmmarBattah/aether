"""The matching engine: validate, apply to a copy, journal, then publish."""
import copy

from . import validate
from .errors import Closed, Conflict, Forbidden, Invalid, NotFound
from .journal import make_record, parse_record
from .matching import OPPOSITE, match, settle, would_fill_completely
from .state import State, close, crosses, expire_due, resting, show_tranche, view, visible


def _report(order=None, trades=(), cancelled=(), expired=()):
    return {"order": None if order is None else view(order), "trades": copy.deepcopy(list(trades)),
            "cancelled": list(cancelled), "expired": list(expired)}


def _owned_open_order(state, args):
    order = state.orders.get(args["order_id"])
    if order is None:
        raise NotFound(args["order_id"])
    if order["owner"] != args["owner"]:
        raise Forbidden(args["order_id"])
    due = order["expires"] is not None and order["expires"] <= args["now"]
    if order["status"] != "open" or due:
        raise Closed(args["order_id"])
    return order


def _apply_submit(state, args):
    validate.check_submit(args, state.clock)
    if args["order_id"] in state.orders:
        raise Conflict(args["order_id"])
    now = args["now"]
    state.clock = now
    expired = expire_due(state, now)
    order = {"id": args["order_id"], "owner": args["owner"], "side": args["side"], "price": args["price"],
             "quantity": args["quantity"], "filled": 0, "remaining": args["quantity"], "tif": args["tif"],
             "expires": args["expires"], "status": "open", "reason": None, "stp": args["stp"], "place": None,
             "display": None, "hidden": 0}
    state.orders[order["id"]] = order
    opposite = resting(state, OPPOSITE[order["side"]])
    if args["post_only"] and opposite and crosses(order["side"], order["price"], opposite[0]["price"]):
        close(order, "rejected", "post_only")
        return _report(order, expired=expired)
    if order["tif"] == "FOK" and not would_fill_completely(state, order, now):
        close(order, "rejected", "fok")
        return _report(order, expired=expired)
    trades, cancelled = match(state, order, now)
    settle(state, order)
    return _report(order, trades, cancelled, expired)


def _apply_submit_iceberg(state, args):
    validate.check_submit_iceberg(args, state.clock)
    if args["order_id"] in state.orders:
        raise Conflict(args["order_id"])
    now = args["now"]
    state.clock = now
    expired = expire_due(state, now)
    order = {"id": args["order_id"], "owner": args["owner"], "side": args["side"], "price": args["price"],
             "quantity": args["quantity"], "filled": 0, "remaining": args["quantity"], "tif": "GTC",
             "expires": None, "status": "open", "reason": None, "stp": args["stp"], "place": None,
             "display": args["display"], "hidden": 0}
    state.orders[order["id"]] = order
    trades, cancelled = match(state, order, now)
    settle(state, order)
    return _report(order, trades, cancelled, expired)


def _apply_amend(state, args):
    validate.check_amend(args, state.clock)
    order = _owned_open_order(state, args)
    now = args["now"]
    state.clock = now
    expired = expire_due(state, now)
    assert order["status"] == "open", "the amended order cannot be expired by its own operation"
    price = order["price"] if args["price"] is None else args["price"]
    remaining = order["remaining"] if args["quantity"] is None else args["quantity"]
    keeps_place = price == order["price"] and remaining <= order["remaining"]
    shown = visible(order)
    order.update(price=price, remaining=remaining, quantity=order["filled"] + remaining, hidden=0)
    if keeps_place:
        show_tranche(order, shown)
        return _report(order, expired=expired)
    order["place"] = None
    trades, cancelled = match(state, order, now)
    settle(state, order)
    return _report(order, trades, cancelled, expired)


def _apply_cancel(state, args):
    validate.check_cancel(args, state.clock)
    order = _owned_open_order(state, args)
    state.clock = args["now"]
    expired = expire_due(state, args["now"])
    assert order["status"] == "open", "the cancelled order cannot be expired by its own operation"
    close(order, "cancelled", "user")
    return _report(order, expired=expired)


def _apply_advance(state, args):
    validate.check_advance(args, state.clock)
    state.clock = args["now"]
    return _report(expired=expire_due(state, args["now"]))


_APPLY = {"submit": _apply_submit, "submit_iceberg": _apply_submit_iceberg, "amend": _apply_amend,
          "cancel": _apply_cancel, "advance": _apply_advance}


class Engine:
    def __init__(self, append=None, records=()):
        self._append = append
        self._state = State()
        for record in records:
            operation, arguments = parse_record(record, self._state.sequence + 1)
            try:
                _APPLY[operation](self._state, arguments)
            except (Conflict, NotFound, Forbidden, Closed) as error:
                raise Invalid(f"journal record {self._state.sequence + 1} cannot be replayed") from error
            self._state.sequence += 1

    def _execute(self, operation, arguments):
        """Apply to a copy, journal, and only then publish the new state."""
        assert tuple(arguments) == validate.PARAMETERS[operation], "arguments must follow the signature"
        staged = copy.deepcopy(self._state)
        report = _APPLY[operation](staged, copy.deepcopy(arguments))
        staged.sequence += 1
        assert staged.sequence == self._state.sequence + 1, "one accepted operation is one sequence step"
        if self._append is not None:
            self._append(make_record(staged.sequence, operation, arguments))
        self._state = staged
        return report

    def submit(self, order_id, owner, side, price, quantity, now, tif="GTC", expires=None,
               post_only=False, stp="cancel_newest"):
        return self._execute("submit", {
            "order_id": order_id, "owner": owner, "side": side, "price": price, "quantity": quantity,
            "now": now, "tif": tif, "expires": expires, "post_only": post_only, "stp": stp})

    def submit_iceberg(self, order_id, owner, side, price, quantity, display, now, stp="cancel_newest"):
        return self._execute("submit_iceberg", {
            "order_id": order_id, "owner": owner, "side": side, "price": price, "quantity": quantity,
            "display": display, "now": now, "stp": stp})

    def amend(self, order_id, owner, now, price=None, quantity=None):
        return self._execute("amend", {"order_id": order_id, "owner": owner, "now": now,
                                       "price": price, "quantity": quantity})

    def cancel(self, order_id, owner, now):
        return self._execute("cancel", {"order_id": order_id, "owner": owner, "now": now})

    def advance(self, now):
        return self._execute("advance", {"now": now})

    def order(self, order_id):
        order = self._state.orders.get(order_id) if isinstance(order_id, str) else None
        return None if order is None else view(order)

    def book(self):
        levels = {}
        for side, name in (("buy", "bids"), ("sell", "asks")):
            rows = []
            for order in resting(self._state, side):
                if rows and rows[-1][0] == order["price"]:
                    rows[-1][1] += visible(order)
                    rows[-1][2] += 1
                else:
                    rows.append([order["price"], visible(order), 1])
            levels[name] = rows
        return levels

    def iceberg(self, order_id):
        order = self._state.orders.get(order_id) if isinstance(order_id, str) else None
        if order is None or order["display"] is None:
            return None
        return {"display": order["display"], "visible": visible(order), "hidden": order["hidden"]}

    def queue(self, side, price):
        if side not in ("buy", "sell"):
            raise Invalid("side must be buy or sell")
        return [order["id"] for order in resting(self._state, side) if order["price"] == price]

    def trades(self):
        return copy.deepcopy(self._state.trades)

    def snapshot(self):
        return {"clock": self._state.clock, "sequence": self._state.sequence,
                "orders": [view(order) for order in self._state.orders.values()],
                "trades": copy.deepcopy(self._state.trades)}
