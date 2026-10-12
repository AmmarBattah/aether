// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ContentBlock,
  InvocationId,
  ModelErrorClass,
  ModelRef,
  SnapshotId,
  StopReason,
  Usage,
} from "../../protocol.ts";


export interface EventKindInvocationCompleted {
  "duration_ms": number;
  "invocation": InvocationId;
  "model": ModelRef;
  "snapshot": SnapshotId;
  "stop_reason"?: StopReason | null;
  "type": "invocation.completed";
  "usage": Usage;
}

/**
 * The turn did not complete. Streamed content accumulated before the
 * failure or cancellation is kept so nothing the user watched is lost.
 */
export interface EventKindInvocationFailed {
  "cancelled"?: boolean;
  "class"?: ModelErrorClass | null;
  "error": string;
  "invocation": InvocationId;
  "partial"?: Array<ContentBlock>;
  "retryable": boolean;
  "snapshot": SnapshotId;
  "type": "invocation.failed";
}
