// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentStatusKind,
  BlockReason,
  WaitCondition,
} from "../../protocol.ts";

/**
 * Tagged by `state`.
 */
export interface AgentStatusCreated {
  "state": "created";
}

export interface AgentStatusReady {
  "state": "ready";
}

export interface AgentStatusRunning {
  "state": "running";
}

export interface AgentStatusWaiting {
  "condition": WaitCondition;
  "state": "waiting";
}

export interface AgentStatusBlocked {
  "reason": BlockReason;
  "state": "blocked";
}

export interface AgentStatusCompleted {
  "state": "completed";
  "summary"?: string | null;
}

export interface AgentStatusFailed {
  "error": string;
  "state": "failed";
}

export interface AgentStatusCancelled {
  "reason"?: string | null;
  "state": "cancelled";
}

/**
 * Read out of another daemon's export. Terminal by construction: an
 * imported agent is a record of work that happened elsewhere, and no
 * local command or scheduler pass may give it a turn.
 */
export interface AgentStatusImported {
  "state": "imported";
  "was": AgentStatusKind;
}

export type AgentStatus =
    AgentStatusCreated |
    AgentStatusReady |
    AgentStatusRunning |
    AgentStatusWaiting |
    AgentStatusBlocked |
    AgentStatusCompleted |
    AgentStatusFailed |
    AgentStatusCancelled |
    AgentStatusImported
;
