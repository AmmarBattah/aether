// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ArtifactId,
  ToolCallId,
} from "../../protocol.ts";

/**
 * How a tool call in this turn ended.
 * Tagged by `result`.
 */
/**
 * Requested, and the log carries no end for it.
 */
export interface ToolResultPending {
  "result": "pending";
}

export interface ToolResultSucceeded {
  "artifacts"?: Array<ArtifactId>;
  "content": string;
  "duration_ms": number;
  "is_error"?: boolean;
  "result": "succeeded";
  "truncated"?: boolean;
}

export interface ToolResultFailed {
  "duration_ms"?: number | null;
  "error": string;
  "result": "failed";
  "retryable": boolean;
}

export interface ToolResultDenied {
  "reason": string;
  "result": "denied";
}

export interface ToolResultCancelled {
  "result": "cancelled";
}

/**
 * Answered from a committed outcome; the call did not run again.
 */
export interface ToolResultReplayed {
  "original": ToolCallId;
  "result": "replayed";
}

export type ToolResult =
    ToolResultPending |
    ToolResultSucceeded |
    ToolResultFailed |
    ToolResultDenied |
    ToolResultCancelled |
    ToolResultReplayed
;
