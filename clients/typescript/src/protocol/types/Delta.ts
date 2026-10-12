// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Transient streaming signal. Not persisted; the completed message is the event.
 * Tagged by `delta`.
 */
export interface DeltaTurnStarted {
  "delta": "turn_started";
  "turn": number;
}

export interface DeltaText {
  "delta": "text";
  "text": string;
}

export interface DeltaThinking {
  "delta": "thinking";
  "text": string;
}

export interface DeltaToolInput {
  "delta": "tool_input";
  "name": string;
  "partial_json": string;
}

export interface DeltaTurnFinished {
  "delta": "turn_finished";
}

export type Delta =
    DeltaTurnStarted |
    DeltaText |
    DeltaThinking |
    DeltaToolInput |
    DeltaTurnFinished
;
