// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  MemoryId,
} from "../../protocol.ts";

/**
 * Curate a memory out. The record stays; it is never retrieved again.
 */
export interface ForgetMemoryCommand {
  "cmd": "forget_memory";
  "id": MemoryId;
  "reason"?: string | null;
}

export function forgetMemory(args: Omit<ForgetMemoryCommand, "cmd">): ForgetMemoryCommand {
  return { cmd: "forget_memory", ...args };
}
