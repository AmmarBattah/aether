// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
} from "../../protocol.ts";

/**
 * Where an imported run came from. Absent on a run this daemon ran itself.
 */
export interface Provenance {
  "exported_at": string;
  "source_instance": string;
  "source_run": RunId;
}
