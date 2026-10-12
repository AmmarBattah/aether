// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  archive,
} from "../../protocol.ts";

/**
 * What an archive command marks: one finished agent, every finished agent of
 * a run, or every finished agent there is.
 */
export type ArchiveTarget = "all_finished" | Record<string, unknown>;
