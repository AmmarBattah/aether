// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  GithubRepositoryConfig,
} from "../../protocol.ts";

export interface GithubWatcherConfig {
  "enabled"?: boolean;
  "label_regex"?: string;
  "mention_regex"?: string;
  "operator_users"?: Array<string> | null;
  "poll_interval_secs"?: number;
  "repositories"?: Array<GithubRepositoryConfig>;
}
