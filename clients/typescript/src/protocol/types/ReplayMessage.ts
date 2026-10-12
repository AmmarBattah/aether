// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ContentBlock,
  InvocationId,
  MessageRole,
} from "../../protocol.ts";

export interface ReplayMessage {
  "at": string;
  "content": Array<ContentBlock>;
  "invocation"?: InvocationId | null;
  "role": MessageRole;
}
