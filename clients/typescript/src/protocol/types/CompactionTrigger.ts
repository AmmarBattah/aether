// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Actor,
  ModelRef,
  TaskId,
  ToolCallId,
  compact,
  trigger,
} from "../../protocol.ts";

/**
 * What triggered a compaction.
 * Tagged by `trigger`.
 */
export interface CompactionTriggerTokenThreshold {
  "threshold": number;
  "tokens": number;
  "trigger": "token_threshold";
}

export interface CompactionTriggerTaskBoundary {
  "task": TaskId;
  "trigger": "task_boundary";
}

export interface CompactionTriggerLargeToolOutput {
  "call": ToolCallId;
  "trigger": "large_tool_output";
}

export interface CompactionTriggerLongWait {
  "trigger": "long_wait";
}

export interface CompactionTriggerExplicit {
  "by": Actor;
  "trigger": "explicit";
}

/**
 * The agent's model moved across the engine/API boundary. `from` is the
 * model that produced the context being folded, so it can compact it.
 */
export interface CompactionTriggerHandoff {
  "from": ModelRef;
  "session"?: string | null;
  "to": ModelRef;
  "trigger": "handoff";
}

export type CompactionTrigger =
    CompactionTriggerTokenThreshold |
    CompactionTriggerTaskBoundary |
    CompactionTriggerLargeToolOutput |
    CompactionTriggerLongWait |
    CompactionTriggerExplicit |
    CompactionTriggerHandoff
;
