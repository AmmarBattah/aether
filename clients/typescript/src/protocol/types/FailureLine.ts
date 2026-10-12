// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * What failed, and what it was. Every failure in the run, in order.
 */
export interface FailureLine {
  "agent"?: AgentId | null;
  "at": string;
  "error": string;
  "kind": string;
}
