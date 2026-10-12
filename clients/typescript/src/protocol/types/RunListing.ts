// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
  Usage,
} from "../../protocol.ts";

/**
 * One row of `aether runs list`, projected from a run's agents and tasks.
 */
export interface RunListing {
  "agents": number;
  "agents_imported"?: number;
  "agents_live": number;
  "objective"?: string | null;
  "run": RunId;
  "started_at": string;
  "updated_at": string;
  "usage": Usage;
}
