// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
} from "../../protocol.ts";

/**
 * What an import recreated.
 */
export interface ImportSummary {
  "agents": number;
  "artifact_payloads": number;
  "artifacts": number;
  "checkpoints": number;
  "events": number;
  "exported_at": string;
  "invocations": number;
  "run": RunId;
  "snapshots": number;
  "source_instance": string;
  "source_run": RunId;
  "tasks": number;
}
