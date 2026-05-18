use app_server_core::llm::{
    AgentModelSource, AgentProviderKind, DiscoveredProviderModel, ModelDiscoveryStatus,
    RigAgentConfig, RigAgentConfigSelection, WebSearchAuth, WebSearchHttpMethod, WebSearchParams,
    apply_provider_model_discovery, load_agent_provider_registry, load_rig_agent_config,
    load_rig_agent_config_with_discovery, merge_provider_models,
    openai_compatible_models_to_discovered, rig_config_from_registry_selection,
};
use app_server_core::{
    AgentExecutionScope, AgentTurnInput, build_rig_prompt_and_history, build_turn_context,
    cadquery_agent_system_prompt, extract_cadquery_code, hosted_tool_requests_for_config,
    rig_agent_additional_params, rig_agent_temperature_param,
};
use app_server_protocol::{
    AgentMode, CadQueryObjectKind, ChatMessageRecord, ChatRole, PathHandle, SelectionKind,
    SelectionRef, WorkspaceId,
};
use rig::completion::Message;

#[test]
fn extract_cadquery_code_from_python_fenced_block() {
    let response = "Here is the code:\n```python\nimport cadquery as cq\nresult = cq.Workplane().box(10, 10, 10)\n```\nDone.";
    let code = extract_cadquery_code(response);
    assert_eq!(
        code,
        "import cadquery as cq\nresult = cq.Workplane().box(10, 10, 10)"
    );
}

#[test]
fn cadquery_agent_prompt_requires_model_description_and_named_refs() {
    let prompt = cadquery_agent_system_prompt();
    assert!(prompt.contains("MODEL_DESCRIPTION"));
    assert!(prompt.contains("MODEL_DETAILS"));
    assert!(prompt.contains("key_dimensions"));
    assert!(prompt.contains("manufacturing_or_placement_constraints"));
    assert!(prompt.contains("REFS.features"));
    assert!(prompt.contains("export_formats"));
    assert!(prompt.contains("export_targets"));
}

#[test]
fn extract_cadquery_code_from_generic_fenced_block() {
    let response = "```\nimport cadquery as cq\nresult = cq.Workplane().box(5, 5, 5)\n```";
    let code = extract_cadquery_code(response);
    assert_eq!(
        code,
        "import cadquery as cq\nresult = cq.Workplane().box(5, 5, 5)"
    );
}

#[test]
fn extract_cadquery_code_returns_whole_response_when_no_fence() {
    let response = "import cadquery as cq\nresult = cq.Workplane().box(1, 1, 1)";
    let code = extract_cadquery_code(response);
    assert_eq!(code, response);
}

#[test]
fn extract_cadquery_code_prefers_python_fence_over_generic() {
    let response = "```\ngeneric\n```\n\n```python\npython_code\n```";
    let code = extract_cadquery_code(response);
    assert_eq!(code, "python_code");
}

#[test]
fn extract_cadquery_code_trims_whitespace_inside_fence() {
    let response = "```python\n  \n  code_here  \n  \n```";
    let code = extract_cadquery_code(response);
    assert_eq!(code, "code_here");
}

#[test]
fn build_rig_prompt_and_history_includes_prompt_context_and_history() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "explain fillet".into(),
        history: vec![chat_msg("user", "hi"), chat_msg("assistant", "hello")],
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let (prompt, history) = build_rig_prompt_and_history(&input);
    assert!(prompt.contains("Mode: Agent"));
    assert!(prompt.contains("explain fillet"));
    assert!(matches!(history.first(), Some(Message::User { .. })));
    assert!(matches!(history.get(1), Some(Message::Assistant { .. })));
}

#[test]
fn build_rig_prompt_and_history_skips_empty_and_tool_history() {
    let input = AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "design a lid".into(),
        history: vec![
            chat_msg("user", "initial"),
            empty_msg("assistant"),
            tool_msg("tool result"),
        ],
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let (_prompt, history) = build_rig_prompt_and_history(&input);
    assert_eq!(history.len(), 1);
    assert!(matches!(history[0], Message::User { .. }));
}

#[test]
fn build_turn_context_includes_mode_plan_ref_and_selection() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: vec![test_selection()],
        active_selection_index: Some(0),
        plan_ref: Some(
            PathHandle::new(WorkspaceId::new("ws"), ["plans", "2026050100-lid"]).unwrap(),
        ),
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: Some(AgentExecutionScope::for_plan(
            "plans/2026050100-lid",
            "plans/2026050100-lid/plan-result.md",
            "parts/lid.py",
            CadQueryObjectKind::Part,
            vec!["parts/lid.py".into()],
            Vec::new(),
            vec!["outputs/lid.step".into()],
        )),
    };
    let context = build_turn_context(&input);
    assert!(context.contains("Mode: Agent"));
    assert!(context.contains("Plan ref: plans/2026050100-lid"));
    assert!(context.contains("Execution scope"));
    assert!(context.contains("target_path=parts/lid.py"));
    assert!(context.contains("plan_result_path=plans/2026050100-lid/plan-result.md"));
    assert!(context.contains("Web preview selection"));
}

#[test]
fn build_turn_context_omits_selection_when_empty() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("Mode: Agent"));
    assert!(!context.contains("Web preview selection"));
    assert!(!context.contains("Plan ref"));
}

#[test]
fn build_turn_context_includes_context_refs() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: vec!["@face[top_lid:f_0]".into(), "@part[bottom_case]".into()],
        native_web_search_enabled: true,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("context refs"));
    assert!(context.contains("@face[top_lid:f_0]"));
    assert!(context.contains("@part[bottom_case]"));
    assert!(context.contains("`web_search`: available this turn"));
}

