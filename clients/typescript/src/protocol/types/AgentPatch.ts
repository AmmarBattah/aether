// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ApprovalPolicy,
  DeadlineUpdate,
  Effort,
  ExecutionBackend,
  ModelRef,
  NetworkPolicy,
  Priority,
  ToolExposure,
} from "../../protocol.ts";

/**
 * Partial update to a live agent. Only set fields change.
 */
export interface AgentPatch {
  "approval"?: ApprovalPolicy | null;
  "backend"?: ExecutionBackend | null;
  "concurrency"?: number | null;
  "cost_budget"?: number | null;
  "deadline"?: DeadlineUpdate | null;
  "effort"?: Effort | null;
  "fallbacks"?: Array<ModelRef> | null;
  "instructions"?: string | null;
  "long_lived"?: boolean | null;
  "max_turns"?: number | null;
  "model"?: ModelRef | null;
  "name"?: string | null;
  "network"?: NetworkPolicy | null;
  "priority"?: Priority | null;
  "token_budget"?: number | null;
  "tools"?: ToolExposure | null;
}
