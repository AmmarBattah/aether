// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  InboxItem,
  InboxItemId,
} from "../../protocol.ts";


export interface EventKindInboxDelivered {
  "item": InboxItem;
  "type": "inbox.delivered";
}

export interface EventKindInboxConsumed {
  "items": Array<InboxItemId>;
  "type": "inbox.consumed";
  "up_to": number;
}
