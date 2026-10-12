//! Where a command goes: the daemon in production, a double in tests.

use aether_protocol::{Command, Reply};
use async_trait::async_trait;

use crate::Client;

/// One command in, one reply out. Everything that drives a fleet from
/// outside - the MCP server, `aether serve` - is written against this so it
/// can be tested without a daemon.
#[async_trait]
pub trait CommandSink: Send + Sync {
    async fn call(&self, command: Command) -> anyhow::Result<Reply>;
}

/// The daemon over its own connection.
#[derive(Debug)]
pub struct DaemonSink(pub Client);

#[async_trait]
impl CommandSink for DaemonSink {
    async fn call(&self, command: Command) -> anyhow::Result<Reply> {
        Ok(self.0.request(command).await?)
    }
}
