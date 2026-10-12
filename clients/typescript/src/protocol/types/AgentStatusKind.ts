// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentStatus,
} from "../../protocol.ts";

/**
 * The variant of an [`AgentStatus`] without its payload.
 */
export type AgentStatusKind = "created" | "ready" | "running" | "waiting" | "blocked" | "completed" | "failed" | "cancelled" | "imported";
