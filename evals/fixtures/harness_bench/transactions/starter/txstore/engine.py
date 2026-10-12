"""Optimistic transactional store; repair the protocol to match CONTRACT.md."""
from .codec import decode, encode
from .errors import Closed, Conflict, Invalid


class Store:
    def __init__(self, journal=None):
        self.journal = journal
        self.revision = 0
        self.data = {}
        self.receipts = {}
        if journal is not None:
            for receipt in decode(journal.read()):
                self._publish(receipt)

    def _publish(self, receipt):
        for operation in receipt["writes"]:
            if operation[0] == "put":
                self.data[operation[1]] = operation[2]
            else:
                self.data.pop(operation[1], None)
        self.revision = receipt["revision"]
        self.receipts[receipt["txid"]] = receipt

    def snapshot(self):
        return dict(self.data)

    def begin(self, txid):
        return Transaction(self, txid)


class Transaction:
    def __init__(self, store, txid):
        self.store, self.txid = store, txid
        self.base = store.revision
        self.writes = {}
        self.points = set()
        self.saved = {}
        self.closed = False

    def get(self, key):
        self.points.add(key)
        operation = self.writes.get(key)
        if operation:
            return operation[2] if operation[0] == "put" else None
        return self.store.data.get(key)

    def scan(self, prefix=""):
        return sorted((k, v) for k, v in self.store.data.items() if k.startswith(prefix))

    def put(self, key, value):
        self.writes[key] = ["put", key, value]

    def delete(self, key):
        self.writes[key] = ["delete", key]

    def savepoint(self, name):
        self.saved[name] = dict(self.writes)

    def rollback(self, name):
        self.writes = self.saved[name]

    def abort(self):
        self.closed = True

    def commit(self):
        if self.closed:
            raise Closed()
        if self.txid in self.store.receipts:
            return self.store.receipts[self.txid]
        if self.points and self.base != self.store.revision:
            raise Conflict()
        receipt = {"txid": self.txid, "revision": self.store.revision + 1,
                   "writes": [self.writes[k] for k in sorted(self.writes)]}
        self.store._publish(receipt)
        if self.store.journal is not None:
            self.store.journal.append(encode(receipt))
        self.closed = True
        return receipt
