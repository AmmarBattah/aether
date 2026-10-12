// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.



export interface UnsubscribeCommand {
  "cmd": "unsubscribe";
  "id": number;
}

export function unsubscribe(args: Omit<UnsubscribeCommand, "cmd">): UnsubscribeCommand {
  return { cmd: "unsubscribe", ...args };
}
