// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  SideEffectClass,
} from "../../protocol.ts";

/**
 * One tool a server offers, as Aether will expose it.
 */
export interface McpToolInfo {
  "declared": boolean;
  "name": string;
  "qualified_name": string;
  "side_effect": SideEffectClass;
}
