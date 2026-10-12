import hashlib
import json


class Journal:
    def __init__(self, data=b""):
        self.data = data
        self.calls = []
        self.reads = 0
        self.failure = None

    def read(self):
        self.reads += 1
        return self.data

    def append(self, record):
        if self.failure is not None:
            raise self.failure
        self.calls.append(record)
        self.data += record


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=False, allow_nan=False).encode()


def frame(payload):
    return f"{len(payload):08x}:".encode() + payload + b":" + hashlib.sha256(payload).hexdigest().encode() + b"\n"


def receipt(txid="r", revision=1, writes=None):
    return {"txid": txid, "revision": revision, "writes": [] if writes is None else writes}


def put(store, txid, key, value):
    tx = store.begin(txid)
    tx.put(key, value)
    return tx.commit()


def delete(store, txid, key):
    tx = store.begin(txid)
    tx.delete(key)
    return tx.commit()
