// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * What one pass of reading a repository's committed notes did.
 */
export interface SeedReport {
  "admitted": number;
  "found": number;
  "rejected": number;
  "retracted"?: number;
  "superseded": number;
  "unchanged": number;
}
