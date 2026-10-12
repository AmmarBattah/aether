// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

export interface Lease {
  "agent_id": AgentId;
  "expires_at": string;
  "granted_at": string;
  "holder": string;
  "turn": number;
}
