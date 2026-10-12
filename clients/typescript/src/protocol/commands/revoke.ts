// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ApprovalRuleId,
} from "../../protocol.ts";


export interface RevokeApprovalRuleCommand {
  "cmd": "revoke_approval_rule";
  "id": ApprovalRuleId;
  "reason"?: string | null;
}

export function revokeApprovalRule(args: Omit<RevokeApprovalRuleCommand, "cmd">): RevokeApprovalRuleCommand {
  return { cmd: "revoke_approval_rule", ...args };
}
