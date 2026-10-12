// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ContentBlock,
  InvocationId,
  ModelRef,
  SnapshotId,
  StopReason,
  Usage,
} from "../../protocol.ts";

/**
 * The result of sending a snapshot to a model.
 */
export interface Invocation {
  "agent_id": AgentId;
  "content"?: Array<ContentBlock>;
  "error"?: string | null;
  "finished_at"?: string | null;
  "id": InvocationId;
  "model": ModelRef;
  "provider_message_id"?: string | null;
  "snapshot_id": SnapshotId;
  "started_at": string;
  "stop_reason"?: StopReason | null;
  "turn": number;
  "usage"?: Usage;
}
