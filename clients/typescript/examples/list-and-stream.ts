#!/usr/bin/env node
// List the daemon's agents, then stream events live. Run with no build
// step: `node --experimental-strip-types examples/list-and-stream.ts`
// (node 22.6+), from clients/typescript/.
//
// Reads AETHER_ADDRESS ("host:port") plus AETHER_TOKEN for the TCP
// transport; falls back to the Unix socket when AETHER_ADDRESS is unset.

import { AetherClient } from "../src/client.ts";
import { listAgents } from "../src/protocol.ts";

function splitHostPort(address: string): [string, number] {
  const colon = address.lastIndexOf(":");
  if (colon < 0) {
    throw new Error(`AETHER_ADDRESS must be "host:port", got "${address}"`);
  }
  return [address.slice(0, colon), Number(address.slice(colon + 1))];
}

async function connect(): Promise<AetherClient> {
  const address = process.env.AETHER_ADDRESS;
  if (!address) {
    return AetherClient.connectUnix();
  }
  const token = process.env.AETHER_TOKEN;
  if (!token) {
    throw new Error("AETHER_ADDRESS is set but AETHER_TOKEN is not; the TCP transport requires a token");
  }
  const [host, port] = splitHostPort(address);
  return AetherClient.connectTcp(host, port, token);
}

async function main(): Promise<void> {
  const client = await connect();
  console.log(`connected: daemon ${client.welcome.daemon_version}, instance ${client.welcome.instance_id}`);

  const agents = await client.request(listAgents({ query: {} }));
  if (agents.reply !== "agents") {
    throw new Error(`expected an "agents" reply, got "${agents.reply}"`);
  }
  for (const agent of agents.data) {
    console.log(`agent  ${agent.id}  ${agent.name}  ${agent.status.state}`);
  }

  const sub = await client.subscribe({ query: {}, deltas: false });
  console.log(`subscribed (id=${sub.id}, last_seq=${sub.lastSeq}); streaming events, ctrl-c to stop`);

  process.on("SIGINT", () => {
    client.close();
    process.exit(0);
  });

  for await (const frame of sub) {
    if (frame.kind !== "event") continue;
    console.log(`event  seq=${frame.event.seq}  ${frame.event.kind.type}`);
  }
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  process.exitCode = 1;
});
