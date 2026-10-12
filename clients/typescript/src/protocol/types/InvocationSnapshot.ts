// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  CapabilityProfile,
  ContextLayer,
  InvocationParams,
  MemoryId,
  Message,
  ModelRef,
  SnapshotId,
  TaskId,
  ToolSpec,
  replay,
} from "../../protocol.ts";

/**
 * Everything sent to the model on one turn, frozen before the request.
 *
 * Remote inference is not deterministic, so this does not promise replay.
 * It promises reconstruction: every input the model received, and the
 * adapter and capability profile that shaped the wire request, are here.
 */
export interface InvocationSnapshot {
  "adapter"?: string | null;
  "agent_id": AgentId;
  "capabilities"?: CapabilityProfile | null;
  "created_at": string;
  "id": SnapshotId;
  "layers": Array<ContextLayer>;
  "memory_ids"?: Array<MemoryId>;
  "messages": Array<Message>;
  "model": ModelRef;
  "params": InvocationParams;
  "task_id"?: TaskId | null;
  "token_estimate": number;
  "tools": Array<ToolSpec>;
  "turn": number;
}
