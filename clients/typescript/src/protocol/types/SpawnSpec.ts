// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentConfig,
  ApprovalPolicy,
  ModelRef,
  Scope,
} from "../../protocol.ts";

/**
 * Root agent + task each spawn occurrence creates, in a new run. Fields as on
 * `create_agent`; no parent or existing run.
 */
export interface SpawnSpec {
  "approval"?: ApprovalPolicy | null;
  "config"?: AgentConfig | null;
  "fallbacks"?: Array<ModelRef>;
  "model"?: ModelRef | null;
  "name"?: string | null;
  "objective": string;
  "role": string;
  "scope"?: Scope | null;
}
