// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  MemoryQuerySpec,
} from "../../protocol.ts";


export interface MemoriesCommand {
  "cmd": "memories";
  "query": MemoryQuerySpec;
}

export function memories(args: Omit<MemoriesCommand, "cmd">): MemoriesCommand {
  return { cmd: "memories", ...args };
}
