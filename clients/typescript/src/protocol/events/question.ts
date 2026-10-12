// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * `ask_user` durably recording what it asked, so a client can show it
 * and a person answering later still has it in view (the reply itself
 * is an ordinary `message.sent` back to this agent).
 */
export interface EventKindQuestionAsked {
  "question": string;
  "type": "question.asked";
}
