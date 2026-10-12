#!/usr/bin/env python3
"""Drive a running daemon with the generated Python client, over TCP.

Called by scripts/e2e_mock.sh once the daemon is up, so the library is checked
against the real binary rather than a double. Reads AETHER_ADDRESS
(`host:port`) and AETHER_TOKEN.
"""

import os
import sys
import threading

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "clients", "python"))

from aether import AetherClient, AetherError, SubscriptionLaggedError, protocol  # noqa: E402


def fail(what):
    print(f"python client: {what}", file=sys.stderr)
    sys.exit(1)


def concurrent(client):
    """Two threads, one connection, each asking for something distinguishable.
    Each must get its own answer, and neither may block on the other's."""
    answers = {}
    failures = []

    def ask(name, command):
        try:
            answers[name] = client.request(command, timeout=30.0)
        except Exception as failed:  # noqa: BLE001 - reported, not swallowed
            failures.append(f"{name}: {failed}")

    threads = [
        threading.Thread(target=ask, args=("status", protocol.status())),
        threading.Thread(target=ask, args=("agents", protocol.list_agents(query={}))),
    ]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join(timeout=60.0)
        if thread.is_alive():
            fail("a concurrent request never returned; the connection is not multiplexed")
    if failures:
        fail(f"concurrent requests failed: {failures}")
    if answers["status"]["reply"] != "status":
        fail(f"a concurrent request got the wrong reply: {answers['status']['reply']}")
    if answers["agents"]["reply"] != "agents":
        fail(f"a concurrent request got the wrong reply: {answers['agents']['reply']}")


def schedules(client):
    """Exercise every timer command over the authenticated TCP connection."""
    agent = client.request(protocol.create_agent(role="timer-fixture", start=False))["data"]
    try:
        job = client.request(protocol.create_schedule(spec={
            "name": "TCP timer", "target": agent["id"],
            "recurrence": {"kind": "interval", "seconds": 3600},
            "payload": "Read README.md", "max_deliveries": 2,
        }))["data"]
        if client.request(protocol.get_schedule(id=job["id"]))["data"] != job:
            fail("inspecting a timer changed its data")
        if client.request(protocol.list_schedules(target=agent["id"]))["data"] != [job]:
            fail("timer target filtering lost the created timer")
        if client.request(protocol.schedule_history(id=job["id"], limit=100))["data"] != []:
            fail("a future timer already has occurrence history")
        for action, state in [("pause", "paused"), ("resume", "active"), ("cancel", "cancelled")]:
            changed = client.request(protocol.update_schedule(id=job["id"], action=action))["data"]
            if changed["status"]["state"] != state:
                fail(f"timer {action} returned {changed['status']}")
        try:
            client.request(protocol.update_schedule(id=job["id"], action="resume"))
        except AetherError as refused:
            if "cannot" not in str(refused):
                fail(f"cancelled timer resume failed unexpectedly: {refused}")
        else:
            fail("a cancelled timer resumed")
    finally:
        client.request(protocol.cancel(agent=agent["id"], reason="TCP timer check finished"))


def malformed_requests(client):
    cases = [
        (protocol.create_memory(spec={
            "kind": "fact", "scope": {"scope": "global"}, "text": "invalid", "unexpected": True,
        }), "unexpected"),
        (protocol.create_memory(spec={
            "kind": "invalid-kind", "scope": {"scope": "global"}, "text": "invalid",
        }), "invalid-kind"),
        (protocol.create_memory(spec={
            "kind": "fact", "scope": {"scope": "global"}, "text": "forged",
            "provenance": {"agent": "agt_00000000000000000000000000"},
        }), "provenance"),
    ]
    for command, problem in cases * 2:
        try:
            client.request(command, timeout=3)
        except AetherError as refused:
            if refused.code != "invalid_argument" or problem not in str(refused):
                fail(f"malformed request lost its validation diagnostic: {refused}")
        else:
            fail(f"malformed request accepted: {command}")
        if client.request(protocol.ping(), timeout=3)["reply"] != "ok":
            fail("connection stopped working after a malformed request")


def unreadable_correlation(host, port, token):
    client = AetherClient.connect_tcp(host, port, token)
    try:
        # The raw command overwrites the generated req, exercising the
        # connection-level policy while a real client has a pending caller.
        try:
            client.request({"cmd": "ping", "req": "unreadable"}, timeout=3)
        except AetherError as refused:
            if refused.code != "invalid_argument" or "req" not in str(refused):
                fail(f"unreadable correlation lost its diagnostic: {refused}")
        else:
            fail("unreadable correlation was accepted")
        try:
            client.request(protocol.ping(), timeout=3)
        except AetherError as refused:
            if refused.code != "invalid_argument" or "req" not in str(refused):
                fail(f"closed connection lost its diagnostic: {refused}")
        else:
            fail("unreadable correlation left the connection open")
    finally:
        client.close()


def main():
    address = os.environ.get("AETHER_ADDRESS")
    token = os.environ.get("AETHER_TOKEN")
    if not address or not token:
        fail("AETHER_ADDRESS and AETHER_TOKEN must be set")
    host, _, port = address.rpartition(":")

    try:
        AetherClient.connect_tcp(host, port, "wrong-token-of-the-right-length--")
    except AetherError as refused:
        if refused.code != "unauthorized":
            fail(f"a wrong token was refused as {refused.code}, not unauthorized")
    else:
        fail("a wrong token was accepted")

    client = AetherClient.connect_tcp(host, port, token)
    welcome = client.welcome
    if welcome["protocol_version"] != protocol.PROTOCOL_VERSION:
        fail(f"handshake settled on protocol {welcome['protocol_version']}")
    if welcome.get("min_protocol_version") != protocol.MIN_PROTOCOL_VERSION:
        fail("the welcome does not report the oldest version served")

    agents = client.request(protocol.list_agents(query={}))["data"]
    if not agents:
        fail("no agents; the end-to-end run should have left some")
    for agent in agents:
        for field in ("id", "role", "status", "model"):
            if field not in agent:
                fail(f"an agent came back without `{field}`")
        # The shape the examples read: the status is tagged by `state`.
        if "state" not in agent["status"]:
            fail(f"agent status is not tagged by `state`: {agent['status']}")

    # Two requests in flight on one connection. Before the client grew a
    # reader thread each call read the other's reply off the socket, parked
    # it, and both blocked until the idle timeout - so this is the check that
    # the connection is actually multiplexed.
    concurrent(client)
    schedules(client)
    malformed_requests(client)
    unreadable_correlation(host, port, token)

    # From sequence 0: everything that happened before this process existed.
    client.subscribe(after=0)
    kinds = set()
    try:
        for event in client.events(timeout=30.0):
            kinds.add(event["kind"]["type"])
            if "agent.created" in kinds and len(kinds) > 2:
                break
    except (TimeoutError, ConnectionError, SubscriptionLaggedError) as stopped:
        fail(f"the replay stopped early ({stopped}); saw {sorted(kinds)}")
    if "agent.created" not in kinds:
        fail(f"the replay carried no agent.created; saw {sorted(kinds)}")

    # The connection is tokened, so it has an idle timeout; ping is what a
    # quiet client uses to stay under it.
    client.request(protocol.ping())
    status = client.request(protocol.status())["data"]
    client.close()
    print(
        f"python client ok: {len(agents)} agents, {len(kinds)} event kinds replayed, "
        f"daemon {status['version']}"
    )


if __name__ == "__main__":
    main()
