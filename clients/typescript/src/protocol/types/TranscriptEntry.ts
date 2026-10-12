// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  InvocationId,
  Message,
} from "../../protocol.ts";

export interface TranscriptEntry {
  "at": string;
  "invocation"?: InvocationId | null;
  "message": Message;
  "seq": number;
}
