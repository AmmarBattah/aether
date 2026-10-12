// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Tagged by `state`.
 */
export interface ScheduleStatusActive {
  "next_due": string;
  "state": "active";
}

export interface ScheduleStatusPaused {
  "next_due": string;
  "state": "paused";
}

export interface ScheduleStatusCancelled {
  "at": string;
  "state": "cancelled";
}

export interface ScheduleStatusExhausted {
  "at": string;
  "state": "exhausted";
}

export interface ScheduleStatusFailed {
  "at": string;
  "error": string;
  "state": "failed";
}

export type ScheduleStatus =
    ScheduleStatusActive |
    ScheduleStatusPaused |
    ScheduleStatusCancelled |
    ScheduleStatusExhausted |
    ScheduleStatusFailed
;
