// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  GithubRepositoryStatus,
  GithubWatcherConfig,
} from "../../protocol.ts";

export interface GithubWatcherStatus {
  "config": GithubWatcherConfig;
  "repositories": Array<GithubRepositoryStatus>;
  "revision": number;
}
