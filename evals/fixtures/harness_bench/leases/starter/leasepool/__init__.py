"""Resource allocation with a durable operation journal."""
from copy import deepcopy


class Conflict(Exception):
    pass


class StaleLease(Exception):
    pass


class Pool:
    def __init__(self, append, records=()):
        self.append = append
        self.time = 0
        self.sequence = 0
        self.requests = {}
        self.counters = {}
        for record in records:
            getattr(self, record["operation"])(**record["arguments"])

    def _record(self, operation, arguments):
        self.sequence += 1
        self.append({"sequence": self.sequence, "operation": operation,
                     "arguments": arguments})

    def request(self, key, resources, priority, ttl, now):
        if key in self.requests:
            return self.requests[key]
        item = dict(key=key, resources=sorted(resources), priority=priority,
                    ttl=ttl, arrival=len(self.requests) + 1, state="queued",
                    fences={}, deadline=None)
        self.requests[key] = item
        self._grant(now)
        self._record("request", dict(key=key, resources=resources,
                                    priority=priority, ttl=ttl, now=now))
        return item

    def _grant(self, now):
        self.time = now
        occupied = {r for item in self.requests.values() if item["state"] == "active"
                    for r in item["resources"]}
        granted = []
        for item in self.requests.values():
            if item["state"] == "queued" and not occupied.intersection(item["resources"]):
                for resource in item["resources"]:
                    self.counters[resource] = self.counters.get(resource, 0) + 1
                item.update(state="active", deadline=now + item["ttl"],
                            fences={r: self.counters[r] for r in item["resources"]})
                occupied.update(item["resources"])
                granted.append(item)
        return granted

    def advance(self, now):
        granted = self._grant(now)
        self._record("advance", dict(now=now))
        return granted

    def renew(self, key, fences, ttl, now):
        item = self.requests[key]
        item["deadline"] = now + ttl
        self._record("renew", dict(key=key, fences=fences, ttl=ttl, now=now))
        return item

    def release(self, key, fences, now):
        item = self.requests[key]
        item["state"] = "released"
        self._grant(now)
        self._record("release", dict(key=key, fences=fences, now=now))
        return item

    def cancel(self, key, now):
        item = self.requests.get(key)
        if item is None:
            return False
        item["state"] = "cancelled"
        self._grant(now)
        self._record("cancel", dict(key=key, now=now))
        return True

    def get(self, key):
        return self.requests.get(key)

    def snapshot(self):
        return deepcopy(dict(time=self.time, sequence=self.sequence,
                             requests=list(self.requests.values()), counters=self.counters))
