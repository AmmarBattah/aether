// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.


/**
 * Tagged by `kind`.
 */
export interface RecurrenceInterval {
  "kind": "interval";
  "seconds": number;
}

/**
 * Five fields: minute, hour, day of month, month, day of week.
 */
export interface RecurrenceCron {
  "expression": string;
  "kind": "cron";
  "timezone": string;
}

export type Recurrence = RecurrenceInterval | RecurrenceCron;