#[test]
fn build_turn_context_lists_current_turn_tools() {
    let input = AgentTurnInput {
        mode: AgentMode::Plan,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: true,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);

    assert!(context.contains("Provider-native tools"));
    assert!(context.contains("`web_search`: available this turn"));
    assert!(context.contains("Current turn app tools"));
    assert!(context.contains("inspect this current-turn app tool list"));
    assert!(context.contains("- `read_file`: Read a bounded UTF-8 text file"));
    assert!(context.contains("- `save_cad_plan`: Persist a structured Markdown CAD Plan"));
    assert!(!context.contains("- `write_file`:"));
}

#[test]
fn build_turn_context_shows_function_web_search_when_available() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: true,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(
        context.contains("`web_search` and `fetch_url` app tools are available"),
        "should mention function web search tools"
    );
    assert!(
        context.contains("cite the source URLs"),
        "should include citation instruction"
    );
    assert!(
        context.contains("- `web_search`:"),
        "web_search should appear in app tool list"
    );
    assert!(
        context.contains("- `fetch_url`:"),
        "fetch_url should appear in app tool list"
    );
}

#[test]
fn build_turn_context_hides_web_search_tool_when_unavailable() {
    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "unused".into(),
        history: Vec::new(),
        selections: Vec::new(),
        active_selection_index: None,
        plan_ref: None,
        context_refs: Vec::new(),
        native_web_search_enabled: false,
        function_web_search_available: false,
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(
        context.contains("Web search: not available"),
        "should state web search is not available"
    );
    assert!(
        !context.contains("- `web_search`:"),
        "web_search should not appear in app tool list"
    );
    assert!(
        context.contains("- `fetch_url`:"),
        "fetch_url should always appear in app tool list"
    );
}

#[test]
fn rig_agent_config_debug_hides_api_key() {
    let config = RigAgentConfig {
        provider_id: "openai".into(),
        provider_kind: AgentProviderKind::OpenAiResponses,
        api_key: "sk-secret-key-12345".into(),
        model: "gpt-5.2".into(),
        timeout_secs: 60,
        max_tokens: 8192,
        temperature: 0.7,
        reasoning_effort: Some("high".into()),
        service_label: Some("default".into()),
        native_web_search: true,
        anthropic_version: None,
        base_url: None,
    };
    let debug = format!("{:?}", config);
    assert!(!debug.contains("sk-secret-key-12345"));
    assert!(debug.contains("***"));
    assert!(debug.contains("gpt-5.2"));
    assert!(debug.contains("native_web_search"));
}

#[test]
fn agent_provider_kind_as_str_matches_config_values() {
    assert_eq!(
        AgentProviderKind::OpenAiResponses.as_str(),
        "openai_responses"
    );
    assert_eq!(
        AgentProviderKind::OpenAiCompletions.as_str(),
        "openai_completions"
    );
    assert_eq!(AgentProviderKind::Anthropic.as_str(), "anthropic");
}

#[test]
fn rig_agent_additional_params_omit_web_search_when_disabled() {
    let config = test_config(false);
    let params = rig_agent_additional_params(&config).expect("reasoning params");
    assert_eq!(params["reasoning"]["effort"], "high");
    assert!(params.get("tools").is_none());
}

#[test]
fn rig_agent_additional_params_omits_reasoning_when_none() {
    let mut config = test_config(false);
    config.reasoning_effort = None;
    config.service_label = None;

    assert!(rig_agent_additional_params(&config).is_none());
}

#[test]
fn rig_agent_additional_params_include_hosted_web_search_when_enabled() {
    let config = test_config(true);
    let params = rig_agent_additional_params(&config).expect("web search params");
    assert_eq!(params["reasoning"]["effort"], "high");
    assert_eq!(params["service_tier"], "default");
    assert_eq!(params["tools"][0]["type"], "web_search");
}

#[test]
fn hosted_tool_requests_include_openai_responses_web_search_when_enabled() {
    let config = test_config(true);

    let requests = hosted_tool_requests_for_config(&config);

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].tool_type, "web_search");
    assert_eq!(requests[0].provider_tool_type, "web_search");
    assert_eq!(requests[0].provider_tool_name, None);
}

#[test]
fn hosted_tool_requests_include_anthropic_web_search_when_enabled() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::Anthropic;

    let requests = hosted_tool_requests_for_config(&config);

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].tool_type, "web_search");
    assert_eq!(requests[0].provider_tool_type, "web_search_20250305");
    assert_eq!(
        requests[0].provider_tool_name.as_deref(),
        Some("web_search")
    );
}

#[test]
fn hosted_tool_requests_omit_openai_completions_web_search() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::OpenAiCompletions;

    let requests = hosted_tool_requests_for_config(&config);

    assert!(requests.is_empty());
}

#[test]
fn rig_agent_additional_params_include_openai_service_label_raw() {
    let mut config = test_config(false);
    config.service_label = Some("fast".into());

    let params = rig_agent_additional_params(&config).expect("reasoning params");

    assert_eq!(params["service_tier"], "fast");
}

#[test]
fn rig_agent_additional_params_include_supported_openai_service_label() {
    let mut config = test_config(false);
    config.service_label = Some("flex".into());

    let params = rig_agent_additional_params(&config).expect("service params");

    assert_eq!(params["service_tier"], "flex");
}

#[test]
fn rig_agent_additional_params_omit_response_only_params_for_openai_completions() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::OpenAiCompletions;
    config.reasoning_effort = Some("high".into());
    config.service_label = Some("flex".into());

    assert!(rig_agent_additional_params(&config).is_none());
}

