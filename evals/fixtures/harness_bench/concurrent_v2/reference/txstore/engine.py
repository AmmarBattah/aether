from threading import RLock

from .codec import canonical, clone, decode, encode, valid_name
from .errors import Closed, Conflict, Corrupt, Invalid, ReentrantCommit


class Store:
    def __init__(self, journal=None):
        self.journal = journal
        self._state_lock = RLock()
        self._commit_lock = RLock()
        self._committing = False
        self._revision = 0
        self.data = {}
        self.versions = {}
        self.receipts = {}
        if journal is not None:
            for receipt in decode(journal.read()):
                old = self.receipts.get(receipt["txid"])
                if old is not None:
                    if canonical(old) != canonical(receipt):
                        raise Corrupt("transaction replay mismatch")
                    continue
                if receipt["revision"] != self.revision + 1:
                    raise Corrupt("noncontiguous revision")
                self._publish(receipt)

    def _publish(self, receipt):
        receipt = clone(receipt)
        for operation in receipt["writes"]:
            key = operation[1]
            if operation[0] == "put":
                self.data[key] = clone(operation[2])
            else:
                self.data.pop(key, None)
            self.versions[key] = receipt["revision"]
        self._revision = receipt["revision"]
        self.receipts[receipt["txid"]] = receipt

    def snapshot(self):
        with self._state_lock:
            return clone(self.data)

    @property
    def revision(self):
        with self._state_lock:
            return self._revision

    def begin(self, txid):
        valid_name(txid)
        with self._state_lock:
            return Transaction(self, txid)


class Transaction:
    def __init__(self, store, txid):
        self.store, self.txid = store, txid
        self.base = store.revision
        self.initial = store.snapshot()
        self.writes = {}
        self.points = set()
        self.ranges = set()
        self.saved = {}
        self.closed = False

    def _open(self):
        if self.closed:
            raise Closed("transaction closed")

    def get(self, key):
        self._open()
        valid_name(key)
        self.points.add(key)
        operation = self.writes.get(key)
        if operation:
            return clone(operation[2]) if operation[0] == "put" else None
        return clone(self.initial.get(key))

    def scan(self, prefix=""):
        self._open()
        valid_name(prefix, empty=True)
        self.ranges.add(prefix)
        view = clone(self.initial)
        for operation in self.writes.values():
            if operation[0] == "put":
                view[operation[1]] = clone(operation[2])
            else:
                view.pop(operation[1], None)
        return [(key, view[key]) for key in sorted(view) if key.startswith(prefix)]

    def put(self, key, value):
        self._open()
        valid_name(key)
        copied = clone(value)
        self.writes[key] = ["put", key, copied]

    def delete(self, key):
        self._open()
        valid_name(key)
        self.writes[key] = ["delete", key]

    def savepoint(self, name):
        self._open()
        valid_name(name)
        if name in self.saved:
            raise Invalid("duplicate savepoint")
        self.saved[name] = clone(self.writes)

    def rollback(self, name):
        self._open()
        valid_name(name)
        if name not in self.saved:
            raise Invalid("unknown savepoint")
        self.writes = clone(self.saved[name])
        later = False
        for key in list(self.saved):
            if later:
                del self.saved[key]
            if key == name:
                later = True

    def abort(self):
        self._open()
        self.closed = True

    def commit(self):
        self._open()
        with self.store._commit_lock:
            if self.store._committing:
                raise ReentrantCommit("commit from append callback")
            self.store._committing = True
            try:
                return self._commit_serial()
            finally:
                self.store._committing = False

    def _commit_serial(self):
        # Only a committer publishes shared state. The commit slot protects
        # validation while readers remain free to observe the old state.
        operations = [self.writes[key] for key in sorted(self.writes)]
        old = self.store.receipts.get(self.txid)
        if old is not None:
            if canonical(old["writes"]) != canonical(operations):
                raise Invalid("transaction id reused with different writes")
            self.closed = True
            return clone(old)
        changed = {key for key, revision in self.store.versions.items() if revision > self.base}
        if (changed & (self.points | self.writes.keys())
                or any(key.startswith(prefix) for key in changed for prefix in self.ranges)):
            raise Conflict("snapshot dependency changed")
        receipt = {"txid": self.txid, "revision": self.store.revision + 1, "writes": operations}
        record = encode(receipt)
        if self.store.journal is not None:
            self.store.journal.append(record)
        with self.store._state_lock:
            self.store._publish(receipt)
        self.closed = True
        return clone(receipt)
