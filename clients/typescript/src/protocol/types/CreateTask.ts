// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Priority,
  TaskId,
} from "../../protocol.ts";

export interface CreateTask {
  "dependencies"?: Array<TaskId>;
  "idempotency_key"?: string | null;
  "objective": string;
  "owner"?: AgentId | null;
  "parent_task"?: TaskId | null;
  "priority"?: Priority;
}
