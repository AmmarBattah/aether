// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Engine host setup a session loads. Lean drops Claude Code's native skills
 * and non-Aether MCP servers; other engines ignore it.
 */
export type HostContext = "full" | "lean";
