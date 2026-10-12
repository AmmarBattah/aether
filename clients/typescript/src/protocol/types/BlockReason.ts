// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Agent,
  AgentId,
  ExhaustionClass,
  ModelRef,
  ResourceKey,
  TaskId,
  pause,
} from "../../protocol.ts";

/**
 * Why an agent is Blocked. One variant per cause, each carrying what that
 * cause needs, so a Blocked agent is never ambiguous about what would clear
 * it. An operator pause is not here: it is orthogonal to scheduling state
 * and lives in [`Agent::paused`] (VISION §3.1).
 * Tagged by `block`.
 */
/**
 * VISION §3.2: the owner of a task whose dependencies are not terminal.
 * Cleared only when that dependency finishes.
 */
export interface BlockReasonDependency {
  "block": "dependency";
  "on": TaskId;
}

/**
 * VISION §3.6 limit 5: a live turn waiting for a resource key another
 * agent holds. The turn itself resumes when the key comes free.
 */
export interface BlockReasonResource {
  "block": "resource";
  "holder": AgentId;
  "key": ResourceKey;
}

/**
 * VISION §3.9: a non-idempotent tool call started without an outcome.
 * Only a person clears it.
 */
export interface BlockReasonInspection {
  "block": "inspection";
  "detail": string;
}

/**
 * Provider credits/allowance spent on every link of the chain. Cleared by
 * the scheduler when `next_probe` comes due, or by a person.
 */
export interface BlockReasonExhausted {
  "block": "exhausted";
  "class": ExhaustionClass;
  "detail": string;
  "model": ModelRef;
  "next_probe": string;
  "probes": number;
  "reported_reset"?: string | null;
  "since": string;
}

export type BlockReason =
    BlockReasonDependency |
    BlockReasonResource |
    BlockReasonInspection |
    BlockReasonExhausted
;
