// A malformed frame from the daemon must fail the connection cleanly, not
// throw out of the socket's `data` handler and take the host process down
// with it. Run: `node --experimental-strip-types --test tests/*.test.ts`
// (node 22.6+), from clients/typescript/.

import test from "node:test";
import assert from "node:assert/strict";
import { createServer, type Server, type Socket } from "node:net";

import { AetherClient, AetherError, MAX_QUEUED_SUBSCRIPTION_FRAMES, SubscriptionLaggedError } from "../src/client.ts";
import { ping, subscribe } from "../src/protocol.ts";

/** A stub daemon: answers the hello with a welcome, then lets the test write
 * whatever raw line it wants onto the same connection. */
async function stubDaemon(helloResult?: unknown, onRequest?: (request: { req: number; cmd: string; after?: number }, send: (line: string) => void) => void): Promise<{
  port: number;
  send: (line: string) => void;
  disconnected: Promise<void>;
  end: () => void;
  close: () => Promise<void>;
}> {
  let send: (line: string) => void = () => {
    throw new Error("no client has connected to the stub daemon yet");
  };
  let connectedSocket: Socket | undefined;
  let disconnectedResolve: () => void = () => {};
  const disconnected = new Promise<void>((resolve) => { disconnectedResolve = resolve; });
  const server: Server = createServer((socket: Socket) => {
    connectedSocket = socket;
    socket.once("close", disconnectedResolve);
    send = (line: string) => socket.write(line + "\n");
    let buffered = "";
    socket.on("data", (chunk: Buffer) => {
      buffered += chunk.toString("utf8");
      let newline: number;
      while ((newline = buffered.indexOf("\n")) >= 0) {
        const line = buffered.slice(0, newline);
        buffered = buffered.slice(newline + 1);
        const req = JSON.parse(line) as { req: number; cmd: string };
        if (req.cmd === "hello") {
          send(
            JSON.stringify({
              frame: "reply",
              id: req.req,
              result: helloResult ?? {
                Ok: {
                  reply: "welcome",
                  data: {
                    protocol_version: 1,
                    daemon_version: "stub",
                    instance_id: "stub",
                    schema_version: 1,
                    pid: 1,
                    idle_timeout_secs: null,
                  },
                },
              },
            }),
          );
        }
        else onRequest?.(req, send);
        // Any other line (e.g. the client's own ping) is left unanswered:
        // these tests only care about surviving a malformed frame.
      }
    });
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()));
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("expected a bound TCP address");
  }
  return {
    port: address.port,
    send: (line: string) => send(line),
    disconnected,
    end: () => connectedSocket?.end(),
    close: () => new Promise<void>((resolve) => {
      connectedSocket?.destroy();
      server.close(() => resolve());
    }),
  };
}

test("a refused handshake closes its connection", async () => {
  const daemon = await stubDaemon({ Err: { code: "unauthorized", message: "bad token" } });
  try {
    await assert.rejects(() => AetherClient.connectTcp("127.0.0.1", daemon.port, "bad-token"), /bad token/);
    await Promise.race([
      daemon.disconnected,
      new Promise<void>((_, reject) => setTimeout(() => reject(new Error("handshake left the socket open")), 2000)),
    ]);
  } finally {
    await daemon.close();
  }
});

test("an unserializable command leaves no pending request", async () => {
  const daemon = await stubDaemon();
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test-token");
  try {
    const circular: { cmd: "ping"; self?: unknown } = { cmd: "ping" };
    circular.self = circular;
    await assert.rejects(() => client.request(circular), /circular/i);
    assert.equal((client as unknown as { pending: Map<number, unknown> }).pending.size, 0);
  } finally {
    client.close();
    await daemon.close();
  }
});

test("a frame that is not an object fails the connection instead of throwing", async () => {
  const daemon = await stubDaemon();
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "irrelevant-token");
  daemon.send("[1,2,3]");
  await assert.rejects(() => client.request(ping(), 2000));
  assert.equal(client.isConnected, false);
  client.close();
  await daemon.close();
});

