// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  ScheduleId,
} from "../../protocol.ts";


export interface ScheduleHistoryCommand {
  "cmd": "schedule_history";
  "id": ScheduleId;
  "limit": number;
}

export function scheduleHistory(args: Omit<ScheduleHistoryCommand, "cmd">): ScheduleHistoryCommand {
  return { cmd: "schedule_history", ...args };
}