#[test]
fn rig_agent_additional_params_include_anthropic_thinking_and_web_search() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.service_label = Some("fast".into());

    let params = rig_agent_additional_params(&config).expect("anthropic params");

    assert_eq!(params["thinking"]["type"], "enabled");
    assert_eq!(params["thinking"]["budget_tokens"], 8191);
    assert_eq!(params["tools"][0]["type"], "web_search_20250305");
    assert_eq!(params["tools"][0]["name"], "web_search");
    assert!(params.get("service_tier").is_none());
}

#[test]
fn rig_agent_additional_params_clamps_anthropic_thinking_below_max_tokens() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.reasoning_effort = Some("xhigh".into());
    config.max_tokens = 8192;

    let params = rig_agent_additional_params(&config).expect("anthropic params");

    assert_eq!(params["thinking"]["budget_tokens"], 8191);
}

#[test]
fn rig_agent_additional_params_omits_anthropic_thinking_when_budget_too_small() {
    let mut config = test_config(true);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.reasoning_effort = Some("low".into());
    config.max_tokens = 1024;

    let params = rig_agent_additional_params(&config).expect("web search params");

    assert!(params.get("thinking").is_none());
    assert_eq!(params["tools"][0]["type"], "web_search_20250305");
    assert_eq!(params["tools"][0]["name"], "web_search");
}

#[test]
fn rig_agent_temperature_param_omits_anthropic_temperature_when_thinking_enabled() {
    let mut config = test_config(false);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.reasoning_effort = Some("high".into());

    assert_eq!(rig_agent_temperature_param(&config), None);
}

#[test]
fn rig_agent_temperature_param_keeps_anthropic_temperature_without_thinking() {
    let mut config = test_config(false);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.reasoning_effort = Some("low".into());
    config.max_tokens = 1024;

    assert_eq!(rig_agent_temperature_param(&config), Some(0.7));
}

#[test]
fn rig_agent_temperature_param_keeps_anthropic_temperature_for_unknown_effort() {
    let mut config = test_config(false);
    config.provider_kind = AgentProviderKind::Anthropic;
    config.reasoning_effort = Some("experimental".into());

    assert!(rig_agent_additional_params(&config).is_none());
    assert_eq!(rig_agent_temperature_param(&config), Some(0.7));
}

#[tokio::test]
async fn rig_agent_config_loads_native_web_search_from_env() {
    let _env = EnvGuard::set_many(&[
        ("BUDN_AGENT_OPENAI_API_KEY", "sk-test"),
        ("BUDN_AGENT_NATIVE_WEB_SEARCH", "true"),
    ]);

    let config = load_rig_agent_config()
        .await
        .expect("config load should succeed")
        .expect("env config should be present");

    assert!(config.native_web_search);
}

#[tokio::test]
async fn rig_agent_config_defaults_native_web_search_to_enabled() {
    let _env = EnvGuard::set_many(&[("BUDN_AGENT_OPENAI_API_KEY", "sk-test")]);

    let config = load_rig_agent_config()
        .await
        .expect("config load should succeed")
        .expect("env config should be present");

    assert!(config.native_web_search);
}

#[tokio::test]
async fn rig_agent_config_loads_native_web_search_from_config_file() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agent.toml");
    tokio::fs::write(
        &config_path,
        r#"
api_key = "sk-test"
model = "gpt-5.2"
native_web_search = true
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[(
        "BUDN_AGENT_CONFIG",
        config_path.to_str().expect("utf8 config path"),
    )]);

    let config = load_rig_agent_config()
        .await
        .expect("config load should succeed")
        .expect("file config should be present");

    assert!(config.native_web_search);
}

#[tokio::test]
async fn rig_agent_config_loads_agents_toml_registry() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[defaults]
timeout_secs = 45
max_tokens = 4096
temperature = 0.2

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"
label = "GPT 5.2"
reasoning_effort = "high"
service_label = "fast"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_OPENAI_API_KEY", "sk-openai"),
        ("BUDN_AGENT_ANTHROPIC_API_KEY", "sk-anthropic"),
    ]);

    let registry = load_agent_provider_registry()
        .await
        .expect("config load should succeed")
        .expect("file config should be present");
    let openai = registry.provider("openai").expect("openai provider");
    let anthropic = registry.provider("anthropic").expect("anthropic provider");
    let active = registry.active_model().expect("active model");

    assert_eq!(registry.active_provider_id, "openai");
    assert_eq!(registry.active_model_id, "gpt-5.2");
    assert!(registry.defaults.native_web_search);
    assert!(registry.defaults.discover_models);
    assert_eq!(registry.defaults.timeout_secs, 45);
    assert_eq!(openai.kind, AgentProviderKind::OpenAiResponses);
    assert_eq!(openai.api_key.as_deref(), Some("sk-openai"));
    assert!(openai.discover_models);
    assert_eq!(anthropic.kind, AgentProviderKind::Anthropic);
    assert_eq!(anthropic.anthropic_version.as_deref(), Some("2023-06-01"));
    assert_eq!(active.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(active.service_label.as_deref(), Some("fast"));
    assert!(active.native_web_search);
    assert!(active.web_search_supported);
}

#[tokio::test]
async fn rig_agent_config_loads_direct_provider_api_key() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "glhf"
active_model = "gemini-3.1-pro-preview"

[[providers]]
id = "glhf"
kind = "openai_completions"
api_key = "ah-direct-test"
base_url = "https://l.glhf.do#"
discover_models = false

