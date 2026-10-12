// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ErrorCode,
} from "../../protocol.ts";

export interface ErrorReply {
  "code": ErrorCode;
  "message": string;
}
