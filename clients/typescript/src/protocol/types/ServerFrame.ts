// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  Delta,
  Event,
  InvocationId,
  Result_of_Reply_or_ErrorReply,
} from "../../protocol.ts";

/**
 * Daemon to client.
 * Tagged by `frame`.
 */
export interface ServerFrameReply {
  "frame": "reply";
  "id": number;
  "result": Result_of_Reply_or_ErrorReply;
}

export interface ServerFrameEvent {
  "event": Event;
  "frame": "event";
  "sub": number;
}

export interface ServerFrameDelta {
  "agent": AgentId;
  "delta": Delta;
  "frame": "delta";
  "invocation": InvocationId;
  "sub": number;
}

export type ServerFrame = ServerFrameReply | ServerFrameEvent | ServerFrameDelta;
