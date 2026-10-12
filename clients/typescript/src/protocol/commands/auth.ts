// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.



export interface AuthStatusCommand {
  "cmd": "auth_status";
}

export function authStatus(): AuthStatusCommand {
  return { cmd: "auth_status" };
}
