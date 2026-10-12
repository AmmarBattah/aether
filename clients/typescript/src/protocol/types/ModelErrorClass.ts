// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Normalized failure class. Every adapter maps its provider's errors here.
 */
export type ModelErrorClass = "rate_limit" | "overloaded" | "context_limit" | "invalid_request" | "not_found" | "network" | "timeout" | "cancelled" | "server" | "unknown" | "auth" | "permission" | "quota_exhausted" | "usage_limit";
