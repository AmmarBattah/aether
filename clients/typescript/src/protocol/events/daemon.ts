// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.



export interface EventKindDaemonStarted {
  "pid": number;
  "type": "daemon.started";
  "version": string;
}

export interface EventKindDaemonStopped {
  "background_processes_stopped"?: number;
  "type": "daemon.stopped";
}
