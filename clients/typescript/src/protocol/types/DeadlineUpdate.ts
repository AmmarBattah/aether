// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * A change to an agent's wall-clock stop (§3.1). Every knob has to be
 * changeable on a live agent, and one that can only ever move further into
 * the future is not: removing the deadline needs a form of its own.
 */
export type DeadlineUpdate = "clear" | Record<string, unknown>;
