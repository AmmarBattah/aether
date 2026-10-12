// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * One tool's tally across the run.
 */
export interface ToolLine {
  "cancelled": number;
  "denied": number;
  "duration_ms": number;
  "failed": number;
  "replayed": number;
  "requested": number;
  "retries": number;
  "succeeded": number;
  "tool": string;
}
