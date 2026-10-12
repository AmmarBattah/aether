// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RunId,
} from "../../protocol.ts";

/**
 * Write a finished run's events, snapshots, checkpoints and artifacts
 * to one file on the daemon's machine.
 */
export interface ExportRunCommand {
  "cmd": "export_run";
  "path": string;
  "run": RunId;
}

export function exportRun(args: Omit<ExportRunCommand, "cmd">): ExportRunCommand {
  return { cmd: "export_run", ...args };
}
