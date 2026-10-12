// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Every provider the registry knows, with its live health.
 */
export interface ProvidersCommand {
  "cmd": "providers";
}

export function providers(): ProvidersCommand {
  return { cmd: "providers" };
}
