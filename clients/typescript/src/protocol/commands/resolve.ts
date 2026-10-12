// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ApprovalDecision,
  ApprovalId,
  RememberSpec,
} from "../../protocol.ts";


export interface ResolveApprovalCommand {
  "cmd": "resolve_approval";
  "decision": ApprovalDecision;
  "id": ApprovalId;
  "note"?: string | null;
  "remember"?: RememberSpec | null;
}

export function resolveApproval(args: Omit<ResolveApprovalCommand, "cmd">): ResolveApprovalCommand {
  return { cmd: "resolve_approval", ...args };
}
