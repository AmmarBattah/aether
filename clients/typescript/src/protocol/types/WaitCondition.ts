// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ApprovalId,
  TaskId,
} from "../../protocol.ts";

/**
 * What a waiting agent is waiting for. A typed expression the scheduler
 * evaluates; the model never polls. Leaves are single facts, and `Any`,
 * `All` and `AtLeast` combine them, so "wait for two of these five workers
 * or a deadline" is a value, not a prompt.
 * Tagged by `on`.
 */
/**
 * The task reached a terminal state.
 */
export interface WaitConditionTask {
  "id": TaskId;
  "on": "task";
}

export interface WaitConditionApproval {
  "id": ApprovalId;
  "on": "approval";
}

export interface WaitConditionMessageFrom {
  "agent": AgentId;
  "on": "message_from";
}

/**
 * Any inbox item at all.
 */
export interface WaitConditionInbox {
  "on": "inbox";
}

export interface WaitConditionDeadline {
  "at": string;
  "on": "deadline";
}

export interface WaitConditionAny {
  "of": Array<WaitCondition>;
  "on": "any";
}

export interface WaitConditionAll {
  "of": Array<WaitCondition>;
  "on": "all";
}

export interface WaitConditionAtLeast {
  "n": number;
  "of": Array<WaitCondition>;
  "on": "at_least";
}

export type WaitCondition =
    WaitConditionTask |
    WaitConditionApproval |
    WaitConditionMessageFrom |
    WaitConditionInbox |
    WaitConditionDeadline |
    WaitConditionAny |
    WaitConditionAll |
    WaitConditionAtLeast
;
