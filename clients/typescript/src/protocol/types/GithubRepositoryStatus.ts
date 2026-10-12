// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


export interface GithubRepositoryStatus {
  "dispatched": number;
  "ignored": number;
  "last_error"?: string | null;
  "last_poll_at"?: string | null;
  "last_success_at"?: string | null;
  "next_poll_at"?: string | null;
  "repository": string;
}
