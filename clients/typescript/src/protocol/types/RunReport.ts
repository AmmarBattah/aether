// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  Provenance,
  RunBody,
  RunId,
} from "../../protocol.ts";

/**
 * A run's report: a header that says which run and when, and a body that
 * says what happened. Only the body is comparable across stores - the
 * header carries the local identity.
 */
export interface RunReport {
  "body": RunBody;
  "from_seq": number;
  "generated_at": string;
  "provenance"?: Provenance | null;
  "run": RunId;
  "schema": number;
  "to_seq": number;
}
