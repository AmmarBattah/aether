// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ApprovalId,
  TaskId,
} from "../../protocol.ts";

/**
 * Something the run left unfinished.
 * Tagged by `open`.
 */
export interface OpenLoopAgent {
  "id": AgentId;
  "name": string;
  "open": "agent";
}

export interface OpenLoopTask {
  "id": TaskId;
  "objective": string;
  "open": "task";
}

export interface OpenLoopApproval {
  "description": string;
  "id": ApprovalId;
  "open": "approval";
  "tool": string;
}

export type OpenLoop = OpenLoopAgent | OpenLoopTask | OpenLoopApproval;