[[providers.models]]
id = "gemini-3.1-pro-preview"
label = "Gemini 3.1 Pro Preview"
service_label = "high"
"#,
    )
    .await
    .expect("direct api key config should load");

    let provider = registry.provider("glhf").expect("glhf provider");
    assert_eq!(registry.active_provider_id, "glhf");
    assert_eq!(registry.active_model_id, "gemini-3.1-pro-preview");
    assert_eq!(provider.kind, AgentProviderKind::OpenAiCompletions);
    assert_eq!(provider.api_key.as_deref(), Some("ah-direct-test"));
    assert!(provider.api_key_env.is_empty());
    assert_eq!(provider.base_url.as_deref(), Some("https://l.glhf.do"));
    assert!(!provider.discover_models);
}

#[tokio::test]
async fn rig_agent_config_loads_anthropic_active_provider() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "anthropic"
active_model = "claude-sonnet"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"

[[providers.models]]
id = "claude-sonnet"
reasoning_effort = "high"
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_ANTHROPIC_API_KEY", "sk-anthropic"),
    ]);

    let config = load_rig_agent_config()
        .await
        .expect("config should load")
        .expect("config should be present");

    assert_eq!(config.provider_kind, AgentProviderKind::Anthropic);
    assert_eq!(config.anthropic_version.as_deref(), Some("2023-06-01"));
    assert_eq!(config.model, "claude-sonnet");
    assert_eq!(config.reasoning_effort.as_deref(), Some("high"));
}

#[tokio::test]
async fn rig_agent_config_selection_none_does_not_fall_back_to_model_defaults() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"
reasoning_effort = "high"
service_label = "default"
"#,
    )
    .await
    .expect("registry should load");

    let selected = rig_config_from_registry_selection(
        registry,
        &RigAgentConfigSelection {
            provider_id: Some("openai".into()),
            model_id: Some("gpt-5.2".into()),
            reasoning_effort: None,
            service_label: None,
        },
    )
    .expect("selected model config should build");

    assert_eq!(selected.reasoning_effort, None);
    assert_eq!(selected.service_label, None);
}

#[tokio::test]
async fn rig_agent_config_with_discovery_uses_provider_registry_path() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "anthropic"
active_model = "claude-sonnet"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"
discover_models = false

[[providers.models]]
id = "claude-sonnet"
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_ANTHROPIC_API_KEY", "sk-anthropic"),
    ]);

    let config = load_rig_agent_config_with_discovery()
        .await
        .expect("config should load")
        .expect("config should be present");

    assert_eq!(config.provider_kind, AgentProviderKind::Anthropic);
    assert_eq!(config.model, "claude-sonnet");
}

#[tokio::test]
async fn rig_agent_config_supports_three_provider_types() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai_responses"
active_model = "gpt-5.2"

[[providers]]
id = "openai_responses"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"

[[providers]]
id = "openai_completions"
kind = "openai_completions"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-4o"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"

[[providers.models]]
id = "claude-sonnet"
"#,
    )
    .await
    .expect("config load should succeed");

    assert_eq!(
        registry.provider("openai_responses").unwrap().kind,
        AgentProviderKind::OpenAiResponses
    );
    assert_eq!(
        registry.provider("openai_completions").unwrap().kind,
        AgentProviderKind::OpenAiCompletions
    );
    assert_eq!(
        registry.provider("anthropic").unwrap().kind,
        AgentProviderKind::Anthropic
    );
}

#[tokio::test]
async fn rig_agent_config_resolves_provider_base_urls() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai_default"
active_model = "gpt-5.2"

[[providers]]
id = "openai_default"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"

[[providers]]
id = "openai_no_slash"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
base_url = "https://openai-compatible.example/api"

[[providers.models]]
id = "gpt-5.2"

[[providers]]
id = "openai_slash"
kind = "openai_completions"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
base_url = "https://openai-compatible.example/api/"

[[providers.models]]
id = "gpt-4o"

[[providers]]
id = "openai_raw"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
base_url = "https://openai-compatible.example/raw#"

[[providers.models]]
id = "gpt-5.2"

[[providers]]
id = "anthropic_default"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"

[[providers.models]]
id = "claude-sonnet"

[[providers]]
id = "anthropic_custom"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"
base_url = "https://anthropic-proxy.example/api"

[[providers.models]]
id = "claude-sonnet"

[[providers]]
id = "anthropic_raw"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"
base_url = "https://anthropic-proxy.example/raw#"

[[providers.models]]
id = "claude-sonnet"
"#,
    )
    .await
    .expect("config load should succeed");

    assert_eq!(registry.provider("openai_default").unwrap().base_url, None);
    assert_eq!(
        registry
            .provider("openai_no_slash")
            .unwrap()
            .base_url
            .as_deref(),
        Some("https://openai-compatible.example/api/v1")
    );
    assert_eq!(
        registry
            .provider("openai_slash")
            .unwrap()
            .base_url
            .as_deref(),
        Some("https://openai-compatible.example/api/")
    );
    assert_eq!(
        registry.provider("openai_raw").unwrap().base_url.as_deref(),
        Some("https://openai-compatible.example/raw")
    );
    assert_eq!(
        registry.provider("anthropic_default").unwrap().base_url,
        None
    );
    assert_eq!(
        registry
            .provider("anthropic_custom")
            .unwrap()
            .base_url
            .as_deref(),
        Some("https://anthropic-proxy.example/api")
    );
    assert_eq!(
        registry
            .provider("anthropic_raw")
            .unwrap()
            .base_url
            .as_deref(),
        Some("https://anthropic-proxy.example/raw")
    );
}

#[tokio::test]
async fn rig_agent_config_selection_uses_resolved_base_url() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
base_url = "https://openai-compatible.example/api"

