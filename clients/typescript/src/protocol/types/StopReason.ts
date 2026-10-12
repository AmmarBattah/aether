// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Why the model stopped generating.
 * Tagged by `reason`.
 */
export interface StopReasonEndTurn {
  "reason": "end_turn";
}

export interface StopReasonToolUse {
  "reason": "tool_use";
}

export interface StopReasonMaxTokens {
  "reason": "max_tokens";
}

export interface StopReasonStopSequence {
  "reason": "stop_sequence";
}

export interface StopReasonRefusal {
  "category"?: string | null;
  "reason": "refusal";
}

export interface StopReasonPauseTurn {
  "reason": "pause_turn";
}

export interface StopReasonOther {
  "reason": "other";
  "value": string;
}

export type StopReason =
    StopReasonEndTurn |
    StopReasonToolUse |
    StopReasonMaxTokens |
    StopReasonStopSequence |
    StopReasonRefusal |
    StopReasonPauseTurn |
    StopReasonOther
;
