// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Usage,
} from "../../protocol.ts";

/**
 * A budget is 80 % spent: the person is told before the stop.
 */
export interface EventKindBudgetWarning {
  "reason": string;
  "type": "budget.warning";
  "usage": Usage;
}

/**
 * A budget is spent; the agent stops as Failed with this reason.
 */
export interface EventKindBudgetExceeded {
  "reason": string;
  "type": "budget.exceeded";
  "usage": Usage;
}
