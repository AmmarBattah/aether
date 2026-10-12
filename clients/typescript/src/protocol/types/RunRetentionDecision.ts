// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  status,
} from "../../protocol.ts";

/**
 * Tagged by `status`.
 */
export interface RunRetentionDecisionEligible {
  "status": "eligible";
}

/**
 * The first blocker, in the same order as the deletion path.
 */
export interface RunRetentionDecisionProtected {
  "reason": string;
  "status": "protected";
}

export type RunRetentionDecision = RunRetentionDecisionEligible | RunRetentionDecisionProtected;
