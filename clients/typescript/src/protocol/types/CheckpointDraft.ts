// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactNote,
  Checkpoint,
  DroppedItem,
} from "../../protocol.ts";

/**
 * The model-facing checkpoint schema. This is what a compactor model is
 * asked to produce; the runtime wraps it into a [`Checkpoint`].
 */
export interface CheckpointDraft {
  "artifacts"?: Array<ArtifactNote>;
  "constraints"?: Array<string>;
  "decisions"?: Array<string>;
  "dropped_constraints"?: Array<DroppedItem>;
  "objective": string;
  "open_loops"?: Array<string>;
  "resolved_open_loops"?: Array<string>;
  "state": string;
  "uncertainty"?: Array<string>;
}
