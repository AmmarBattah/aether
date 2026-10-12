// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  EventQuery,
} from "../../protocol.ts";

/**
 * Stream events with `seq > after` (0 replays everything, `None` starts live).
 */
export interface SubscribeCommand {
  "cmd": "subscribe";
  "after"?: number | null;
  "deltas": boolean;
  "query": EventQuery;
}

export function subscribe(args: Omit<SubscribeCommand, "cmd">): SubscribeCommand {
  return { cmd: "subscribe", ...args };
}
