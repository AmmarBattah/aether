// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  CheckpointId,
  CompactionTrigger,
  ModelRef,
  SnapshotId,
  trigger,
} from "../../protocol.ts";

/**
 * Where a checkpoint came from.
 */
export interface CheckpointProvenance {
  "compactor_model"?: ModelRef | null;
  "from_seq": number;
  "previous"?: CheckpointId | null;
  "source_snapshot"?: SnapshotId | null;
  "to_seq": number;
  "trigger"?: CompactionTrigger | null;
}
