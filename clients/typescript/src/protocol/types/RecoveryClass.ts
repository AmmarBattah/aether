// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  restart,
  resume,
} from "../../protocol.ts";

/**
 * How an interrupted turn should be handled after a restart.
 */
export type RecoveryClass = "resume" | "retry" | "inspect";
