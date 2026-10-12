// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Read-only retention decisions; never applies deletion or cleanup.
 */
export interface PreviewRetentionCommand {
  "cmd": "preview_retention";
}

export function previewRetention(): PreviewRetentionCommand {
  return { cmd: "preview_retention" };
}
