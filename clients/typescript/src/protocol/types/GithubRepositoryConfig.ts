// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  GithubDispatchMode,
} from "../../protocol.ts";

export interface GithubRepositoryConfig {
  "agent"?: AgentId | null;
  "allowed_users"?: Array<string> | null;
  "dispatch"?: GithubDispatchMode;
  "enabled": boolean;
  "repository": string;
}
