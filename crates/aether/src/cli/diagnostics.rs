use super::{Client, ClientError, PathBuf};

/// Print installation checks and return the worst check's exit code.
pub async fn doctor(socket: PathBuf, explicit_socket: bool, json: bool) -> anyhow::Result<i32> {
    let report = doctor_report(socket, explicit_socket).await;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(report.level.exit_code());
    }
    for check in &report.checks {
        println!(
            "{:<5} {:<10} {}",
            check.level.as_str(),
            check.name,
            super::sane(&check.detail)
        );
        if let Some(fix) = &check.fix {
            println!("      {:<10} → {}", "", super::sane(fix));
        }
    }
    println!("\n{}", report.level.as_str());
    Ok(report.level.exit_code())
}

/// Collect the same installation checks for doctor and diagnostic bundles.
pub(super) async fn doctor_report(socket: PathBuf, explicit_socket: bool) -> crate::doctor::Report {
    use crate::doctor::{self, Check, Paths, Report, SocketState};

    let command = super::command_hint(&socket, explicit_socket);
    let paths = Paths {
        config_dir: aether_model::config::config_dir(None),
        data_dir: aether_protocol::paths::data_dir(),
        socket: socket.clone(),
    };
    // A daemon answers some checks better than the client can; without one
    // the client still answers every check it can reach on its own. Why the
    // connection failed is kept: a daemon that refused this build is a fault
    // to report, not the absence of one.
    let connected = Client::connect(&socket).await;
    let refusal = match &connected {
        Err(ClientError::Incompatible(why)) => Some(why.clone()),
        _ => None,
    };
    let client = connected.ok();
    let status = match &client {
        Some(c) => c.status().await.ok(),
        None => None,
    };
    let auth = match &client {
        Some(c) => c.auth_status().await.unwrap_or_default(),
        None => Vec::new(),
    };
    let remote = match &client {
        Some(c) => remote_status(c).await,
        None => None,
    };
    let mcp_health = status.as_ref().map(|s| s.mcp.clone()).unwrap_or_default();
    // A config that does not load is `check_config`'s to report.
    let config = aether_model::config::load_config(&paths.config_dir).ok();
    let configured_mcp = config.as_ref().map_or(0, |c| c.mcp.servers.len());
    let disk_floor = match &config {
        Some(c) => c.scheduler.disk_floor().ok().flatten(),
        None => Some(aether_core::disk::DiskFloor::Default),
    };

    // Credentials live with the client, so `aether auth`'s answer is the
    // daemon's; without one, say so rather than guessing.
    let providers = if client.is_some() {
        doctor::check_providers(&auth, &command)
    } else {
        Check::no_daemon_providers(&command)
    };
    let daemon = match (status.as_ref(), refusal.as_deref()) {
        (Some(status), _) => SocketState::Answering(status),
        (None, Some(why)) => SocketState::Refused(why),
        (None, None) => SocketState::Silent,
    };
    // Written in the order they are printed. It used to be built short and
    // have providers spliced in at index 4, which was right only for as long
    // as nobody reordered the list.
    Report::new(vec![
        doctor::check_config(&paths),
        doctor::check_socket(&paths, daemon, &command),
        doctor::check_store(&paths),
        doctor::check_disk(
            &paths,
            disk_floor,
            status.as_ref().and_then(|s| s.low_disk.as_ref()),
        ),
        doctor::check_logs(&crate::logging::log_file("aetherd.log")),
        providers,
        doctor::check_engines(&engines_found(&auth).await, &command),
        cli_installs(&auth, &paths.config_dir).await,
        doctor::check_mcp(configured_mcp, &mcp_health, &command),
        doctor::check_remote(remote.as_ref(), &command),
    ])
}

async fn cli_installs(
    auth: &[aether_protocol::AuthInfo],
    config_dir: &std::path::Path,
) -> crate::doctor::Check {
    let models = match aether_model::Models::load(Some(config_dir.to_path_buf())) {
        Ok(models) => models,
        Err(error) => {
            return crate::doctor::Check::warn(
                "cli-installs",
                error.to_string(),
                "fix the provider configuration first",
            );
        }
    };
    let mut installs = Vec::new();
    for provider in models.registry().providers().iter().filter(|p| p.enabled) {
        let Some(cli) = &provider.auth.cli else {
            continue;
        };
        let name = match provider.id.as_str() {
            "claude-code" => "claude",
            "codex" => "codex",
            _ => &cli.program,
        };
        let paths = aether_model::auth::cli::discover_programs(
            name,
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .as_deref(),
        );
        if paths.len() < 2 {
            continue;
        }
        let used = match auth.iter().find(|info| info.provider == provider.id) {
            Some(info) => info.program.clone(),
            None => cli.status().await.program,
        };
        installs.push((provider.id.clone(), paths, used));
    }
    crate::doctor::check_cli_installs(&installs)
}

/// The daemon's remote access status, or `None` from a daemon that predates
/// the command or does not answer it.
async fn remote_status(client: &Client) -> Option<aether_protocol::remote::RemoteStatus> {
    match client.request(aether_protocol::Command::GetRemote).await {
        Ok(aether_protocol::Reply::Remote(status)) => Some(status),
        _ => None,
    }
}

/// The engine CLIs on this machine: ready, usable with a known limitation, or
/// unavailable and why - not installed, unreadable output, or a
/// missing capability - rather than a bare `None`.
async fn engines_found(
    auth: &[aether_protocol::AuthInfo],
) -> Vec<(String, crate::doctor::EngineStatus)> {
    use crate::doctor::EngineStatus;
    use aether_engine::Engine as _;
    let program = |provider: &str, fallback: &str| {
        auth.iter()
            .find(|info| info.provider == provider)
            .and_then(|info| info.program.clone())
            .unwrap_or_else(|| fallback.to_owned())
    };
    let claude =
        match aether_engine::ClaudeCodeEngine::detect_program(&program("claude-code", "claude"))
            .await
        {
            Ok(e) => match e.limitation() {
                Some(limitation) => EngineStatus::Limited {
                    adapter: e.adapter(),
                    limitation,
                },
                None => EngineStatus::Ready(e.adapter()),
            },
            Err(e) => EngineStatus::Unavailable(e.to_string()),
        };
    let codex =
        match aether_engine::CodexAppEngine::detect_program(&program("codex", "codex")).await {
            Ok(e) => EngineStatus::Ready(e.adapter()),
            Err(app_reason) => {
                match aether_engine::CodexEngine::detect_program(&program("codex", "codex")).await {
                    Ok(e) => EngineStatus::Ready(e.adapter()),
                    // The common case - codex simply absent - fails both probes with
                    // the identical reason; do not repeat it.
                    Err(exec_reason) if exec_reason == app_reason => {
                        EngineStatus::Unavailable(app_reason.to_string())
                    }
                    Err(exec_reason) => EngineStatus::Unavailable(format!(
                        "{app_reason} (legacy exec fallback: {exec_reason})"
                    )),
                }
            }
        };
    vec![
        ("claude-code".to_owned(), claude),
        ("codex".to_owned(), codex),
    ]
}
