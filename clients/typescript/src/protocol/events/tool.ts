// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  IdempotencyKey,
  SideEffectClass,
  ToolCall,
  ToolCallId,
  ToolOutcome,
} from "../../protocol.ts";


export interface EventKindToolRequested {
  "call": ToolCall;
  "side_effect": SideEffectClass;
  "type": "tool.requested";
}

export interface EventKindToolStarted {
  "call": ToolCallId;
  "type": "tool.started";
}

export interface EventKindToolSucceeded {
  "call": ToolCallId;
  "outcome": ToolOutcome;
  "type": "tool.succeeded";
}

export interface EventKindToolFailed {
  "call": ToolCallId;
  "duration_ms"?: number | null;
  "error": string;
  "retryable": boolean;
  "type": "tool.failed";
}

export interface EventKindToolCancelled {
  "call": ToolCallId;
  "type": "tool.cancelled";
}

export interface EventKindToolDenied {
  "call": ToolCallId;
  "reason": string;
  "type": "tool.denied";
}

/**
 * The call carried an idempotency key already committed for this agent,
 * so the recorded outcome was returned instead of running again.
 */
export interface EventKindToolReplayed {
  "call": ToolCallId;
  "key": IdempotencyKey;
  "original": ToolCallId;
  "type": "tool.replayed";
}

/**
 * A retryable failure is being tried again, within the class's policy.
 */
export interface EventKindToolRetried {
  "attempt": number;
  "call": ToolCallId;
  "error": string;
  "of": number;
  "type": "tool.retried";
}
