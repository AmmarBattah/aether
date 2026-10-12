use crate::{LEAF_WITHHELD_TOOLS, Result, Runtime, RuntimeError, turn_engine};
use aether_core::id::AgentId;
use std::{collections::HashMap, sync::Arc};

pub(super) struct EngineSet {
    pub by_kind: HashMap<aether_engine::EngineKind, Arc<dyn aether_engine::Engine>>,
    pub unavailable: HashMap<aether_engine::EngineKind, String>,
    pub detect_native: bool,
}

pub(super) async fn detect_engines(models: &aether_model::Models) -> EngineSet {
    let mut engines = HashMap::new();
    let mut unavailable = HashMap::new();
    for provider in models.registry().providers().iter().filter(|p| p.enabled) {
        let Some(kind) = turn_engine::engine_kind(provider.protocol) else {
            continue;
        };
        let program = match &provider.auth.cli {
            Some(cli) => cli.status().await.program,
            None => None,
        };
        let Some(program) = program else {
            unavailable.insert(kind, format!("{} CLI not found", provider.id));
            continue;
        };
        match detect_provider_engine(provider, &program, models.config().model.prompt_cache_ttl)
            .await
        {
            Ok(engine) => {
                assert_eq!(
                    engine.kind(),
                    kind,
                    "detected engine must match its provider protocol"
                );
                assert!(
                    !engine.adapter().is_empty(),
                    "detected engine must identify its adapter"
                );
                engines.insert(kind, engine);
            }
            Err(reason) => {
                tracing::warn!(engine = %kind, %reason, "engine CLI unusable");
                unavailable.insert(kind, reason.to_string());
            }
        }
    }
    EngineSet {
        by_kind: engines,
        unavailable,
        detect_native: true,
    }
}

async fn detect_provider_engine(
    provider: &aether_model::registry::ProviderDef,
    program: &str,
    prompt_cache_ttl: Option<aether_core::model::PromptCacheTtl>,
) -> std::result::Result<Arc<dyn aether_engine::Engine>, aether_engine::EngineError> {
    use aether_engine::{ClaudeCodeEngine, CodexAppEngine, CodexEngine, EngineError};
    assert!(!program.is_empty(), "resolved engine path cannot be empty");
    assert!(
        provider.protocol.is_engine(),
        "only engine providers can be detected"
    );
    match provider.protocol {
        aether_model::Protocol::ClaudeCode => {
            let mut engine = ClaudeCodeEngine::detect_program(program)
                .await
                .map_err(|e| EngineError::Unsupported(e.to_string()))?;
            if let Some(ttl) = prompt_cache_ttl {
                engine = engine.with_prompt_cache_ttl(ttl);
            }
            if let Some(limitation) = engine.limitation() {
                tracing::warn!(%program, %limitation, "engine usable, with limits");
            }
            Ok(Arc::new(engine))
        }
        aether_model::Protocol::Codex => match CodexAppEngine::detect_program(program).await {
            Ok(engine) => Ok(Arc::new(engine)),
            Err(app_reason) => CodexEngine::detect_program(program)
                .await
                .map(|engine| Arc::new(engine) as Arc<dyn aether_engine::Engine>)
                .map_err(|exec_reason| {
                    EngineError::Unsupported(format!(
                        "{app_reason} (legacy exec fallback: {exec_reason})"
                    ))
                }),
        },
        _ => unreachable!("validated engine protocol"),
    }
}

