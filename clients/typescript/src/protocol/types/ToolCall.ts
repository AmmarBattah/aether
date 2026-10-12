// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  IdempotencyKey,
  InvocationId,
  ResourceKey,
  TaskId,
  ToolCallId,
} from "../../protocol.ts";

/**
 * One tool invocation requested by a model.
 */
export interface ToolCall {
  "agent_id": AgentId;
  "id": ToolCallId;
  "idempotency_key"?: IdempotencyKey | null;
  "input": unknown;
  "invocation_id"?: InvocationId | null;
  "name": string;
  "provider_call_id": string;
  "requested_at": string;
  "resources"?: Array<ResourceKey>;
  "task_id"?: TaskId | null;
}
