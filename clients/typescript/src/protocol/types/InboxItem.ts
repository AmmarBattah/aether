// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  InboxItemId,
  InboxKind,
} from "../../protocol.ts";

/**
 * One delivered item. `seq` is per-agent and strictly increasing.
 */
export interface InboxItem {
  "agent_id": AgentId;
  "delivered_at": string;
  "id": InboxItemId;
  "kind": InboxKind;
  "seq": number;
}
