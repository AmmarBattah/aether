"""Shared helpers for the held-out matchbook checks."""
from matchbook import Engine


class Journal:
    def __init__(self):
        self.rows = []
        self.fail = None
        self.scribble = False

    def append(self, row):
        if self.fail is not None:
            raise self.fail
        self.rows.append(row)
        if self.scribble:
            self.rows[-1] = dict(row)
            row.clear()


def engine():
    journal = Journal()
    return Engine(journal.append), journal


def status(eng, order_id):
    view = eng.order(order_id)
    return view["status"], view["reason"], view["filled"], view["remaining"]


def pairs(trades):
    return [(t["buy"], t["sell"], t["price"], t["quantity"]) for t in trades]
