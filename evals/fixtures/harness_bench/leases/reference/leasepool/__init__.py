from copy import deepcopy


class Conflict(Exception):
    pass


class StaleLease(Exception):
    pass


FIELDS = {
    "request": {"key", "resources", "priority", "ttl", "now"},
    "advance": {"now"}, "renew": {"key", "fences", "ttl", "now"},
    "release": {"key", "fences", "now"}, "cancel": {"key", "now"},
}


def integer(value, minimum=None):
    if type(value) is not int or (minimum is not None and value < minimum):
        raise ValueError("invalid integer")


def key_string(value):
    if not isinstance(value, str) or not value:
        raise ValueError("invalid key")


class Pool:
    def __init__(self, append, records=()):
        self._append = append
        self._state = dict(time=0, sequence=0, requests=[], counters={})
        for record in records:
            try:
                if not isinstance(record, dict) or set(record) != {"sequence", "operation", "arguments"}:
                    raise ValueError("invalid record")
                integer(record["sequence"], 1)
                if record["sequence"] != self._state["sequence"] + 1:
                    raise ValueError("noncontiguous journal")
                op, args = record["operation"], record["arguments"]
                if not isinstance(op, str) or op not in FIELDS or not isinstance(args, dict) or set(args) != FIELDS[op]:
                    raise ValueError("invalid operation")
                if op == "request" and args["resources"] != sorted(args["resources"]):
                    raise ValueError("noncanonical resources")
                self._apply(op, args, replay=True)
            except (Conflict, StaleLease, TypeError, KeyError) as error:
                raise ValueError("invalid journal") from error

    def _apply(self, op, arguments, replay=False):
        args = deepcopy(arguments)
        now = args["now"]
        integer(now, 0)
        if now < self._state["time"]:
            raise ValueError("backwards time")
        key = args.get("key")
        if op != "advance":
            key_string(key)
        original = next((item for item in self._state["requests"] if item["key"] == key), None)
        if op == "request":
            resources = args["resources"]
            if not isinstance(resources, list) or not resources:
                raise ValueError("invalid resources")
            for resource in resources:
                key_string(resource)
            if len(set(resources)) != len(resources):
                raise ValueError("duplicate resources")
            args["resources"] = sorted(resources)
            integer(args["priority"])
            integer(args["ttl"], 1)
            if original and any(original[name] != args[name] for name in ("resources", "priority", "ttl")):
                raise Conflict(key)
        if op in {"renew", "release"}:
            fences = args["fences"]
            if not isinstance(fences, dict):
                raise ValueError("invalid fences")
            for resource, fence in fences.items():
                key_string(resource)
                integer(fence, 1)
            if op == "renew":
                integer(args["ttl"], 1)
            if original is None or original["state"] != "active" or original["deadline"] <= now or original["fences"] != fences:
                raise StaleLease(key)
        state = deepcopy(self._state)
        state["time"] = now
        for item in state["requests"]:
            if item["state"] == "active" and item["deadline"] <= now:
                item["state"] = "expired"
        item = next((item for item in state["requests"] if item["key"] == key), None)
        cancelled = False
        if op == "request" and item is None:
            item = dict(key=key, resources=args["resources"], priority=args["priority"],
                        ttl=args["ttl"], arrival=len(state["requests"]) + 1,
                        state="queued", fences={}, deadline=None)
            state["requests"].append(item)
        elif op == "renew":
            item["deadline"] = now + args["ttl"]
        elif op == "release":
            item["state"] = "released"
        elif op == "cancel" and item and item["state"] in {"queued", "active"}:
            item["state"] = "cancelled"
            cancelled = True
        granted = self._schedule(state, now)
        state["sequence"] += 1
        record = dict(sequence=state["sequence"], operation=op, arguments=args)
        if not replay:
            self._append(deepcopy(record))
        self._state = state
        result = granted if op == "advance" else cancelled if op == "cancel" else item
        return deepcopy(result)

    @staticmethod
    def _schedule(state, now):
        occupied = {r for item in state["requests"] if item["state"] == "active" for r in item["resources"]}
        queued = sorted((item for item in state["requests"] if item["state"] == "queued"),
                        key=lambda item: (-item["priority"], item["arrival"]))
        granted = []
        for item in queued:
            blocked = occupied.intersection(item["resources"])
            occupied.update(item["resources"])
            if blocked:
                continue
            for resource in item["resources"]:
                state["counters"][resource] = state["counters"].get(resource, 0) + 1
            item.update(state="active", deadline=now + item["ttl"],
                        fences={r: state["counters"][r] for r in item["resources"]})
            granted.append(item)
        return granted

    def request(self, key, resources, priority, ttl, now):
        return self._apply("request", dict(key=key, resources=resources, priority=priority, ttl=ttl, now=now))

    def advance(self, now):
        return self._apply("advance", dict(now=now))

    def renew(self, key, fences, ttl, now):
        return self._apply("renew", dict(key=key, fences=fences, ttl=ttl, now=now))

    def release(self, key, fences, now):
        return self._apply("release", dict(key=key, fences=fences, now=now))

    def cancel(self, key, now):
        return self._apply("cancel", dict(key=key, now=now))

    def get(self, key):
        return deepcopy(next((item for item in self._state["requests"] if item["key"] == key), None))

    def snapshot(self):
        return deepcopy(self._state)
