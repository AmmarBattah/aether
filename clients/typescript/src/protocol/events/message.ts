// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  AgentId,
  ContentBlock,
  MessageId,
} from "../../protocol.ts";


export interface EventKindMessageSent {
  "content": Array<ContentBlock>;
  "from": Actor;
  "message": MessageId;
  "to": AgentId;
  "type": "message.sent";
}
