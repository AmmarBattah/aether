// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Recreate a run from such a file, under fresh ids.
 */
export interface ImportRunCommand {
  "cmd": "import_run";
  "path": string;
}

export function importRun(args: Omit<ImportRunCommand, "cmd">): ImportRunCommand {
  return { cmd: "import_run", ...args };
}
