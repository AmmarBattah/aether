// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * One line of the run's story: the events a person would want to see.
 */
export interface Milestone {
  "agent"?: AgentId | null;
  "at": string;
  "detail": string;
  "kind": string;
}
