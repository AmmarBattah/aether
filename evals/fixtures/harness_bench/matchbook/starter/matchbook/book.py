"""Resting orders of the book."""


class Book:
    def __init__(self):
        self.orders = {}

    def add(self, order):
        self.orders[order["id"]] = order

    def open_orders(self, side):
        found = [o for o in self.orders.values() if o["side"] == side and o["status"] == "open"]
        return sorted(found, key=lambda o: o["price"], reverse=(side == "buy"))

    def levels(self, side):
        rows = []
        for order in self.open_orders(side):
            if rows and rows[-1][0] == order["price"]:
                rows[-1][1] += order["remaining"]
                rows[-1][2] += 1
            else:
                rows.append([order["price"], order["remaining"], 1])
        return rows
