//! Turning `[[mcp.servers]]` configuration into connectable servers, and
//! turning their health transitions into durable events.

use std::sync::Arc;

use aether_core::mcp::McpServerHealth;
use aether_model::config::{McpSection, McpServerSection};
use aether_runtime::Runtime;
use aether_tools::{McpServerSpec, McpTransportSpec};

/// Build the specs the tool runtime connects with. A token is resolved here,
/// once, from the environment the daemon started with: Aether never stores it.
pub(crate) fn specs(section: &McpSection) -> Vec<McpServerSpec> {
    section.servers.iter().filter_map(spec).collect()
}

fn spec(section: &McpServerSection) -> Option<McpServerSpec> {
    // Configuration was validated at load; a server that slips through with
    // no transport is skipped loudly rather than connected to nothing.
    let transport = match (&section.command, &section.url) {
        (Some(command), None) => McpTransportSpec::Stdio {
            command: command.into(),
            args: section.args.clone(),
            env: section.env.clone(),
            cwd: section.cwd.clone(),
        },
        (None, Some(url)) => McpTransportSpec::Http {
            url: url.clone(),
            token: section.token_env.as_ref().and_then(|var| {
                let value = std::env::var(var).ok().filter(|v| !v.is_empty());
                if value.is_none() {
                    tracing::warn!(
                        server = %section.name,
                        variable = %var,
                        "mcp: token environment variable is unset; connecting without one"
                    );
                }
                value
            }),
        },
        _ => {
            tracing::error!(
                server = %section.name,
                "mcp: server names no usable transport; skipping it"
            );
            return None;
        }
    };
    Some(McpServerSpec {
        name: section.name.clone(),
        transport,
        classes: section.classes.clone(),
        send_caller_meta: section.send_caller_meta,
    })
}

/// Drain the manager's health channel into the event log for as long as the
/// daemon lives.
///
/// The runtime owns the registry, which owns the manager, which owns the
/// sender: holding a strong reference here would make the runtime undroppable.
/// A weak one ends the task with the runtime instead.
pub(crate) fn spawn_recorder(
    runtime: &Arc<Runtime>,
    mut changes: tokio::sync::mpsc::UnboundedReceiver<McpServerHealth>,
) {
    let runtime = Arc::downgrade(runtime);
    tokio::spawn(async move {
        while let Some(health) = changes.recv().await {
            let Some(runtime) = runtime.upgrade() else {
                break;
            };
            if let Err(e) = runtime.record_mcp_state(&health) {
                tracing::error!(server = %health.name, error = %e, "could not record mcp state");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(name: &str) -> McpServerSection {
        McpServerSection {
            name: name.into(),
            command: None,
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            url: None,
            token_env: None,
            classes: Default::default(),
            send_caller_meta: false,
        }
    }

    #[test]
    fn a_server_without_a_transport_is_skipped_not_connected_to_nothing() {
        let servers = vec![
            McpServerSection {
                command: Some("mock".into()),
                ..section("good")
            },
            section("broken"),
        ];
        let specs = specs(&McpSection { servers });
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "good");
    }

    /// A config written before `send_caller_meta` existed, for a server that
    /// refuses params it does not know, must keep working with no migration.
    #[tokio::test]
    async fn an_existing_config_keeps_a_strict_server_working() {
        let mock = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/mock_mcp_server.sh"
        );
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("config.toml"),
            format!(
                "[[mcp.servers]]\nname = \"strict\"\ncommand = \"bash\"\n\
                 args = [{mock:?}]\nenv = {{ AETHER_MOCK_MCP_STRICT = \"1\" }}\n"
            ),
        )
        .expect("write config");
        let config = aether_model::config::load_config(dir.path()).expect("config");
        let specs = specs(&config.mcp);
        assert!(!specs[0].send_caller_meta, "off unless configured");

        let manager = aether_tools::McpManager::new(specs, None, Vec::new().into());
        manager.discover_all().await;
        let caller = aether_tools::McpCaller {
            agent_id: aether_core::id::AgentId::new(),
            task_id: None,
        };
        let result = manager
            .call(
                "strict",
                "read_note",
                serde_json::json!({"key": "k"}),
                &caller,
            )
            .await
            .expect("the strict server is sent nothing it refuses");
        assert_eq!(result.text, "note contents");
    }
}
