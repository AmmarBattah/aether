// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ContextLayer,
  ModelRef,
  SnapshotId,
} from "../../protocol.ts";


export interface EventKindSnapshotCreated {
  "layers": Array<ContextLayer>;
  "model": ModelRef;
  "snapshot": SnapshotId;
  "token_estimate": number;
  "turn": number;
  "type": "snapshot.created";
}
