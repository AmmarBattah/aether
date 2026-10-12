// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  GithubWatcherConfig,
} from "../../protocol.ts";

/**
 * Replace watcher settings only if no other operator changed them.
 */
export interface ConfigureGithubWatcherCommand {
  "cmd": "configure_github_watcher";
  "config": GithubWatcherConfig;
  "expected_revision": number;
}

export function configureGithubWatcher(args: Omit<ConfigureGithubWatcherCommand, "cmd">): ConfigureGithubWatcherCommand {
  return { cmd: "configure_github_watcher", ...args };
}
