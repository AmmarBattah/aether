// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * One agent's turns as they happened, folded from the log.
 */
export interface ReplayCommand {
  "cmd": "replay";
  "agent": AgentId;
}

export function replay(args: Omit<ReplayCommand, "cmd">): ReplayCommand {
  return { cmd: "replay", ...args };
}
