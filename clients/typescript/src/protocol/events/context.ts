// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  CheckpointId,
  ModelRef,
} from "../../protocol.ts";

/**
 * An agent's context crossed the engine/API boundary. Names what carried
 * over and, honestly, what did not.
 */
export interface EventKindContextHandoff {
  "carried": number;
  "checkpoint"?: CheckpointId | null;
  "from": ModelRef;
  "lost"?: Array<string>;
  "to": ModelRef;
  "type": "context.handoff";
}
