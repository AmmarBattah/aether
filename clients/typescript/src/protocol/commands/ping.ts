// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Reply,
} from "../../protocol.ts";

/**
 * Keep an otherwise silent connection alive under an idle timeout.
 * Answered with [`Reply::Ok`].
 */
export interface PingCommand {
  "cmd": "ping";
}

export function ping(): PingCommand {
  return { cmd: "ping" };
}
