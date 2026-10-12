// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * The per-agent context layers an engine session has already been sent, as
 * digests of their rendered text.
 *
 * An engine session ordinarily retains every user turn it was sent, so a
 * layer whose text has not changed since it was last sent is normally
 * already in front of the model; sending it again would duplicate it in a
 * conversation that keeps both copies. That guarantee ends at the
 * engine's own context compaction - Claude Code's recorded system prompt is
 * reused only "until the conversation is compacted" - which can replace a
 * retained turn with a summary the model never sees again as the original
 * text. Aether requests its own compaction first in the common case, and
 * `turn_engine` clears this record on a sharp context drop as a cheap bound
 * against the other order, but neither proves the engine's compaction never
 * wins the race.
 *
 * The session id is part of the record: a cleared or replaced session
 * retains nothing, so its digests must not gate the next turn.
 */
export interface EngineSentLayers {
  "layers": Record<string, unknown>;
  "session": string;
}
