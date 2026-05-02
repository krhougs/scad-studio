use std::{fs, path::Path};

use app_server_protocol::{
    AgentCancelRequest, AgentId, AgentModelDiscoveryState, AgentModelDiscoveryStatus,
    AgentModelRegistryModel, AgentModelRegistryProvider, AgentModelRegistryResponse,
    AgentModelSource, AgentProviderCapabilities, AgentProviderType, BoundAgentModel,
    CURRENT_PROTOCOL_VERSION, PreviewRequestKind, ProtocolVersionRange, ServerCapabilities,
};

#[test]
fn protocol_wire_payload_does_not_expose_pathbuf_or_json_config_payload() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/protocol.rs"))
        .expect("protocol source should be readable");

    assert!(
        !source.contains("use std::path::PathBuf"),
        "protocol wire payload must not import PathBuf"
    );
    assert!(
        !source.contains("pub json: String"),
        "config wire payload must not carry structured config as json string"
    );
    assert!(
        !source.contains("output_path: PathBuf"),
        "export output target must be workspace portable path"
    );
}

#[test]
fn protocol_v11_capabilities_expose_chat_identity_fields() {
    assert_eq!(CURRENT_PROTOCOL_VERSION, 11);
    let capabilities = ServerCapabilities {
        protocol_version: ProtocolVersionRange::new(11, 11),
        reconnect_window_ms: 30_000,
        supports_watch: true,
        supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        supports_session_reclaim: true,
        cadquery: true,
        agent: true,
        selection_sync: true,
        llm_configured: true,
        agent_provider: Some(AgentProviderCapabilities {
            provider: "openai_responses".into(),
            model: Some("gpt-5.2".into()),
            native_web_search_enabled: true,
            search_sources_supported: false,
        }),
        agent_model_registry: Some(sample_agent_model_registry()),
    };

    let bytes = borsh::to_vec(&capabilities).expect("server capabilities encode");
    let decoded: ServerCapabilities =
        borsh::from_slice(&bytes).expect("server capabilities decode");
    assert!(decoded.agent_provider.is_some());
    let registry = decoded
        .agent_model_registry
        .expect("agent model registry survives wire roundtrip");
    assert_eq!(registry.active_provider_id, "openai");
    assert!(registry.active_reasoning_effort_applied);
    assert!(registry.active_service_label_applied);
    let model = &registry.providers[0].models[0];
    assert!(model.native_web_search_enabled);
    assert!(!model.native_web_search_applied);
    assert!(!model.web_search_supported);
}

#[test]
fn protocol_agent_operations_target_agent_id_not_run_id() {
    let request = AgentCancelRequest {
        agent_id: AgentId("agent-abc".into()),
    };
    let bytes = borsh::to_vec(&request).expect("agent cancel encode");
    let decoded: AgentCancelRequest = borsh::from_slice(&bytes).expect("agent cancel decode");
    assert_eq!(decoded.agent_id.0, "agent-abc");

    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/protocol.rs"))
        .expect("protocol source should be readable");
    let cancel_start = source
        .find("pub struct AgentCancelRequest")
        .expect("cancel request exists");
    let cancel_end = source[cancel_start..]
        .find("pub struct AgentCancelledResponse")
        .expect("cancel response follows request")
        + cancel_start;
    let cancel_source = &source[cancel_start..cancel_end];
    assert!(
        !cancel_source.contains("run_id"),
        "agent.cancel request must target agent_id, not run_id"
    );
}

#[test]
fn bound_agent_model_does_not_include_base_url() {
    let model = BoundAgentModel {
        provider_id: "openai".into(),
        provider_type: AgentProviderType::OpenAiResponses,
        model_id: "gpt-5.2".into(),
        reasoning_effort: Some("high".into()),
        service_label: Some("flex".into()),
    };
    let bytes = borsh::to_vec(&model).expect("bound model encode");
    let decoded: BoundAgentModel = borsh::from_slice(&bytes).expect("bound model decode");
    assert_eq!(decoded, model);

    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/protocol.rs"))
        .expect("protocol source should be readable");
    let model_start = source
        .find("pub struct BoundAgentModel")
        .expect("bound model exists");
    let model_end = source[model_start..]
        .find("pub enum AgentRuntimeStatus")
        .expect("runtime status follows bound model")
        + model_start;
    let model_source = &source[model_start..model_end];
    assert!(
        !model_source.contains("base_url"),
        "bound model must not persist provider base_url"
    );
}

fn sample_agent_model_registry() -> AgentModelRegistryResponse {
    AgentModelRegistryResponse {
        active_provider_id: "openai".into(),
        active_model_id: "gpt-5.2".into(),
        active_reasoning_effort: Some("high".into()),
        active_reasoning_effort_applied: true,
        active_service_label: Some("flex".into()),
        active_service_label_applied: true,
        reasoning_effort_options: vec!["low".into(), "medium".into(), "high".into()],
        service_label_options: vec!["default".into(), "flex".into()],
        providers: vec![AgentModelRegistryProvider {
            id: "openai".into(),
            kind: "openai_responses".into(),
            label: None,
            discovery: AgentModelDiscoveryState {
                enabled: true,
                status: AgentModelDiscoveryStatus::Succeeded,
                error: None,
            },
            models: vec![AgentModelRegistryModel {
                id: "gpt-5.2".into(),
                label: Some("GPT 5.2".into()),
                source: AgentModelSource::DiscoveredWithOverride,
                reasoning_effort: Some("high".into()),
                service_label: Some("flex".into()),
                native_web_search_enabled: true,
                native_web_search_applied: false,
                web_search_supported: false,
                web_search_unsupported_reason: Some("model does not support web search".into()),
                search_sources_supported: false,
            }],
        }],
    }
}
