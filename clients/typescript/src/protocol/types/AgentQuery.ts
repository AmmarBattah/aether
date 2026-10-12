// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  AgentStatusKind,
  RunId,
} from "../../protocol.ts";

export interface AgentQuery {
  "include_archived"?: boolean;
  "live_only"?: boolean;
  "parent"?: AgentId | null;
  "run"?: RunId | null;
  "statuses"?: Array<AgentStatusKind>;
}