impl Runtime {
    pub(crate) async fn provider_engine(
        &self,
        provider: &aether_model::registry::ProviderDef,
    ) -> std::result::Result<Arc<dyn aether_engine::Engine>, aether_engine::EngineError> {
        let kind = turn_engine::engine_kind(provider.protocol).ok_or_else(|| {
            aether_engine::EngineError::Unsupported("provider is not an engine".into())
        })?;
        if !self.detect_native_engines {
            return self.engines.get(&kind).cloned().ok_or_else(|| {
                aether_engine::EngineError::Unsupported(
                    self.engine_unavailable
                        .get(&kind)
                        .cloned()
                        .unwrap_or_else(|| format!("{kind} engine is unavailable")),
                )
            });
        }
        assert!(provider.enabled, "disabled engine provider must not run");
        let cli = provider.auth.cli.as_ref().ok_or_else(|| {
            aether_engine::EngineError::Unsupported("native engine needs CLI auth".into())
        })?;
        let status = cli.status().await;
        let program = status
            .program
            .ok_or(aether_engine::EngineError::Unsupported(status.detail))?;
        let key = (provider.id.clone(), program);
        if let Some(engine) = self.native_engines.lock().await.get(&key) {
            return Ok(engine.clone());
        }
        let engine = detect_provider_engine(
            provider,
            &key.1,
            self.models.config().model.prompt_cache_ttl,
        )
        .await?;
        self.native_engines.lock().await.insert(key, engine.clone());
        Ok(engine)
    }

    /// Engines detected on this machine, by provider protocol name.
    pub fn engines(&self) -> Vec<String> {
        self.engines.keys().map(|k| k.as_str().to_owned()).collect()
    }

    /// The channel of the engine turn that issued `token`, or a refusal. The
    /// token is per-turn, so a stale bridge cannot reach a later turn.
    fn engine_channel(
        &self,
        agent: AgentId,
        token: &str,
    ) -> Result<tokio::sync::mpsc::Sender<turn_engine::EngineRequest>> {
        let turns = self.engine_turns.lock();
        match turns.get(&agent) {
            Some(h) if h.token == token => Ok(h.tx.clone()),
            Some(_) => Err(RuntimeError::Invalid("engine token mismatch".into())),
            None => Err(RuntimeError::Invalid(format!(
                "no engine turn is running for {agent}"
            ))),
        }
    }

    /// Tools this agent may receive through Aether's API or native model path.
    pub(crate) fn agent_tool_specs(
        &self,
        agent: &aether_core::agent::Agent,
    ) -> Vec<aether_core::model::ToolSpec> {
        self.tools
            .specs_for(&agent.config.tools, &agent.scope)
            .into_iter()
            .filter(|spec| !agent.config.leaf || !LEAF_WITHHELD_TOOLS.contains(&spec.name.as_str()))
            .collect()
    }

    /// What an engine reaches through Aether: the harness tools, plus the MCP
    /// servers' tools this agent may use. The engine's own shell and file
    /// tools are its business, not Aether's.
    pub fn engine_tool_specs(
        &self,
        agent: &aether_core::agent::Agent,
    ) -> Vec<aether_core::model::ToolSpec> {
        let mut offered: std::collections::BTreeSet<String> =
            aether_tools::builtin::harness_tools()
                .iter()
                .map(|t| t.descriptor().spec.name)
                .collect();
        if let Some(mcp) = self.tools.mcp() {
            offered.extend(mcp.tools().iter().map(|t| t.descriptor().spec.name));
        }
        self.agent_tool_specs(agent)
            .into_iter()
            .filter(|s| offered.contains(&s.name))
            .collect()
    }

    /// The same list, for the bridge that presents it over MCP. Authenticated
    /// by the per-turn token so only the running turn's bridge can ask.
    pub fn engine_tools(
        &self,
        agent: AgentId,
        token: &str,
    ) -> Result<Vec<aether_core::model::ToolSpec>> {
        self.engine_channel(agent, token)?;
        let record = self
            .store
            .agent(agent)?
            .ok_or_else(|| RuntimeError::NotFound(agent.to_string()))?;
        Ok(self.engine_tool_specs(&record))
    }

    /// Route a bridge tool call or permission prompt to the engine turn that
    /// issued `token`. Returns the model-facing text and an error flag.
    pub async fn engine_tool(
        &self,
        agent: AgentId,
        token: &str,
        name: String,
        input: serde_json::Value,
    ) -> Result<(String, bool)> {
        let tx = self.engine_channel(agent, token)?;
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        tx.send(turn_engine::EngineRequest {
            name,
            input,
            reply: reply_tx,
        })
        .await
        .map_err(|_| RuntimeError::Internal("engine turn ended before answering".into()))?;
        reply_rx
            .await
            .map_err(|_| RuntimeError::Internal("engine turn dropped the request".into()))
    }
}
