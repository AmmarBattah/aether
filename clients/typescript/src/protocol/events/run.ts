// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
} from "../../protocol.ts";

/**
 * A run was replayed into this store from another daemon's export. It is
 * the imported run's first event, so everything after it reads as
 * history that happened elsewhere.
 */
export interface EventKindRunImported {
  "events": number;
  "exported_at": string;
  "source_instance": string;
  "source_run": RunId;
  "type": "run.imported";
}