[[providers.models]]
id = "gpt-5.2"
"#,
    )
    .await
    .expect("config load should succeed");

    let config = rig_config_from_registry_selection(
        registry,
        &RigAgentConfigSelection {
            provider_id: Some("openai".into()),
            model_id: Some("gpt-5.2".into()),
            reasoning_effort: None,
            service_label: None,
        },
    )
    .expect("selected model config should build");

    assert_eq!(
        config.base_url.as_deref(),
        Some("https://openai-compatible.example/api/v1")
    );
}

#[tokio::test]
async fn rig_agent_config_does_not_apply_native_web_search_for_openai_completions() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "gpt-4o"

[[providers]]
id = "openai"
kind = "openai_completions"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-4o"
native_web_search = true
web_search_supported = true
"#,
    )
    .await
    .expect("config should load");
    let config = rig_config_from_registry_selection(
        registry,
        &RigAgentConfigSelection {
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        },
    )
    .expect("active config should build");

    assert!(!config.native_web_search);
}

#[tokio::test]
async fn rig_agent_config_rejects_invalid_agents_toml() {
    let cases = [
        (
            "duplicate provider",
            r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
"#,
            "provider id",
        ),
        (
            "duplicate model",
            r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"

[[providers.models]]
id = "gpt-5.2"
"#,
            "model id",
        ),
        (
            "missing active model",
            r#"
active_provider = "openai"
active_model = "missing"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
"#,
            "active model",
        ),
        (
            "anthropic version on openai",
            r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
anthropic_version = "2023-06-01"

[[providers.models]]
id = "gpt-5.2"
"#,
            "anthropic_version",
        ),
        (
            "empty anthropic version",
            r#"
active_provider = "anthropic"
active_model = "claude-sonnet"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"
anthropic_version = ""

[[providers.models]]
id = "claude-sonnet"
"#,
            "anthropic_version",
        ),
        (
            "unsupported web search reason",
            r#"
active_provider = "openai"
active_model = "local-model"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "local-model"
web_search_supported = false
web_search_unsupported_reason = ""
"#,
            "web_search_unsupported_reason",
        ),
    ];

    for (name, toml, expected) in cases {
        let error = load_registry_from_toml(toml).await.expect_err(name).message;
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[tokio::test]
async fn rig_agent_config_reports_missing_api_key_env() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[(
        "BUDN_AGENT_CONFIG",
        config_path.to_str().expect("utf8 config path"),
    )]);

    let error = load_agent_provider_registry()
        .await
        .expect_err("missing api key env")
        .message;

    assert!(error.contains("BUDN_AGENT_OPENAI_API_KEY"));
}

#[tokio::test]
async fn rig_agent_config_allows_inactive_provider_without_api_key_env() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"

[[providers]]
id = "anthropic"
kind = "anthropic"
api_key_env = "BUDN_AGENT_ANTHROPIC_API_KEY"

[[providers.models]]
id = "claude-sonnet"
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_OPENAI_API_KEY", "sk-openai"),
    ]);

    let registry = load_agent_provider_registry()
        .await
        .expect("inactive missing key should not block registry")
        .expect("file config should be present");

    assert_eq!(
        registry
            .provider("anthropic")
            .expect("anthropic provider")
            .api_key,
        None
    );
}

#[tokio::test]
async fn rig_agent_config_preserves_web_search_intent_when_model_does_not_support_it() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "local-model"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "local-model"
native_web_search = true
web_search_supported = false
"#,
    )
    .await
    .expect("config load should succeed");
    let active = registry.active_model().expect("active model");

    assert!(active.native_web_search);
    assert!(!active.web_search_supported);
    assert!(active.web_search_unsupported_reason.is_some());
}

#[tokio::test]
async fn rig_agent_config_disables_web_search_tool_when_model_does_not_support_it() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(
        &config_path,
        r#"
active_provider = "openai"
active_model = "local-model"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "local-model"
native_web_search = true
web_search_supported = false
"#,
    )
    .await
    .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_OPENAI_API_KEY", "sk-openai"),
    ]);

    let config = load_rig_agent_config()
        .await
        .expect("config load should succeed")
        .expect("file config should be present");

    assert!(!config.native_web_search);
    assert!(rig_agent_additional_params(&config).is_none());
}

#[tokio::test]
async fn rig_agent_config_supports_discovery_defaults_and_provider_override() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "manual-model"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
discover_models = false

[[providers.models]]
id = "manual-model"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");
    let active = registry.active_model().expect("active model");

    assert!(registry.defaults.discover_models);
    assert!(!provider.discover_models);
    assert!(active.web_search_supported);
}

#[tokio::test]
async fn rig_agent_config_merges_discovered_models_with_manual_overrides() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"
label = "Pinned GPT"
reasoning_effort = "xhigh"
service_label = "fast"

[[providers.models]]
id = "manual-only"
label = "Manual Only"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");
    let merged = merge_provider_models(
        provider,
        vec![DiscoveredProviderModel {
            id: "gpt-5.2".into(),
            label: "Discovered GPT".into(),
            web_search_supported: true,
        }],
    );

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].id, "gpt-5.2");
    assert_eq!(merged[0].source, AgentModelSource::DiscoveredWithOverride);
    assert_eq!(merged[0].label.as_deref(), Some("Pinned GPT"));
    assert_eq!(merged[0].reasoning_effort.as_deref(), Some("xhigh"));
    assert_eq!(merged[0].service_label.as_deref(), Some("fast"));
    assert!(merged[0].web_search_supported);
    assert_eq!(merged[1].id, "manual-only");
    assert_eq!(merged[1].label.as_deref(), Some("Manual Only"));
    assert_eq!(merged[1].source, AgentModelSource::Manual);
}

