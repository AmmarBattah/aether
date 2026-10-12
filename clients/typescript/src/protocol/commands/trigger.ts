// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * Deliver an external trigger (a webhook, a timer, a CI result) to an
 * agent's inbox. It wakes a waiting agent like a person's message.
 */
export interface TriggerCommand {
  "cmd": "trigger";
  "agent": AgentId;
  "idempotency_key"?: string | null;
  "payload"?: unknown;
  "source": string;
}

export function trigger(args: Omit<TriggerCommand, "cmd">): TriggerCommand {
  return { cmd: "trigger", ...args };
}
