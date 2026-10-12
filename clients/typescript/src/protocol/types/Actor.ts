// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  ApprovalRuleId,
  OperatorOrigin,
  ToolCallId,
} from "../../protocol.ts";

/**
 * Who caused an event.
 * Tagged by `actor`.
 */
export interface ActorSystem {
  "actor": "system";
}

export interface ActorScheduler {
  "actor": "scheduler";
}

export interface ActorUser {
  "actor": "user";
  "name"?: string | null;
  "origin"?: OperatorOrigin | null;
}

export interface ActorAgent {
  "actor": "agent";
  "id": AgentId;
}

export interface ActorTool {
  "actor": "tool";
  "call": ToolCallId;
}

/**
 * A remembered decision answered on a person's behalf.
 */
export interface ActorRule {
  "actor": "rule";
  "rule": ApprovalRuleId;
}

export interface ActorClient {
  "actor": "client";
  "id": string;
}

/**
 * An actor that acted on another machine, read out of an export. The
 * original travels for the record; nothing here acted.
 */
export interface ActorImported {
  "actor": "imported";
  "was": Actor;
}

export type Actor =
    ActorSystem |
    ActorScheduler |
    ActorUser |
    ActorAgent |
    ActorTool |
    ActorRule |
    ActorClient |
    ActorImported
;
