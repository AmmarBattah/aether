// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ContentBlock,
  MessageRole,
} from "../../protocol.ts";

/**
 * One turn of conversation as the model sees it.
 */
export interface Message {
  "content": Array<ContentBlock>;
  "role": MessageRole;
}
