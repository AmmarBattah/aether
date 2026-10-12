// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Usage,
} from "../../protocol.ts";

/**
 * Lifetime token counters and activity stats.
 */
export interface AgentStats {
  "context_tokens"?: number | null;
  "last_turn_at"?: string | null;
  "tool_calls"?: number;
  "turns"?: number;
  "usage"?: Usage;
}