test("a reply with no result fails the connection instead of throwing", async () => {
  const daemon = await stubDaemon();
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "irrelevant-token");
  daemon.send(JSON.stringify({ frame: "reply", id: 1 }));
  await assert.rejects(() => client.request(ping(), 2000));
  assert.equal(client.isConnected, false);
  client.close();
  await daemon.close();
});

test("request rejects on its own timeout rather than hanging forever", async () => {
  const daemon = await stubDaemon();
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "irrelevant-token");
  // The stub never answers a ping; a short timeout proves the caller is not
  // left waiting on a daemon that has stopped replying.
  await assert.rejects(() => client.request(ping(), 50), /within 50ms/);
  client.close();
  await daemon.close();
});

test("a connection diagnostic and EOF reject every pending request", { timeout: 5000 }, async () => {
  let requests = 0;
  let ready: () => void = () => {};
  const pending = new Promise<void>((resolve) => { ready = resolve; });
  const daemon = await stubDaemon(undefined, () => { if (++requests === 2) ready(); });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    const results = Promise.allSettled([client.request(ping(), 0), client.request(ping(), 0)]);
    await pending;
    daemon.send(JSON.stringify({ frame: "reply", id: 0, result: { Err: {
      code: "invalid_argument", message: "bad frame: invalid `req`",
    } } }));
    daemon.end();
    const settled = await Promise.race([
      results,
      new Promise<never>((_, reject) => setTimeout(() => reject(new Error("closure left requests pending")), 2000).unref()),
    ]);
    for (const result of settled) {
      assert.equal(result.status, "rejected");
      if (result.status === "rejected") {
        assert.ok(result.reason instanceof AetherError);
        assert.equal(result.reason.code, "invalid_argument");
        assert.match(result.reason.message, /invalid `req`/);
      }
    }
    assert.equal(client.isConnected, false);
  } finally {
    client.close();
    await daemon.close();
  }
});


test("slow subscriptions bound the prefix, report overflow, and resume while replies work", async () => {
  let subscriptions = 0;
  const unsubscribed: number[] = [];
  const daemon = await stubDaemon(undefined, (request, send) => {
    if (request.cmd === "subscribe") {
      subscriptions++;
      const id = subscriptions;
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "subscribed", data: { id, last_seq: request.after ?? 0 } } } }));
      if (id === 2) send(JSON.stringify({ frame: "event", sub: id, event: { seq: (request.after ?? 0) + 1 } }));
    } else {
      if (request.cmd === "unsubscribe") unsubscribed.push(1);
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "ok" } } }));
    }
  });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    const sub = await client.subscribe({ after: 0, query: {}, deltas: false });
    for (let seq = 1; seq <= MAX_QUEUED_SUBSCRIPTION_FRAMES + 2; seq++) {
      daemon.send(JSON.stringify({ frame: "event", sub: sub.id, event: { seq } }));
    }
    assert.equal((await client.request(ping(), 2000)).reply, "ok");
    let lastSeq = 0;
    for (let i = 0; i < MAX_QUEUED_SUBSCRIPTION_FRAMES; i++) {
      const frame = await sub.next();
      assert.equal(frame.done, false);
      assert.equal(frame.value.kind, "event");
      if (frame.value.kind === "event") {
        assert.equal(frame.value.event.seq, lastSeq + 1);
        lastSeq = frame.value.event.seq;
      }
    }
    await assert.rejects(() => sub.next(), SubscriptionLaggedError);
    const resumed = await client.subscribe({ after: lastSeq, query: {}, deltas: false });
    const frame = await resumed.next();
    assert.equal(frame.value.kind, "event");
    if (frame.value.kind === "event") assert.equal(frame.value.event.seq, lastSeq + 1);
    assert.equal(unsubscribed.length, 1);
    await resumed.unsubscribe();
  } finally {
    client.close();
    await daemon.close();
  }
});

test("generic Subscribe requests cannot create an unread subscription", async () => {
  let requests = 0;
  const daemon = await stubDaemon(undefined, () => { requests++; });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    await assert.rejects(() => client.request(subscribe({ query: {}, deltas: false })), /AetherClient.subscribe/);
    assert.equal(requests, 0);
  } finally {
    client.close();
    await daemon.close();
  }
});


