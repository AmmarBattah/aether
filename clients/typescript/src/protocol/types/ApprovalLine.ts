// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ApprovalDecision,
  ApprovalId,
} from "../../protocol.ts";

export interface ApprovalLine {
  "decided_at"?: string | null;
  "decided_by"?: Actor | null;
  "decision"?: ApprovalDecision | null;
  "description": string;
  "id": ApprovalId;
  "requested_at": string;
  "tool": string;
}
