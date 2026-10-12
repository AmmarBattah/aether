// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Ended,
  ModelRef,
  Role,
  Usage,
} from "../../protocol.ts";

export interface AgentLine {
  "created_at": string;
  "depth": number;
  "ended_at"?: string | null;
  "id": AgentId;
  "model": ModelRef;
  "name": string;
  "outcome": Ended;
  "parent"?: AgentId | null;
  "role": Role;
  "subtree_usage": Usage;
  "tool_calls": number;
  "turns": number;
  "usage": Usage;
  "workspace"?: string | null;
}
