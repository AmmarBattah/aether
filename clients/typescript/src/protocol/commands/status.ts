// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.



export interface StatusCommand {
  "cmd": "status";
}

export function status(): StatusCommand {
  return { cmd: "status" };
}
