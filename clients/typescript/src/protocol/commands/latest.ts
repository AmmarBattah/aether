// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";


export interface LatestCheckpointCommand {
  "cmd": "latest_checkpoint";
  "agent": AgentId;
}

export function latestCheckpoint(args: Omit<LatestCheckpointCommand, "cmd">): LatestCheckpointCommand {
  return { cmd: "latest_checkpoint", ...args };
}
