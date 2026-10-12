// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Ended,
  Priority,
  TaskId,
} from "../../protocol.ts";

export interface TaskLine {
  "created_at": string;
  "dependencies"?: Array<TaskId>;
  "ended_at"?: string | null;
  "id": TaskId;
  "objective": string;
  "outcome": Ended;
  "owner"?: AgentId | null;
  "parent_task"?: TaskId | null;
  "priority": Priority;
}
