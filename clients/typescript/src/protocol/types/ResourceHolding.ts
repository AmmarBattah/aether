// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ResourceKey,
} from "../../protocol.ts";

/**
 * Resource keys one agent's mutating tool call holds for the length of that
 * call. Two agents never hold conflicting keys at once (VISION §3.6 limit 5).
 */
export interface ResourceHolding {
  "agent": AgentId;
  "keys": Array<ResourceKey>;
  "quarantined"?: boolean;
  "since": string;
}
