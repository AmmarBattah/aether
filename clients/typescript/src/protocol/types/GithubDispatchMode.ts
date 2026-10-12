// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  trigger,
} from "../../protocol.ts";

/**
 * How a repository's accepted requests reach an agent.
 * Tagged by `mode`.
 */
/**
 * Deliver every trigger to the fixed, operator-chosen `agent`. Stale if
 * that agent pauses or goes terminal; an operator must repoint it.
 */
export interface GithubDispatchModeFixed {
  "mode": "fixed";
}

/**
 * Create an Aether-managed checkout and agent for each issue/PR thread.
 * The daemon's current model, approval, and network defaults apply.
 */
export interface GithubDispatchModeAutomatic {
  "mode": "automatic";
}

/**
 * Deliver to (or spawn, the first time or after the previous one went
 * terminal) a durable agent keyed by (repository, issue/PR number), so
 * the watcher heals itself instead of pointing at a dead agent forever.
 */
export interface GithubDispatchModeThread {
  "mode": "thread";
}

export type GithubDispatchMode =
    GithubDispatchModeFixed |
    GithubDispatchModeAutomatic |
    GithubDispatchModeThread
;
