// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ApprovalDecision,
  ApprovalId,
} from "../../protocol.ts";

export interface ReplayApproval {
  "decided_by"?: Actor | null;
  "decision"?: ApprovalDecision | null;
  "description": string;
  "id": ApprovalId;
  "requested_at": string;
  "tool": string;
}
