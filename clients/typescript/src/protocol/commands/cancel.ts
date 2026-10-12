// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";


export interface CancelCommand {
  "cmd": "cancel";
  "agent": AgentId;
  "reason"?: string | null;
}

export function cancel(args: Omit<CancelCommand, "cmd">): CancelCommand {
  return { cmd: "cancel", ...args };
}
