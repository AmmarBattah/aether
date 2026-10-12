// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
} from "../../protocol.ts";

/**
 * Operator-only reply to one unanswered, unsuperseded question event.
 * A stale question is refused without delivering anything.
 */
export interface ReplyToQuestionCommand {
  "cmd": "reply_to_question";
  "content": string;
  "idempotency_key"?: string | null;
  "question_seq": number;
  "to": AgentId;
}

export function replyToQuestion(args: Omit<ReplyToQuestionCommand, "cmd">): ReplyToQuestionCommand {
  return { cmd: "reply_to_question", ...args };
}
