// Generated from clients/protocol.schema.json by scripts/generate_clients.py.
// Do not edit: change the Rust types and regenerate.

import type {
  AddApprovalRuleCommand,
  ArchiveCommand,
  AuthStatusCommand,
  CancelCommand,
  CompactCommand,
  ConfigureGithubWatcherCommand,
  CreateAgentCommand,
  CreateMemoryCommand,
  CreateScheduleCommand,
  CreateTaskCommand,
  EngineToolCommand,
  EngineToolsCommand,
  ExportRunCommand,
  ForgetMemoryCommand,
  GetAgentCommand,
  GetArtifactCommand,
  GetCheckpointCommand,
  GetEventsCommand,
  GetGithubWatcherCommand,
  GetInboxCommand,
  GetInvocationCommand,
  GetRunReportCommand,
  GetScheduleCommand,
  GetSnapshotCommand,
  GetTaskCommand,
  GetTranscriptCommand,
  GetTreeCommand,
  HelloCommand,
  ImportRunCommand,
  InterruptCommand,
  LatestCheckpointCommand,
  ListAgentsCommand,
  ListApprovalRulesCommand,
  ListApprovalsCommand,
  ListArtifactsCommand,
  ListInvocationsCommand,
  ListModelsCommand,
  ListRunsCommand,
  ListSchedulesCommand,
  ListTasksCommand,
  McpServersCommand,
  MemoriesCommand,
  PauseCommand,
  PinMemoryCommand,
  PingCommand,
  PreviewRetentionCommand,
  ProvidersCommand,
  PublishArtifactCommand,
  ReadArtifactCommand,
  ReplayCommand,
  ReplyToQuestionCommand,
  ResolveApprovalCommand,
  RestartCommand,
  RestoreMemoryCommand,
  ResumeCommand,
  RevokeApprovalRuleCommand,
  ScheduleHistoryCommand,
  SendMessageCommand,
  SetApprovalsCommand,
  ShutdownCommand,
  StatusCommand,
  SubscribeCommand,
  SyncRepositoryKnowledgeCommand,
  TriggerCommand,
  UnsubscribeCommand,
  UpdateAgentCommand,
  UpdateScheduleCommand,
  WakeCommand,
} from "../../protocol.ts";

export type Command =
  | AddApprovalRuleCommand
  | ArchiveCommand
  | AuthStatusCommand
  | CancelCommand
  | CompactCommand
  | ConfigureGithubWatcherCommand
  | CreateAgentCommand
  | CreateMemoryCommand
  | CreateScheduleCommand
  | CreateTaskCommand
  | EngineToolCommand
  | EngineToolsCommand
  | ExportRunCommand
  | ForgetMemoryCommand
  | GetAgentCommand
  | GetArtifactCommand
  | GetCheckpointCommand
  | GetEventsCommand
  | GetGithubWatcherCommand
  | GetInboxCommand
  | GetInvocationCommand
  | GetRunReportCommand
  | GetScheduleCommand
  | GetSnapshotCommand
  | GetTaskCommand
  | GetTranscriptCommand
  | GetTreeCommand
  | HelloCommand
  | ImportRunCommand
  | InterruptCommand
  | LatestCheckpointCommand
  | ListAgentsCommand
  | ListApprovalRulesCommand
  | ListApprovalsCommand
  | ListArtifactsCommand
  | ListInvocationsCommand
  | ListModelsCommand
  | ListRunsCommand
  | ListSchedulesCommand
  | ListTasksCommand
  | McpServersCommand
  | MemoriesCommand
  | PauseCommand
  | PinMemoryCommand
  | PingCommand
  | PreviewRetentionCommand
  | ProvidersCommand
  | PublishArtifactCommand
  | ReadArtifactCommand
  | ReplayCommand
  | ReplyToQuestionCommand
  | ResolveApprovalCommand
  | RestartCommand
  | RestoreMemoryCommand
  | ResumeCommand
  | RevokeApprovalRuleCommand
  | ScheduleHistoryCommand
  | SendMessageCommand
  | SetApprovalsCommand
  | ShutdownCommand
  | StatusCommand
  | SubscribeCommand
  | SyncRepositoryKnowledgeCommand
  | TriggerCommand
  | UnsubscribeCommand
  | UpdateAgentCommand
  | UpdateScheduleCommand
  | WakeCommand;
