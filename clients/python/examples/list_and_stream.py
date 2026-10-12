#!/usr/bin/env python3
"""List the fleet's agents, then stream events as they happen.

Over the Unix socket by default; set AETHER_ADDRESS=host:port and
AETHER_TOKEN to reach a daemon over TCP instead.

    python3 clients/python/examples/list_and_stream.py
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from aether import AetherClient, SubscriptionLaggedError, protocol  # noqa: E402


def main():
    client = AetherClient.connect()
    welcome = client.welcome
    print(
        f"daemon {welcome['daemon_version']} · protocol {welcome['protocol_version']}"
        f" (serves {welcome.get('min_protocol_version')}"
        f"-{welcome.get('daemon_protocol_version')}) · instance {welcome['instance_id']}"
    )

    agents = client.request(protocol.list_agents(query={}))["data"]
    print(f"\n{len(agents)} agents")
    for agent in agents:
        status = agent["status"]["state"]
        print(f"  {agent['id']}  {agent['role']:<14} {status:<10} {agent['model']['model']}")

    # From sequence 0 the daemon replays the whole log first, so a client that
    # just connected sees what it missed before it sees anything new.
    last_seq = 0
    print("\nevents (ctrl-c to stop)")
    try:
        while True:
            subscription = client.subscribe(after=last_seq)
            try:
                for event in subscription.events(timeout=3600.0):
                    print(f"  {event['at']}  {event['kind']['type']}  {event.get('agent_id') or ''}")
            except SubscriptionLaggedError as lag:
                last_seq = lag.last_seq
    except KeyboardInterrupt:
        pass
    finally:
        client.close()


if __name__ == "__main__":
    main()
