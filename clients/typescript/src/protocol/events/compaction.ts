// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  CheckpointId,
  CompactionTrigger,
  ModelRef,
  trigger,
} from "../../protocol.ts";


export interface EventKindCompactionRequested {
  "trigger": CompactionTrigger;
  "type": "compaction.requested";
}

export interface EventKindCompactionCompleted {
  "after_tokens"?: number | null;
  "before_tokens"?: number | null;
  "checkpoint": CheckpointId;
  "from_seq": number;
  "to_seq": number;
  "type": "compaction.completed";
  "version": number;
}

/**
 * `models` names what was asked to fold, in the order it was tried: a
 * transcript that reached a provider that then failed leaves no other
 * durable trace of the egress.
 */
export interface EventKindCompactionFailed {
  "error": string;
  "models"?: Array<ModelRef>;
  "type": "compaction.failed";
}
