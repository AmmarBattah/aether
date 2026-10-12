// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Why a provider is being avoided, and until when.
 */
export interface ProviderBackoff {
  "reason": string;
  "until": string;
}
