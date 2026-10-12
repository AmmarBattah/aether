// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * Ask the runtime to compact the agent's context before its next turn.
 */
export interface CompactCommand {
  "cmd": "compact";
  "agent": AgentId;
}

export function compact(args: Omit<CompactCommand, "cmd">): CompactCommand {
  return { cmd: "compact", ...args };
}
