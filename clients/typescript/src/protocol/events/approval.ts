// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ApprovalDecision,
  ApprovalId,
  ApprovalRule,
  ApprovalRuleId,
  Revocation,
  SideEffectClass,
  ToolCallId,
} from "../../protocol.ts";


export interface EventKindApprovalRequested {
  "approval": ApprovalId;
  "argument"?: string | null;
  "call": ToolCallId;
  "description": string;
  "side_effect": SideEffectClass;
  "tool": string;
  "type": "approval.requested";
}

/**
 * A decision was remembered as a durable rule.
 */
export interface EventKindApprovalRuleAdded {
  "rule": ApprovalRule;
  "type": "approval.rule_added";
}

/**
 * The rule stops deciding anything from here on.
 */
export interface EventKindApprovalRuleRevoked {
  "revocation": Revocation;
  "rule": ApprovalRuleId;
  "type": "approval.rule_revoked";
}

export interface EventKindApprovalResolved {
  "approval": ApprovalId;
  "by": Actor;
  "decision": ApprovalDecision;
  "note"?: string | null;
  "type": "approval.resolved";
}
