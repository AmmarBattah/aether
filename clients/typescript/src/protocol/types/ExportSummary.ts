// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
} from "../../protocol.ts";

/**
 * What an export wrote.
 */
export interface ExportSummary {
  "agents": number;
  "artifact_payloads": number;
  "artifacts": number;
  "bytes_written": number;
  "checkpoints": number;
  "events": number;
  "invocations": number;
  "path": string;
  "run": RunId;
  "snapshots": number;
  "tasks": number;
}
