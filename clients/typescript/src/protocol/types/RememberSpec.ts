// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RuleScope,
} from "../../protocol.ts";

/**
 * How to remember the answer a person just gave. The defaults are the
 * narrowest reading of it: the answered agent, this tool, and what this
 * call touched. Anything wider is asked for explicitly.
 */
export interface RememberSpec {
  "note"?: string | null;
  "scope"?: RuleScope | null;
  "tool_wide"?: boolean;
}
