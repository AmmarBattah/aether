# @aether/client

A client for the Aether daemon protocol.

`src/protocol.ts` is the public facade for the generated modules in
`src/protocol/`. Both are generated from the Rust protocol types by
`scripts/generate_clients.py` and must not be edited: change the types and
regenerate. Keep that directory when copying the client; existing facade imports
are unchanged. `src/client.ts` is the hand-written connection on top of it -
handshake, request/reply multiplexing over one socket, subscriptions, and
the keepalive a tokened TCP transport needs.

```ts
import { AetherClient } from "./src/client.ts";
import { listAgents } from "./src/protocol.ts";

const client = await AetherClient.connectUnix();                       // the daemon's socket
// const client = await AetherClient.connectTcp("127.0.0.1", 7777, token);  // or over TCP

const agents = await client.request(listAgents({ query: {} }));
if (agents.reply === "agents") {
  for (const agent of agents.data) console.log(agent.id, agent.name);
}

const sub = await client.subscribe({ query: {}, deltas: false }); // after: 0 replays the log, omitted starts live
for await (const frame of sub) {
  if (frame.kind === "event") console.log(frame.event.kind.type);
}
```

Subscriptions buffer at most 1024 frames. On overflow the daemon subscription
is stopped; iteration drains the buffered prefix, then rejects with
`SubscriptionLaggedError`. Resume using `subscribe({ after: lastSeq, ... })`,
where `lastSeq` is the last event sequence you consumed (use `sub.lastSeq`
until the first event). Live deltas cannot be
replayed. Other subscriptions and requests keep working. Use `subscribe` to
open streams; generic `request` rejects Subscribe commands.

The TCP transport closes a connection that says nothing for
`idle_timeout_secs`; the client pings for you as long as it is open.

## Running the example

Node 22.6+ runs this library directly with no build step:

```sh
node --experimental-strip-types examples/list-and-stream.ts
```

It lists the daemon's agents, then streams events until you stop it.
`AETHER_ADDRESS` (`host:port`) plus `AETHER_TOKEN` selects the TCP
transport; otherwise it connects over the Unix socket.

## Typechecking

`npm run typecheck` runs `tsc --noEmit` if you have `typescript` installed
(a devDependency, not required to use or run the library).

No runtime dependencies beyond Node's own `node:net`, `node:os`, and
`node:path`.

## Recurring timers

Use the generated builders through the same authenticated connection:

```ts
import { createSchedule, scheduleHistory, updateSchedule } from "./src/protocol.ts";

const created = await client.request(createSchedule({ spec: {
  name: "Read docs",
  target: agentId,
  recurrence: { kind: "interval", seconds: 60 },
  payload: "Read documentation and report changes",
  max_deliveries: 3,
} }));
if (created.reply === "schedule") {
  const id = created.data.id;
  const history = await client.request(scheduleHistory({ id, limit: 100 }));
  await client.request(updateSchedule({ id, action: "pause" }));
}
```

`listSchedules`, `getSchedule`, and `updateSchedule` also support inspection,
resuming, and cancellation. History records trigger delivery, skipping, or
failure; it does not claim that the model completed the task. Put a `spawn`
template (`role`, `objective`, optional model and scope) in place of `target`
to start a fresh root run on each occurrence instead (protocol 9).
