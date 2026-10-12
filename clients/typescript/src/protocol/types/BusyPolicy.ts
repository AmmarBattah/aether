// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  trigger,
} from "../../protocol.ts";

/**
 * What an occurrence does when the target is busy. Either way at most one
 * trigger from a schedule is ever pending in the target's inbox.
 */
export type BusyPolicy = "skip" | "queue_one";
