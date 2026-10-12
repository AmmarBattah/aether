// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  MessageId,
} from "../../protocol.ts";

/**
 * One agent's copy of a fanned-out message.
 */
export interface MessageDelivery {
  "agent": AgentId;
  "message": MessageId;
}
