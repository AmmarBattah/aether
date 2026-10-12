// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * The agent's wall-clock deadline passed; it stops as Failed.
 */
export interface EventKindDeadlinePassed {
  "reason": string;
  "type": "deadline.passed";
}
