"""Intentionally flawed starter implementation."""
from copy import deepcopy
from .errors import Busy, Conflict, Invalid, StaleClaim

class Hub:
    def __init__(self, append, records=()):
        self.append, self.sequence, self.time = append, 0, 0
        self.events, self.keys, self.groups = [], {}, {}
        for record in records:
            self._apply(record["operation"], record["arguments"])

    def _write(self, op, args):
        self.sequence += 1
        self.append({"sequence": self.sequence, "operation": op, "arguments": args})

    def _apply(self, op, a):
        return getattr(self, op)(**a)

    def publish(self, key, payload):
        if key in self.keys:
            if self.keys[key]["payload"] == payload: return self.keys[key]
            raise Conflict(key)
        event = {"offset": len(self.events)+1, "key": key, "payload": payload}
        self.events.append(event); self.keys[key] = event
        self._write("publish", {"key": key, "payload": payload})
        return event

    def create_group(self, name, start="earliest"):
        if name in self.groups: return self.groups[name]
        group = {"name": name, "committed": 0 if start == "earliest" else len(self.events),
                 "start": start, "owner": None, "generation": 0, "lease_until": None}
        self.groups[name] = group
        self._write("create_group", {"name": name, "start": start})
        return group

    def claim(self, group, consumer, now, ttl):
        item = self.groups[group]
        if item["owner"] not in (None, consumer) and item["lease_until"] > now: raise Busy(group)
        self.time = now; item["owner"] = consumer; item["generation"] += 1
        item["lease_until"] = now + ttl
        self._write("claim", {"group": group, "consumer": consumer, "now": now, "ttl": ttl})
        return item

    def _check(self, group, consumer, generation, now):
        item = self.groups[group]
        if item["owner"] != consumer or item["generation"] != generation or item["lease_until"] < now:
            raise StaleClaim(group)
        self.time = now
        return item

    def poll(self, group, consumer, generation, limit, now):
        item = self._check(group, consumer, generation, now)
        return [e for e in self.events if e["offset"] > item["committed"]][:limit]

    def ack(self, group, consumer, generation, through, now):
        item = self._check(group, consumer, generation, now); item["committed"] = through
        self._write("ack", {"group": group, "consumer": consumer, "generation": generation,
                             "through": through, "now": now})
        return item

    def release(self, group, consumer, generation, now):
        item = self._check(group, consumer, generation, now); item["owner"] = None; item["lease_until"] = None
        self._write("release", {"group": group, "consumer": consumer, "generation": generation, "now": now})
        return item

    def get_group(self, name): return self.groups.get(name)

    def snapshot(self):
        return deepcopy({"sequence": self.sequence, "time": self.time, "events": self.events,
                         "groups": list(self.groups.values())})
