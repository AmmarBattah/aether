// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Priority,
  TaskId,
  TaskResult,
} from "../../protocol.ts";


export interface EventKindTaskCreated {
  "dependencies"?: Array<TaskId>;
  "objective": string;
  "parent_task"?: TaskId | null;
  "priority": Priority;
  "type": "task.created";
}

export interface EventKindTaskAssigned {
  "owner": AgentId;
  "type": "task.assigned";
}

export interface EventKindTaskStarted {
  "type": "task.started";
}

export interface EventKindTaskBlocked {
  "dependency"?: TaskId | null;
  "reason": string;
  "type": "task.blocked";
}

export interface EventKindTaskUnblocked {
  "type": "task.unblocked";
}

export interface EventKindTaskRequeued {
  "reason": string;
  "type": "task.requeued";
}

export interface EventKindTaskCompleted {
  "result": TaskResult;
  "type": "task.completed";
}

export interface EventKindTaskFailed {
  "error": string;
  "structured"?: unknown;
  "type": "task.failed";
}

export interface EventKindTaskCancelled {
  "reason"?: string | null;
  "type": "task.cancelled";
}
