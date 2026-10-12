// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  MemoryId,
} from "../../protocol.ts";

/**
 * Keep a memory in front of the model, or stop.
 */
export interface PinMemoryCommand {
  "cmd": "pin_memory";
  "id": MemoryId;
  "pinned": boolean;
}

export function pinMemory(args: Omit<PinMemoryCommand, "cmd">): PinMemoryCommand {
  return { cmd: "pin_memory", ...args };
}
