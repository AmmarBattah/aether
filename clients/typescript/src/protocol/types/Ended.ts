// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * How one agent or task ended.
 * Tagged by `outcome`.
 */
export interface EndedRunning {
  "outcome": "running";
}

export interface EndedCompleted {
  "outcome": "completed";
  "summary"?: string | null;
}

export interface EndedFailed {
  "error": string;
  "outcome": "failed";
}

export interface EndedCancelled {
  "outcome": "cancelled";
  "reason"?: string | null;
}

export type Ended = EndedRunning | EndedCompleted | EndedFailed | EndedCancelled;
