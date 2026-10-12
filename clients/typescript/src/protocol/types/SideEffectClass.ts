// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * What a tool does to the world. Drives retry policy, idempotency tracking
 * and approval requirements.
 */
export type SideEffectClass = "read_only" | "idempotent_mutation" | "mutation" | "external";
