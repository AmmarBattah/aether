// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Checkpoint,
} from "../../protocol.ts";


export interface EventKindCheckpointCreated {
  "checkpoint": Checkpoint;
  "type": "checkpoint.created";
}
