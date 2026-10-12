// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
  RunRetentionDecision,
} from "../../protocol.ts";

export interface RunRetentionPreview {
  "decision": RunRetentionDecision;
  "events": number;
  "history_payload_bytes": number;
  "rows": number;
  "run": RunId;
}
