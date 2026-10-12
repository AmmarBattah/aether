// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  MemoryId,
} from "../../protocol.ts";

/**
 * Put a forgotten memory back. The inverse of `ForgetMemory`, so a
 * mis-keyed curation has a way back.
 */
export interface RestoreMemoryCommand {
  "cmd": "restore_memory";
  "id": MemoryId;
}

export function restoreMemory(args: Omit<RestoreMemoryCommand, "cmd">): RestoreMemoryCommand {
  return { cmd: "restore_memory", ...args };
}
