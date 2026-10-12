"""Reference implementation for the eventlog benchmark."""
from copy import deepcopy
import math
from .errors import Busy, Conflict, Invalid, StaleClaim

_MISSING = object()

def _int(value, minimum=0):
    return type(value) is int and value >= minimum

def _name(value):
    return isinstance(value, str) and bool(value)

def _validate_json(value, seen=None):
    if value is None or type(value) in (bool, int, str): return
    if type(value) is float:
        if not math.isfinite(value): raise Invalid("non-finite number")
        return
    if seen is None: seen = set()
    ident = id(value)
    if ident in seen: raise Invalid("cyclic payload")
    if type(value) is list:
        seen.add(ident)
        for child in value: _validate_json(child, seen)
        seen.remove(ident)
        return
    if type(value) is dict:
        seen.add(ident)
        for key, child in value.items():
            if type(key) is not str: raise Invalid("object keys must be strings")
            _validate_json(child, seen)
        seen.remove(ident)
        return
    raise Invalid("payload is not JSON")

def _same(left, right):
    if type(left) is not type(right): return False
    if type(left) is dict:
        return left.keys() == right.keys() and all(_same(left[k], right[k]) for k in left)
    if type(left) in (list, tuple):
        return len(left) == len(right) and all(_same(a, b) for a, b in zip(left, right))
    return left == right

