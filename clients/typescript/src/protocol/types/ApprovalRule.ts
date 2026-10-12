// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ApprovalDecision,
  ApprovalRuleId,
  Revocation,
  RuleMatch,
  RuleScope,
} from "../../protocol.ts";

/**
 * A decision a person asked Aether to remember.
 */
export interface ApprovalRule {
  "created_at": string;
  "created_by": Actor;
  "decision": ApprovalDecision;
  "id": ApprovalRuleId;
  "match": RuleMatch;
  "note"?: string | null;
  "revoked"?: Revocation | null;
  "scope": RuleScope;
}
