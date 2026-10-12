"""A first cut of the matching engine. See CONTRACT.md for what it must do."""
from .book import Book
from .errors import Closed, Conflict, Invalid, NotFound
from .journal import make_record, read_record


class Engine:
    def __init__(self, append=None, records=()):
        self._append = append
        self._book = Book()
        self._trades = []
        self._clock = 0
        self._sequence = 0
        for record in records:
            operation, arguments = read_record(record)
            getattr(self, "_" + operation)(**arguments)
            self._sequence += 1

    def _journal(self, operation, arguments):
        self._sequence += 1
        if self._append is not None:
            self._append(make_record(self._sequence, operation, arguments))

    def submit(self, order_id, owner, side, price, quantity, now, tif="GTC", expires=None,
               post_only=False, stp="cancel_newest"):
        arguments = {"order_id": order_id, "owner": owner, "side": side, "price": price, "quantity": quantity,
                     "now": now, "tif": tif, "expires": expires, "post_only": post_only, "stp": stp}
        report = self._submit(**arguments)
        self._journal("submit", arguments)
        return report

    def submit_iceberg(self, order_id, owner, side, price, quantity, display, now, stp="cancel_newest"):
        # The whole quantity is shown for now; `display` is not honoured yet.
        report = self._submit(order_id, owner, side, price, quantity, now, "GTC", None, False, stp)
        self._journal("submit", {"order_id": order_id, "owner": owner, "side": side, "price": price,
                                 "quantity": quantity, "now": now, "tif": "GTC", "expires": None,
                                 "post_only": False, "stp": stp})
        return report

    def _submit(self, order_id, owner, side, price, quantity, now, tif, expires, post_only, stp):
        if side not in ("buy", "sell") or not price or quantity <= 0:
            raise Invalid("bad order")
        if order_id in self._book.orders:
            raise Conflict(order_id)
        self._clock = now
        order = {"id": order_id, "owner": owner, "side": side, "price": price, "quantity": quantity,
                 "filled": 0, "remaining": quantity, "tif": tif, "expires": expires,
                 "status": "open", "reason": None}
        trades = []
        for other in self._book.open_orders("sell" if side == "buy" else "buy"):
            crossing = other["price"] <= price if side == "buy" else other["price"] >= price
            if not crossing or order["remaining"] == 0:
                break
            traded = min(order["remaining"], other["remaining"])
            buy, sell = (order, other) if side == "buy" else (other, order)
            trade = {"id": len(self._trades) + 1, "time": now, "price": price, "quantity": traded,
                     "buy": buy["id"], "sell": sell["id"], "taker": order_id}
            for party in (order, other):
                party["remaining"] -= traded
                party["filled"] += traded
                if party["remaining"] == 0:
                    party["status"] = "filled"
            trades.append(trade)
            self._trades.append(trade)
        self._book.add(order)
        return {"order": order, "trades": trades, "cancelled": [], "expired": []}

    def amend(self, order_id, owner, now, price=None, quantity=None):
        arguments = {"order_id": order_id, "owner": owner, "now": now, "price": price, "quantity": quantity}
        report = self._amend(**arguments)
        self._journal("amend", arguments)
        return report

    def _amend(self, order_id, owner, now, price, quantity):
        order = self._find_open(order_id)
        self._clock = now
        if price is not None:
            order["price"] = price
        if quantity is not None:
            order["remaining"] = quantity
        return {"order": order, "trades": [], "cancelled": [], "expired": []}

    def cancel(self, order_id, owner, now):
        arguments = {"order_id": order_id, "owner": owner, "now": now}
        report = self._cancel(**arguments)
        self._journal("cancel", arguments)
        return report

    def _cancel(self, order_id, owner, now):
        order = self._find_open(order_id)
        self._clock = now
        order.update(status="cancelled", reason="user", remaining=0)
        return {"order": order, "trades": [], "cancelled": [], "expired": []}

    def advance(self, now):
        self._advance(now)
        self._journal("advance", {"now": now})
        return {"order": None, "trades": [], "cancelled": [], "expired": []}

    def _advance(self, now):
        self._clock = now

    def _find_open(self, order_id):
        order = self._book.orders.get(order_id)
        if order is None:
            raise NotFound(order_id)
        if order["status"] != "open":
            raise Closed(order_id)
        return order

    def order(self, order_id):
        return self._book.orders.get(order_id)

    def book(self):
        return {"bids": self._book.levels("buy"), "asks": self._book.levels("sell")}

    def iceberg(self, order_id):
        return None

    def queue(self, side, price):
        return [o["id"] for o in self._book.open_orders(side) if o["price"] == price]

    def trades(self):
        return self._trades

    def snapshot(self):
        return {"clock": self._clock, "sequence": self._sequence,
                "orders": list(self._book.orders.values()), "trades": self._trades}