#[tokio::test]
async fn rig_agent_config_does_not_apply_discovered_web_search_for_openai_completions() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "manual-model"

[[providers]]
id = "openai"
kind = "openai_completions"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "manual-model"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");
    let merged = merge_provider_models(
        provider,
        vec![DiscoveredProviderModel {
            id: "discovered-model".into(),
            label: "Discovered Model".into(),
            web_search_supported: true,
        }],
    );
    let discovered = merged
        .iter()
        .find(|model| model.id == "discovered-model")
        .expect("discovered model");

    assert!(!discovered.web_search_supported);
    assert!(discovered.web_search_unsupported_reason.is_some());
}

#[tokio::test]
async fn rig_agent_config_manual_override_preserves_unspecified_discovered_fields() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "gpt-5.2"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "gpt-5.2"
reasoning_effort = "xhigh"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");
    let merged = merge_provider_models(
        provider,
        vec![DiscoveredProviderModel {
            id: "gpt-5.2".into(),
            label: "Discovered GPT".into(),
            web_search_supported: false,
        }],
    );

    assert_eq!(merged[0].label.as_deref(), Some("Discovered GPT"));
    assert_eq!(merged[0].reasoning_effort.as_deref(), Some("xhigh"));
    assert!(!merged[0].web_search_supported);
    assert!(merged[0].web_search_unsupported_reason.is_some());
}

#[tokio::test]
async fn rig_agent_config_discovered_models_inherit_provider_defaults() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "manual-model"

[defaults]
max_tokens = 2048
temperature = 0.25
native_web_search = false

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "manual-model"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");
    let merged = merge_provider_models(
        provider,
        vec![DiscoveredProviderModel {
            id: "discovered-model".into(),
            label: "Discovered Model".into(),
            web_search_supported: true,
        }],
    );
    let discovered = merged
        .iter()
        .find(|model| model.id == "discovered-model")
        .expect("discovered model");

    assert_eq!(discovered.max_tokens, 2048);
    assert_eq!(discovered.temperature, 0.25);
    assert!(!discovered.native_web_search);
    assert_eq!(discovered.source, AgentModelSource::Discovered);
}

#[tokio::test]
async fn agent_model_discovery_failure_keeps_manual_models() {
    let registry = load_registry_from_toml(
        r#"
active_provider = "openai"
active_model = "manual-model"

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"

[[providers.models]]
id = "manual-model"
"#,
    )
    .await
    .expect("config load should succeed");
    let provider = registry.provider("openai").expect("provider");

    let provider = apply_provider_model_discovery(provider, Err("401 unauthorized".into()));

    assert_eq!(provider.models.len(), 1);
    assert_eq!(provider.models[0].id, "manual-model");
    assert_eq!(
        provider.model_discovery_status,
        ModelDiscoveryStatus::Failed("401 unauthorized".into())
    );
}

#[test]
fn agent_model_discovery_accepts_openai_compatible_models_without_created() {
    let models = openai_compatible_models_to_discovered(
        br#"{"object":"list","data":[{"id":"deepseek-v4-flash","object":"model","owned_by":"deepseek"},{"id":"deepseek-v4-pro","object":"model","owned_by":"deepseek"}]}"#,
        "/models",
    )
    .expect("compatible model list should parse");

    assert!(models.iter().any(|model| model.id == "deepseek-v4-flash"));
    assert!(models.iter().any(|model| model.id == "deepseek-v4-pro"));
    assert!(models.iter().all(|model| model.web_search_supported));
}

async fn load_registry_from_toml(
    content: &str,
) -> Result<app_server_core::llm::AgentProviderRegistry, app_server_core::llm::RigAgentConfigError>
{
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agents.toml");
    tokio::fs::write(&config_path, content)
        .await
        .expect("config file should be writable");
    let _env = EnvGuard::set_many(&[
        (
            "BUDN_AGENT_CONFIG",
            config_path.to_str().expect("utf8 config path"),
        ),
        ("BUDN_AGENT_OPENAI_API_KEY", "sk-openai"),
        ("BUDN_AGENT_ANTHROPIC_API_KEY", "sk-anthropic"),
    ]);
    load_agent_provider_registry()
        .await
        .map(|config| config.expect("file config should be present"))
}

fn test_config(native_web_search: bool) -> RigAgentConfig {
    RigAgentConfig {
        provider_id: "openai".into(),
        provider_kind: AgentProviderKind::OpenAiResponses,
        api_key: "sk-test".into(),
        model: "gpt-5.2".into(),
        timeout_secs: 60,
        max_tokens: 8192,
        temperature: 0.7,
        reasoning_effort: Some("high".into()),
        service_label: Some("default".into()),
        native_web_search,
        anthropic_version: None,
        base_url: None,
    }
}

fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn set_many(values: &[(&'static str, &str)]) -> Self {
        let keys = [
            "BUDN_AGENT_CONFIG",
            "BUDN_AGENT_OPENAI_API_KEY",
            "BUDN_AGENT_ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "BUDN_AGENT_MODEL",
            "BUDN_AGENT_REASONING_EFFORT",
            "BUDN_AGENT_MAX_TOKENS",
            "BUDN_AGENT_TIMEOUT_SECS",
            "BUDN_AGENT_TEMPERATURE",
            "BUDN_AGENT_NATIVE_WEB_SEARCH",
        ];
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = keys
            .iter()
            .map(|key| {
                let previous = std::env::var_os(key);
                unsafe {
                    std::env::remove_var(key);
                }
                (*key, previous)
            })
            .collect();
        for (key, value) in values {
            unsafe {
                std::env::set_var(key, value);
            }
        }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, previous) in &self.previous {
            unsafe {
                if let Some(value) = previous {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

fn chat_msg(role: &str, content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: "m1".into(),
        ts_ms: 1,
        role: match role {
            "assistant" => ChatRole::Assistant,
            _ => ChatRole::User,
        },
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
        search_sources: Vec::new(),
        run_id: None,
        agent_id: None,
        turn_id: None,
    }
}

fn empty_msg(role: &str) -> ChatMessageRecord {
    chat_msg(role, "")
}

fn tool_msg(content: &str) -> ChatMessageRecord {
    ChatMessageRecord {
        message_id: "t1".into(),
        ts_ms: 1,
        role: ChatRole::Tool,
        content: content.into(),
        related_files: Vec::new(),
        tool_call_id: None,
        tool_calls: Vec::new(),
        tool_result: None,
        mesh_result: None,
        search_sources: Vec::new(),
        run_id: None,
        agent_id: None,
        turn_id: None,
    }
}

fn test_selection() -> SelectionRef {
    SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[lid:f_0]".into(),
        owner_ref_text: Some("@part[lid]".into()),
        owner_object_kind: Some(CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[lid.lid_alignment_surface]".into()),
        build_id: Some("sha256:test".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    }
}

#[tokio::test]
async fn web_search_provider_parses_searxng_config() {
    let registry = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"
        active_web_search = "searxng-local"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "searxng-local"
        endpoint = "http://localhost:8080/search"
        method = "GET"
        query_key = "q"
        [web_search_providers.params]
        format = "json"
        engines = "google,bing"
        [web_search_providers.result_map]
        results = "results"
        title = "title"
        url = "url"
        snippet = "content"
        date = "publishedDate"
        source = "engine"
        "#,
    )
    .await
    .expect("should parse");

    assert_eq!(registry.active_web_search_id.as_deref(), Some("searxng-local"));
    assert_eq!(registry.web_search_providers.len(), 1);
    let p = &registry.web_search_providers[0];
    assert_eq!(p.id, "searxng-local");
    assert_eq!(p.endpoint, "http://localhost:8080/search");
    assert_eq!(p.method, WebSearchHttpMethod::Get);
    assert_eq!(p.query_key, "q");
    assert!(matches!(p.auth, WebSearchAuth::None));
    assert!(matches!(&p.params, WebSearchParams::QueryPairs(pairs) if pairs.len() == 2));
    assert_eq!(p.result_map.results_path, "results");
    assert_eq!(p.result_map.snippet, "content");
    assert_eq!(p.result_map.date.as_deref(), Some("publishedDate"));
    assert_eq!(p.result_map.source.as_deref(), Some("engine"));
    assert!(registry.active_web_search_provider().is_some());
}

#[tokio::test]
async fn web_search_provider_parses_post_with_header_auth() {
    let registry = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"
        active_web_search = "tavily"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "tavily"
        endpoint = "https://api.tavily.com/search"
        method = "POST"
        query_key = "query"
        api_key = "tvly-secret"
        auth_header = "Authorization"
        auth_prefix = "Bearer "
        top_k_param = "max_results"
        max_result_chars = 8000
        [web_search_providers.params]
        max_results = 5
        [web_search_providers.result_map]
        results = "results"
        title = "title"
        url = "url"
        snippet = "content"
        "#,
    )
    .await
    .expect("should parse");

    let p = &registry.web_search_providers[0];
    assert_eq!(p.method, WebSearchHttpMethod::Post);
    assert!(matches!(&p.auth, WebSearchAuth::Header { header, prefix, .. }
        if header == "Authorization" && prefix.as_deref() == Some("Bearer ")));
    assert_eq!(p.top_k_param.as_deref(), Some("max_results"));
    assert_eq!(p.max_result_chars, Some(8000));
    assert!(matches!(&p.params, WebSearchParams::JsonObject(m) if m.contains_key("max_results")));
    assert!(p.result_map.date.is_none());
    assert!(p.result_map.source.is_none());
}

#[tokio::test]
async fn web_search_provider_parses_query_param_auth() {
    let registry = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"
        active_web_search = "serpapi"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "serpapi"
        endpoint = "https://serpapi.com/search"
        query_key = "q"
        api_key = "serp-secret"
        auth_query_param = "api_key"
        [web_search_providers.result_map]
        results = "organic_results"
        title = "title"
        url = "link"
        snippet = "snippet"
        "#,
    )
    .await
    .expect("should parse");

    let p = &registry.web_search_providers[0];
    assert_eq!(p.method, WebSearchHttpMethod::Get);
    assert!(matches!(&p.auth, WebSearchAuth::QueryParam { param, .. } if param == "api_key"));
}

#[tokio::test]
async fn web_search_provider_rejects_mutual_exclusive_auth() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "bad"
        endpoint = "https://example.com"
        query_key = "q"
        api_key = "key"
        auth_header = "Authorization"
        auth_query_param = "api_key"
        [web_search_providers.result_map]
        results = "results"
        title = "title"
        url = "url"
        snippet = "snippet"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("mutually exclusive"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_auth_without_api_key() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "bad"
        endpoint = "https://example.com"
        query_key = "q"
        auth_header = "X-Token"
        [web_search_providers.result_map]
        results = "results"
        title = "title"
        url = "url"
        snippet = "snippet"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("auth requires api_key"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_duplicate_ids() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "dup"
        endpoint = "https://a.com"
        query_key = "q"
        [web_search_providers.result_map]
        results = "r"
        title = "t"
        url = "u"
        snippet = "s"

        [[web_search_providers]]
        id = "dup"
        endpoint = "https://b.com"
        query_key = "q"
        [web_search_providers.result_map]
        results = "r"
        title = "t"
        url = "u"
        snippet = "s"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("duplicate"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_invalid_method() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "bad"
        endpoint = "https://example.com"
        method = "PATCH"
        query_key = "q"
        [web_search_providers.result_map]
        results = "r"
        title = "t"
        url = "u"
        snippet = "s"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("GET or POST"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_invalid_active_web_search() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"
        active_web_search = "nonexistent"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("nonexistent"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_defaults_to_empty_when_not_configured() {
    let registry = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"
        "#,
    )
    .await
    .expect("should parse");

    assert!(registry.active_web_search_id.is_none());
    assert!(registry.web_search_providers.is_empty());
    assert!(registry.active_web_search_provider().is_none());
}

#[tokio::test]
async fn web_search_provider_rejects_empty_result_map_field() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "bad"
        endpoint = "https://example.com"
        query_key = "q"
        [web_search_providers.result_map]
        results = "results"
        title = ""
        url = "url"
        snippet = "snippet"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("result_map.title"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_empty_id() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = ""
        endpoint = "https://example.com"
        query_key = "q"
        [web_search_providers.result_map]
        results = "r"
        title = "t"
        url = "u"
        snippet = "s"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("web_search_providers.id"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_rejects_empty_endpoint() {
    let err = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "bad"
        endpoint = ""
        query_key = "q"
        [web_search_providers.result_map]
        results = "r"
        title = "t"
        url = "u"
        snippet = "s"
        "#,
    )
    .await
    .unwrap_err();
    assert!(err.message.contains("web_search_providers.endpoint"), "{}", err.message);
}

#[tokio::test]
async fn web_search_provider_debug_hides_api_key() {
    let registry = load_registry_from_toml(
        r#"
        active_provider = "openai"
        active_model = "gpt-5.2"
        active_web_search = "brave"

        [[providers]]
        id = "openai"
        kind = "openai_responses"
        api_key = "sk-test"
        [[providers.models]]
        id = "gpt-5.2"

        [[web_search_providers]]
        id = "brave"
        endpoint = "https://api.search.brave.com/res/v1/web/search"
        query_key = "q"
        api_key = "BSA-super-secret"
        auth_header = "X-Subscription-Token"
        [web_search_providers.result_map]
        results = "web.results"
        title = "title"
        url = "url"
        snippet = "description"
        "#,
    )
    .await
    .expect("should parse");

    let debug = format!("{:?}", registry.web_search_providers[0]);
    assert!(!debug.contains("BSA-super-secret"));
    assert!(debug.contains("***"));
}

#[tokio::test]
#[ignore = "requires live LLM provider; run with: cargo test -p app-server-core --test llm_tests live_web_search -- --ignored"]
async fn live_web_search_agent_turn() {
    use app_server_core::{
        AgentToolCall, AgentToolObserver, AgentToolRunContext, HostedToolRequest,
        RigAgentCallbacks, ToolExecutor, run_rig_agent_turn,
    };
    use std::sync::{Arc, Mutex};

    struct PrintObserver;
    impl AgentToolObserver for PrintObserver {
        fn tool_start(&self, call: &AgentToolCall) {
            eprintln!("[tool_start] {}", call.function_name);
        }
        fn tool_result(&self, call: &AgentToolCall, result: &str) {
            let n = result.len().min(120);
            eprintln!("[tool_result] {} => {}...", call.function_name, &result[..n]);
        }
    }

    struct StubExecutor;
    #[async_trait::async_trait]
    impl ToolExecutor for StubExecutor {
        async fn execute(&self, call: &AgentToolCall, _ctx: &AgentToolRunContext) -> String {
            format!(r#"{{"status":"error","tool":"{}","message":"not available in test harness"}}"#, call.function_name)
        }
    }

    let hosted: Arc<Mutex<Vec<String>>> = Arc::default();
    let hosted_cb = Arc::clone(&hosted);

    let input = AgentTurnInput {
        mode: AgentMode::Agent,
        prompt: "What is the current weather in Tokyo? Use web search.".into(),
        history: vec![],
        selections: vec![],
        active_selection_index: None,
        plan_ref: None,
        context_refs: vec![],
        native_web_search_enabled: true,
        function_web_search_available: false,
        execution_scope: None,
    };

    let callbacks = RigAgentCallbacks {
        on_token: Arc::new(|_| true),
        on_reasoning: Arc::new(|_| true),
        on_hosted_tool_requested: Arc::new(move |req: &HostedToolRequest| {
            eprintln!("[hosted_tool_requested] type={}", req.tool_type);
            hosted_cb.lock().unwrap().push(req.tool_type.clone());
            true
        }),
        cancelled: Arc::new(|| false),
    };

    let executor: Arc<dyn ToolExecutor> = Arc::new(StubExecutor);
    let ctx = AgentToolRunContext::new(std::path::PathBuf::from("/tmp/test-ws"), AgentMode::Agent);

    let result = run_rig_agent_turn(input, executor, ctx, &PrintObserver, callbacks).await;

    match result {
        Ok(r) => {
            let hosted = hosted.lock().unwrap();
            eprintln!("[hosted tools] {:?}", *hosted);
            assert!(hosted.iter().any(|t| t == "web_search"), "hosted web_search should be requested");
            let n = r.text.len().min(300);
            eprintln!("[agent response] {}", &r.text[..n]);
            assert!(!r.text.is_empty(), "response should not be empty");
        }
        Err(e) => panic!("agent turn failed: {}", e.message),
    }
}