test("delta overflow in a reply batch remains observable", async () => {
  const daemon = await stubDaemon(undefined, (request, send) => {
    if (request.cmd === "subscribe") {
      const frames: unknown[] = [{ frame: "reply", id: request.req, result: { Ok: { reply: "subscribed", data: { id: 7, last_seq: 0 } } } }];
      for (let i = 0; i <= MAX_QUEUED_SUBSCRIPTION_FRAMES; i++) {
        frames.push({ frame: "delta", sub: 7, agent: "agent", invocation: "call", delta: { type: "text", text: "d" } });
      }
      send(frames.map(frame => JSON.stringify(frame)).join("\n"));
    } else {
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "ok" } } }));
    }
  });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    const sub = await client.subscribe({ query: {}, deltas: true });
    await client.request(ping(), 2000);
    for (let i = 0; i < MAX_QUEUED_SUBSCRIPTION_FRAMES; i++) {
      assert.equal((await sub.next()).value.kind, "delta");
    }
    await assert.rejects(() => sub.next(), SubscriptionLaggedError);
  } finally {
    client.close();
    await daemon.close();
  }
});

test("failed overflow cleanup keeps other subscriptions and requests working", async (t) => {
  const warnings: string[] = [];
  t.mock.method(console, "warn", (message: string) => warnings.push(message));
  let nextId = 7;
  let released: () => void = () => {};
  const releaseAttempted = new Promise<void>(resolve => { released = resolve; });
  const daemon = await stubDaemon(undefined, (request, send) => {
    if (request.cmd === "subscribe") {
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "subscribed", data: { id: nextId++, last_seq: 0 } } } }));
    } else if (request.cmd === "unsubscribe") {
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Err: { code: "internal", message: "cleanup refused" } } }));
      released();
    } else {
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "ok" } } }));
    }
  });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    const slow = await client.subscribe({ query: {}, deltas: false });
    const other = await client.subscribe({ query: {}, deltas: false });
    const frames = Array.from({ length: MAX_QUEUED_SUBSCRIPTION_FRAMES + 1 }, (_, seq) =>
      JSON.stringify({ frame: "event", sub: slow.id, event: { seq } }));
    daemon.send(frames.join("\n"));
    await releaseAttempted;
    assert.equal((await client.request(ping(), 2000)).reply, "ok");
    daemon.send(JSON.stringify({ frame: "event", sub: other.id, event: { seq: 123 } }));
    const frame = await other.next();
    assert.equal(frame.value.kind, "event");
    if (frame.value.kind === "event") assert.equal(frame.value.event.seq, 123);
    assert.equal(warnings.length, 1);
    const warning = warnings[0];
    assert.ok(warning);
    assert.match(warning, /cleanup refused/);
    for (let i = 0; i < MAX_QUEUED_SUBSCRIPTION_FRAMES; i++) await slow.next();
    await assert.rejects(() => slow.next(), SubscriptionLaggedError);
  } finally {
    client.close();
    await daemon.close();
  }
});


test("breaking iteration unsubscribes once and ends the local queue", async () => {
  let releases = 0;
  const daemon = await stubDaemon(undefined, (request, send) => {
    if (request.cmd === "subscribe") {
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "subscribed", data: { id: 7, last_seq: 0 } } } }));
      send(JSON.stringify({ frame: "event", sub: 7, event: { seq: 1 } }));
    } else if (request.cmd === "unsubscribe") {
      releases++;
      send(JSON.stringify({ frame: "reply", id: request.req, result: { Ok: { reply: "ok" } } }));
    }
  });
  const client = await AetherClient.connectTcp("127.0.0.1", daemon.port, "test");
  try {
    const sub = await client.subscribe({ query: {}, deltas: false });
    for await (const frame of sub) { assert.equal(frame.kind, "event"); break; }
    assert.equal(releases, 1);
    assert.equal((await sub.next()).done, true);
    await sub.unsubscribe();
    assert.equal(releases, 1);
  } finally { client.close(); await daemon.close(); }
});
