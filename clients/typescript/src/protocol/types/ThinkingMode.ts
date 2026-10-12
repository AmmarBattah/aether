// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * How the model should think. Adapters honor what the capability profile
 * supports and report the rest as unsupported.
 * Tagged by `mode`.
 */
export interface ThinkingModeOff {
  "mode": "off";
}

export interface ThinkingModeAdaptive {
  "mode": "adaptive";
}

export interface ThinkingModeBudget {
  "mode": "budget";
  "tokens": number;
}

export type ThinkingMode = ThinkingModeOff | ThinkingModeAdaptive | ThinkingModeBudget;
