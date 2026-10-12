// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  SideEffectClass,
} from "../../protocol.ts";

/**
 * Which side-effect classes need explicit approval before execution.
 */
export interface ApprovalPolicy {
  "require_for"?: Array<SideEffectClass>;
}
