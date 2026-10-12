// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  RunId,
  TaskId,
} from "../../protocol.ts";

export interface EventQuery {
  "agent"?: AgentId | null;
  "before"?: number | null;
  "families"?: Array<string>;
  "run"?: RunId | null;
  "task"?: TaskId | null;
  "types"?: Array<string>;
}