class Hub:
    def __init__(self, append, records=()):
        if not callable(append): raise Invalid("append must be callable")
        self.append = append
        self.sequence = 0
        self.time = 0
        self.events, self.keys, self.groups = [], {}, {}
        for record in records:
            self._replay_one(record)

    def _replay_one(self, record):
        if type(record) is not dict or set(record) != {"sequence", "operation", "arguments"}:
            raise Invalid("malformed journal record")
        if record["sequence"] != self.sequence + 1 or type(record["sequence"]) is not int:
            raise Invalid("non-contiguous journal sequence")
        op, args = record["operation"], record["arguments"]
        if type(op) is not str or type(args) is not dict:
            raise Invalid("malformed journal operation")
        try:
            result = self._execute(op, deepcopy(args), replay=True)
        except (Invalid, Conflict, Busy, StaleClaim, KeyError, TypeError) as exc:
            raise Invalid("invalid journal operation") from exc
        self.sequence += 1
        return result

    def _execute(self, op, args, replay=False):
        expected = {
            "publish": {"key", "payload"}, "create_group": {"name", "start"},
            "claim": {"group", "consumer", "now", "ttl"},
            "ack": {"group", "consumer", "generation", "through", "now"},
            "release": {"group", "consumer", "generation", "now"},
        }
        if op not in expected or set(args) != expected[op]: raise Invalid("invalid record keys")
        if op == "publish": return self._publish(args["key"], args["payload"], replay)
        if op == "create_group": return self._create_group(args["name"], args["start"], replay)
        if op == "claim": return self._claim(args["group"], args["consumer"], args["now"], args["ttl"], replay)
        if op == "ack": return self._ack(args["group"], args["consumer"], args["generation"], args["through"], args["now"], replay)
        return self._release(args["group"], args["consumer"], args["generation"], args["now"], replay)

    def _record(self, op, args, replay):
        if replay:
            return self._execute(op, args, True)
        candidate = object.__new__(Hub)
        candidate.append = self.append
        candidate.sequence = self.sequence
        candidate.time = self.time
        candidate.events = deepcopy(self.events)
        candidate.keys = deepcopy(self.keys)
        candidate.groups = deepcopy(self.groups)
        result = candidate._execute(op, deepcopy(args), True)
        record = {"sequence": self.sequence + 1, "operation": op,
                  "arguments": deepcopy(args)}
        self.append(deepcopy(record))
        self.time, self.events, self.keys, self.groups = (
            candidate.time, candidate.events, candidate.keys, candidate.groups)
        self.sequence += 1
        return deepcopy(result)

    def publish(self, key, payload): return self._record("publish", {"key": key, "payload": payload}, False)
    def create_group(self, name, start="earliest"):
        return self._record("create_group", {"name": name, "start": start}, False)
    def claim(self, group, consumer, now, ttl):
        return self._record("claim", {"group": group, "consumer": consumer, "now": now, "ttl": ttl}, False)
    def ack(self, group, consumer, generation, through, now):
        return self._record("ack", {"group": group, "consumer": consumer, "generation": generation,
                                     "through": through, "now": now}, False)
    def release(self, group, consumer, generation, now):
        return self._record("release", {"group": group, "consumer": consumer,
                                         "generation": generation, "now": now}, False)

    def _publish(self, key, payload, replay):
        if not _name(key): raise Invalid("invalid event key")
        _validate_json(payload)
        existing = self.keys.get(key)
        if existing is not None:
            if _same(existing["payload"], payload): return existing
            raise Conflict(key)
        event = {"offset": len(self.events) + 1, "key": key, "payload": deepcopy(payload)}
        def apply(): self.events.append(event); self.keys[key] = event; return event
        return apply() if replay else self._commit("publish", {"key": key, "payload": payload}, apply)

    def _create_group(self, name, start, replay):
        if not _name(name) or start not in ("earliest", "latest") or type(start) is not str:
            raise Invalid("invalid group")
        old = self.groups.get(name)
        if old is not None:
            if old["start"] == start: return old
            raise Conflict(name)
        item = {"name": name, "committed": 0 if start == "earliest" else len(self.events),
                "start": start, "owner": None, "generation": 0, "lease_until": None}
        def apply(): self.groups[name] = item; return item
        return apply() if replay else self._commit("create_group", {"name": name, "start": start}, apply)

    def _check_time(self, now):
        if not _int(now) or now < self.time: raise Invalid("invalid or backwards time")

    def _claim(self, group, consumer, now, ttl, replay):
        if not _name(group) or not _name(consumer) or not _int(ttl, 1): raise Invalid("invalid claim")
        self._check_time(now)
        item = self.groups.get(group)
        if item is None: raise Invalid("unknown group")
        active = item["owner"] is not None and item["lease_until"] > now
        if active and item["owner"] != consumer: raise Busy(group)
        generation = item["generation"] if active else item["generation"] + 1
        updated = dict(item, owner=consumer, generation=generation, lease_until=now + ttl)
        def apply(): self.groups[group] = updated; self.time = now; return updated
        return apply() if replay else self._commit("claim", {"group": group, "consumer": consumer, "now": now, "ttl": ttl}, apply)

    def _authorized(self, group, consumer, generation, now):
        self._check_time(now)
        if not _name(group) or not _name(consumer) or not _int(generation): raise Invalid("invalid claim token")
        item = self.groups.get(group)
        if item is None or item["owner"] != consumer or item["generation"] != generation or item["lease_until"] <= now:
            raise StaleClaim(group)
        return item

    def poll(self, group, consumer, generation, limit, now):
        if not _int(limit, 1): raise Invalid("invalid poll limit")
        self._check_time(now)
        item = self._authorized(group, consumer, generation, now)
        return deepcopy([event for event in self.events if event["offset"] > item["committed"]][:limit])

    def _ack(self, group, consumer, generation, through, now, replay):
        item = self._authorized(group, consumer, generation, now)
        if not _int(through) or through < item["committed"] or through > len(self.events):
            raise Invalid("invalid cursor")
        updated = dict(item, committed=through)
        args = {"group": group, "consumer": consumer, "generation": generation, "through": through, "now": now}
        def apply(): self.groups[group] = updated; self.time = now; return updated
        return apply() if replay else self._commit("ack", args, apply)

    def _release(self, group, consumer, generation, now, replay):
        item = self._authorized(group, consumer, generation, now)
        updated = dict(item, owner=None, lease_until=None)
        args = {"group": group, "consumer": consumer, "generation": generation, "now": now}
        def apply(): self.groups[group] = updated; self.time = now; return updated
        return apply() if replay else self._commit("release", args, apply)

    def get_group(self, name): return deepcopy(self.groups.get(name))
    def snapshot(self):
        return deepcopy({"sequence": self.sequence, "time": self.time,
                         "events": self.events, "groups": list(self.groups.values())})
