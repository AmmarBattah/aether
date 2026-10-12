// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ApprovalId,
  ApprovalStatus,
  ResourceKey,
  SideEffectClass,
  TaskId,
  ToolCallId,
  status,
} from "../../protocol.ts";

/**
 * A request for a human (or supervising agent) to allow a tool call.
 */
export interface ApprovalRequest {
  "agent_id": AgentId;
  "argument"?: string | null;
  "call_id": ToolCallId;
  "description": string;
  "id": ApprovalId;
  "requested_at": string;
  "resources"?: Array<ResourceKey>;
  "side_effect": SideEffectClass;
  "status": ApprovalStatus;
  "task_id"?: TaskId | null;
  "tool": string;
}
