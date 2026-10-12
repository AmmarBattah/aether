// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  trigger,
} from "../../protocol.ts";

/**
 * When several occurrences elapsed between scheduler ticks, coalescing
 * delivers one trigger; skipping advances the clock without delivering one.
 * A tick that is late by less than one recurrence still delivers normally.
 */
export type MissedRunPolicy = "coalesce" | "skip";
