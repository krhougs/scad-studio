use app_server_core::llm::{RigAgentConfig, load_rig_agent_config};
use app_server_core::{
    AgentExecutionScope, AgentTurnInput, build_rig_prompt_and_history, build_turn_context,
    cadquery_agent_system_prompt, extract_cadquery_code, rig_agent_additional_params,
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
        execution_scope: None,
    };
    let context = build_turn_context(&input);
    assert!(context.contains("context refs"));
    assert!(context.contains("@face[top_lid:f_0]"));
    assert!(context.contains("@part[bottom_case]"));
    assert!(context.contains("Native web search: enabled"));
}

#[test]
fn rig_agent_config_debug_hides_api_key() {
    let config = RigAgentConfig {
        api_key: "sk-secret-key-12345".into(),
        model: "gpt-5.2".into(),
        timeout_secs: 60,
        max_tokens: 8192,
        temperature: 0.7,
        reasoning_effort: Some("high".into()),
        native_web_search: true,
    };
    let debug = format!("{:?}", config);
    assert!(!debug.contains("sk-secret-key-12345"));
    assert!(debug.contains("***"));
    assert!(debug.contains("gpt-5.2"));
    assert!(debug.contains("native_web_search"));
}

#[test]
fn rig_agent_additional_params_omit_web_search_when_disabled() {
    let config = test_config(false);
    let params = rig_agent_additional_params(&config).expect("reasoning params");
    assert_eq!(params["reasoning"]["effort"], "high");
    assert!(params.get("tools").is_none());
}

#[test]
fn rig_agent_additional_params_include_hosted_web_search_when_enabled() {
    let config = test_config(true);
    let params = rig_agent_additional_params(&config).expect("web search params");
    assert_eq!(params["reasoning"]["effort"], "high");
    assert_eq!(params["tools"][0]["type"], "web_search");
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
async fn rig_agent_config_defaults_native_web_search_to_disabled() {
    let _env = EnvGuard::set_many(&[("BUDN_AGENT_OPENAI_API_KEY", "sk-test")]);

    let config = load_rig_agent_config()
        .await
        .expect("config load should succeed")
        .expect("env config should be present");

    assert!(!config.native_web_search);
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
async fn rig_agent_config_file_defaults_native_web_search_to_disabled() {
    let temp_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = temp_dir.path().join("agent.toml");
    tokio::fs::write(
        &config_path,
        r#"
api_key = "sk-test"
model = "gpt-5.2"
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

    assert!(!config.native_web_search);
}

fn test_config(native_web_search: bool) -> RigAgentConfig {
    RigAgentConfig {
        api_key: "sk-test".into(),
        model: "gpt-5.2".into(),
        timeout_secs: 60,
        max_tokens: 8192,
        temperature: 0.7,
        reasoning_effort: Some("high".into()),
        native_web_search,
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
