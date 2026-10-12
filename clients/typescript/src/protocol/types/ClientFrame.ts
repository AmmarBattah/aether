// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Client to daemon. The request id is `req` on the wire so it can never
 * collide with a command's own `id` field after flattening.
 */
export interface ClientFrame {
  "req": number;
}
