// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.



export interface ShutdownCommand {
  "cmd": "shutdown";
}

export function shutdown(): ShutdownCommand {
  return { cmd: "shutdown" };
}
