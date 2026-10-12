// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  TaskId,
  TaskResult,
} from "../../protocol.ts";

/**
 * Tagged by `state`.
 */
export interface TaskStatusQueued {
  "state": "queued";
}

export interface TaskStatusAssigned {
  "state": "assigned";
}

export interface TaskStatusActive {
  "state": "active";
}

export interface TaskStatusBlocked {
  "dependency"?: TaskId | null;
  "reason": string;
  "state": "blocked";
}

export interface TaskStatusCompleted {
  "result": TaskResult;
  "state": "completed";
}

export interface TaskStatusFailed {
  "error": string;
  "state": "failed";
  "structured"?: unknown;
}

export interface TaskStatusCancelled {
  "reason"?: string | null;
  "state": "cancelled";
}

export type TaskStatus =
    TaskStatusQueued |
    TaskStatusAssigned |
    TaskStatusActive |
    TaskStatusBlocked |
    TaskStatusCompleted |
    TaskStatusFailed |
    TaskStatusCancelled
;
