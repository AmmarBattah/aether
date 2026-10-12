// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  RunId,
  TaskStatusKind,
} from "../../protocol.ts";

export interface TaskQuery {
  "owner"?: AgentId | null;
  "run"?: RunId | null;
  "statuses"?: Array<TaskStatusKind>;
}
