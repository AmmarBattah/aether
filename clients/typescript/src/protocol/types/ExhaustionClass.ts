// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ModelErrorClass,
} from "../../protocol.ts";

/**
 * The subset of [`ModelErrorClass`] a probe can recover from. `ContextLimit`
 * is deliberately unrepresentable: it is not credits, and waiting cannot fix it.
 */
export type ExhaustionClass = "quota_exhausted" | "usage_limit";
