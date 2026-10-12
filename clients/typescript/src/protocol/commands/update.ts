// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AgentId,
  AgentPatch,
  ScheduleAction,
  ScheduleId,
} from "../../protocol.ts";


export interface UpdateAgentCommand {
  "cmd": "update_agent";
  "id": AgentId;
  "patch": AgentPatch;
}

export function updateAgent(args: Omit<UpdateAgentCommand, "cmd">): UpdateAgentCommand {
  return { cmd: "update_agent", ...args };
}

export interface UpdateScheduleCommand {
  "cmd": "update_schedule";
  "action": ScheduleAction;
  "id": ScheduleId;
}

export function updateSchedule(args: Omit<UpdateScheduleCommand, "cmd">): UpdateScheduleCommand {
  return { cmd: "update_schedule", ...args };
}
