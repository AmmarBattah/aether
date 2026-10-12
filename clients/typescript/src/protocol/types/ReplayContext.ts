// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ContextLayer,
  ModelRef,
  SnapshotId,
} from "../../protocol.ts";

/**
 * The shape of what the model was given. The layer texts are here; the
 * whole snapshot is a separate record, named so a caller can fetch it.
 */
export interface ReplayContext {
  "layers": Array<ContextLayer>;
  "model": ModelRef;
  "snapshot": SnapshotId;
  "token_estimate": number;
}
