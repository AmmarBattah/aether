// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  SideEffectClass,
  ToolCallId,
  ToolResult,
} from "../../protocol.ts";

export interface ReplayTool {
  "call": ToolCallId;
  "input": unknown;
  "name": string;
  "outcome": ToolResult;
  "requested_at": string;
  "side_effect": SideEffectClass;
}
