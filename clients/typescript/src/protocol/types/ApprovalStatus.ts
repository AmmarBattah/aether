// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ApprovalDecision,
} from "../../protocol.ts";

/**
 * Tagged by `state`.
 */
export interface ApprovalStatusPending {
  "state": "pending";
}

export interface ApprovalStatusResolved {
  "at": string;
  "by": Actor;
  "decision": ApprovalDecision;
  "note"?: string | null;
  "state": "resolved";
}

export type ApprovalStatus = ApprovalStatusPending | ApprovalStatusResolved;
