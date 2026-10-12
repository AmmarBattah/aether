// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  CheckpointDraft,
  CheckpointId,
  CheckpointProvenance,
} from "../../protocol.ts";

/**
 * A durable, validated checkpoint.
 */
export interface Checkpoint {
  "agent_id": AgentId;
  "created_at": string;
  "draft": CheckpointDraft;
  "id": CheckpointId;
  "provenance": CheckpointProvenance;
  "version": number;
}
