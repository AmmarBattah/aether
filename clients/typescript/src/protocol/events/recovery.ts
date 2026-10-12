// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  RecoveryClass,
} from "../../protocol.ts";


export interface EventKindRecoveryClassified {
  "class": RecoveryClass;
  "detail": string;
  "type": "recovery.classified";
}
