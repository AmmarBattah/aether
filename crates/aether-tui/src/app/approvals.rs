//! Application approvals.
use aether_core::tool::ApprovalRequest;

use super::{App, Command, Effect, Then};

impl App {
    pub(super) fn approvals_request(&self) -> Effect {
        Effect::Request {
            command: Command::ListApprovals,
            then: Then::Approvals {
                revision: self.approvals_revision,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalKind<'a> {
    Command(&'a str),
    Network,
    File(&'a str),
    Other,
}

pub(crate) fn kind(request: &ApprovalRequest) -> ApprovalKind<'_> {
    let tool = request.tool.to_ascii_lowercase();
    let argument = request
        .argument
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(command) = argument
        && (tool == "bash" || tool.contains("shell"))
    {
        return ApprovalKind::Command(command);
    }
    if tool.starts_with("web") || request.resources.iter().any(|r| r.0 == "network") {
        return ApprovalKind::Network;
    }
    match request
        .resources
        .iter()
        .find_map(|r| r.0.strip_prefix("file:"))
    {
        Some(path) => ApprovalKind::File(path),
        None => ApprovalKind::Other,
    }
}

/// What an approval would let the agent do, without its raw JSON description.
pub(crate) fn wants(request: &ApprovalRequest) -> String {
    let argument = request
        .argument
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty());
    match kind(request) {
        ApprovalKind::Command(command) => format!("Wants to run {command}"),
        ApprovalKind::Network => match argument {
            Some(query) if request.tool.to_ascii_lowercase().contains("search") => {
                format!("Wants to search the web for {query}")
            }
            Some(target) => format!("Wants to reach {target}"),
            None => "Wants to reach the network".to_owned(),
        },
        ApprovalKind::File(path) => format!("Wants to change {path}"),
        ApprovalKind::Other => match argument {
            Some(argument) => format!("Wants to use {} on {argument}", request.tool),
            None => format!("Wants to use {}", request.tool),
        },
    }
}

impl App {
    /// Ask for MCP health. `refresh` reconnects every server first, which
    /// can take as long as the slowest one.
    pub(super) fn mcp_request(&self, refresh: bool) -> Effect {
        Effect::Request {
            command: Command::McpServers {
                refresh,
                server: None,
            },
            then: Then::McpServers,
        }
    }

    pub(super) fn rules_request(&self) -> Effect {
        Effect::Request {
            command: Command::ListApprovalRules {
                include_revoked: false,
            },
            then: Then::ApprovalRules,
        }
    }
}
