// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  RunId,
} from "../../protocol.ts";

/**
 * Where a remembered decision applies.
 * Tagged by `scope`.
 */
/**
 * Every agent this daemon runs.
 */
export interface RuleScopeEverywhere {
  "scope": "everywhere";
}

/**
 * One agent only.
 */
export interface RuleScopeAgent {
  "id": AgentId;
  "scope": "agent";
}

/**
 * An agent and everything it delegates to.
 */
export interface RuleScopeSubtree {
  "root": AgentId;
  "scope": "subtree";
}

/**
 * Every agent in one run.
 */
export interface RuleScopeRun {
  "id": RunId;
  "scope": "run";
}

export type RuleScope = RuleScopeEverywhere | RuleScopeAgent | RuleScopeSubtree | RuleScopeRun;
