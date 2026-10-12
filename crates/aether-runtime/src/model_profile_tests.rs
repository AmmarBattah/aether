use super::*;

#[tokio::test]
async fn global_profiles_apply_to_api_models_but_not_native_harnesses() {
    let config_dir = tempfile::tempdir().expect("config dir");
    std::fs::write(
        config_dir.path().join("config.toml"),
        "[model]\nprofiles = true\n",
    )
    .expect("config");
    let models = Arc::new(
        aether_model::Models::load(Some(config_dir.path().to_path_buf())).expect("models"),
    );
    let runtime = Runtime::start_with_engines(
        RuntimeConfig::default(),
        Arc::new(aether_store::SqliteStore::open_in_memory().expect("store")),
        models.clone(),
        Arc::new(ToolRegistry::with_builtins()),
        None,
        HashMap::new(),
    )
    .await
    .expect("runtime");

    for provider in ["claude-code", "codex"] {
        let resolved = models
            .resolve(&ModelRef::new(provider, "default"))
            .expect("native model");
        assert!(runtime.model_profile(&resolved).is_none(), "{provider}");
    }
    let api = models
        .resolve(&ModelRef::new("anthropic", "claude-sonnet-5-5"))
        .expect("API model");
    assert!(
        runtime
            .model_profile(&api)
            .expect("API profile remains enabled")
            .contains("Model profile: Claude.")
    );
    runtime.shutdown().await;
}
