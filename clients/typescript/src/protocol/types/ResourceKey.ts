// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * A resource a call touches, used to decide which calls may run together.
 * Conventional forms: `file:<absolute path>`, `dir:<absolute path>`,
 * `repo:<root>`, `network`, `agent:<id>`.
 */
export type ResourceKey = string;
