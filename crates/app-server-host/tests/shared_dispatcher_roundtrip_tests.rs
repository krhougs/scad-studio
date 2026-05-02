use std::sync::{Arc, Mutex, OnceLock};

use app_server_core::{AGENT_ERROR_FACT_PREFIX, ChatStore, ChatSummaryUpdate};
use app_server_host::HostRequestDispatcher;
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentDoneEvent, AgentErrorEvent, AgentEventId,
    AgentEventPayload, AgentEventRecord, AgentInvokeRequest, AgentMode,
    AgentModelParamsUpdateRequest, AgentModelSelectRequest, AgentPlanConfirmRequest,
    AgentProviderType, AgentRuntimeStatus, AgentSnapshotRequest, AgentStartTurnRequest,
    AgentSubscribeRequest, AgentTurnId, BoundAgentModel, CURRENT_PROTOCOL_VERSION,
    CadQueryExecuteRequest, CadQueryExportFormat, CadQueryObjectKind, CapabilityHandshakeRequest,
    ChatArchiveRequest, ChatCreateInitialTurn, ChatCreateRequest, ChatHistoryRequest,
    ChatListRequest, ChatRole, ChatSendRequest, ChatSessionId, ChatToolResultRecord,
    ClientCapabilities, ClientCommand, ClientPlatform, ClientRequestEnvelope, CommandSuccess,
    ExportFormat, ExportRunRequest, HostLocalPath, PathHandle, PreviewArtifact, PreviewRequest,
    PreviewRequestKind, ProtocolErrorCode, ProtocolVersionRange, RequestId, SelectionKind,
    SelectionRef, SelectionUpdateRequest, ServerPushEnvelope, ServerPushEvent, SessionToken,
    WorkspaceId, WorkspaceListRequest, web_file_read_capability,
};
use futures_util::future::join_all;
use serde_json::Value;

#[tokio::test]
async fn shared_dispatcher_roundtrips_handshake_workspace_file_and_preview() {
    let workspace = temp_workspace("shared-dispatcher");
    let pushes = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
    let push_sink = {
        let pushes = Arc::clone(&pushes);
        Arc::new(move |push: ServerPushEnvelope| {
            pushes.lock().expect("push buffer lock").push(push);
        })
    };
    let mut dispatcher = HostRequestDispatcher::with_session_token(
        Some(workspace.to_path_buf()),
        SessionToken("session-1".into()),
        Vec::new(),
        push_sink,
    );

    let handshake = dispatcher
        .handshake(handshake_request())
        .await
        .expect("handshake should negotiate");
    assert_eq!(handshake.session_token.0, "session-1");
    assert_eq!(handshake.negotiated_version, CURRENT_PROTOCOL_VERSION);

    let current = dispatcher
        .dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(1),
            command: ClientCommand::WorkspaceCurrent,
        })
        .await;
    let workspace_id = match current.result.expect("workspace current should succeed") {
        CommandSuccess::WorkspaceCurrent(response) => {
            assert_eq!(
                response.root_name,
                workspace.file_name().unwrap().to_string_lossy()
            );
            response.workspace_id
        }
        other => panic!("unexpected workspace current response: {other:?}"),
    };

    let list = dispatcher
        .dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(2),
            command: ClientCommand::WorkspaceList(WorkspaceListRequest { directory: None }),
        })
        .await;
    let entries = match list.result.expect("workspace list should succeed") {
        CommandSuccess::WorkspaceList(response) => response.entries,
        other => panic!("unexpected workspace list response: {other:?}"),
    };
    let readme = entries
        .iter()
        .find(|entry| {
            entry
                .path
                .as_ref()
                .is_some_and(|path| path.display_path() == "README.md")
        })
        .expect("README entry should exist")
        .path
        .as_ref()
        .expect("README entry should be operable")
        .clone();
    let model = entries
        .iter()
        .find(|entry| {
            entry
                .path
                .as_ref()
                .is_some_and(|path| path.display_path() == "model.stl")
        })
        .expect("model entry should exist")
        .path
        .as_ref()
        .expect("model entry should be operable")
        .clone();
    assert_eq!(readme.workspace_id().0, workspace_id.0);

    let file_read = dispatcher
        .dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(3),
            command: ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: readme }),
        })
        .await;
    match file_read.result.expect("file read should succeed") {
        CommandSuccess::FileRead(response) => match response.contents {
            app_server_protocol::FileReadContents::Utf8Text(text) => {
                assert!(text.contains("hello"))
            }
            other => panic!("unexpected file contents: {other:?}"),
        },
        other => panic!("unexpected file read response: {other:?}"),
    }

    let preview = dispatcher
        .dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(4),
            command: ClientCommand::PreviewRequest(PreviewRequest {
                source: model,
                defines: vec![],
                kind: PreviewRequestKind::GeometryArtifact,
                configured_openscad_path: None,
            }),
        })
        .await;
    match preview.result.expect("preview should succeed") {
        CommandSuccess::PreviewReady(response) => match response.artifact {
            PreviewArtifact::Stl(stl) => assert!(!stl.bytes.is_empty()),
            other => panic!("unexpected preview artifact: {other:?}"),
        },
        other => panic!("unexpected preview response: {other:?}"),
    }

    assert!(pushes.lock().expect("push buffer lock").is_empty());
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn shared_dispatcher_rejects_unsupported_protocol_version() {
    let workspace = temp_workspace("shared-dispatcher-protocol-version");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = dispatcher
        .handshake(handshake_request_with_version(ProtocolVersionRange::new(
            2, 2,
        )))
        .await
        .expect_err("protocol version without overlap should reject");

    assert_eq!(error.code, ProtocolErrorCode::UnsupportedProtocolVersion);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_model_commands_update_active_snapshot() {
    let workspace = temp_workspace("dispatcher-agent-model-registry");
    let config_path = workspace.join("agents.toml");
    std::fs::write(&config_path, agent_model_registry_config()).unwrap();
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let config_before = std::fs::read_to_string(workspace.join("agents.toml")).unwrap();

    let registry =
        dispatch_agent_model_command(&mut dispatcher, 10, ClientCommand::AgentModelRegistry).await;
    assert_eq!(registry.active_model_id, "gpt-5.2");
    assert_eq!(registry.active_reasoning_effort.as_deref(), Some("high"));
    assert_eq!(registry.active_service_label.as_deref(), Some("default"));

    let cleared_defaults = dispatch_agent_model_command(
        &mut dispatcher,
        11,
        ClientCommand::AgentModelParamsUpdate(AgentModelParamsUpdateRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5.2".into(),
            reasoning_effort: None,
            service_label: None,
        }),
    )
    .await;
    assert_eq!(cleared_defaults.active_model_id, "gpt-5.2");
    assert_eq!(cleared_defaults.active_reasoning_effort, None);
    assert_eq!(cleared_defaults.active_service_label, None);

    let selected = dispatch_agent_model_command(
        &mut dispatcher,
        12,
        ClientCommand::AgentModelSelect(AgentModelSelectRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
        }),
    )
    .await;
    assert_eq!(selected.active_model_id, "gpt-5-mini");

    let updated = dispatch_agent_model_command(
        &mut dispatcher,
        13,
        ClientCommand::AgentModelParamsUpdate(AgentModelParamsUpdateRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
            reasoning_effort: Some("low".into()),
            service_label: Some("flex".into()),
        }),
    )
    .await;
    assert_eq!(updated.active_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(updated.active_service_label.as_deref(), Some("flex"));
    assert!(updated.active_reasoning_effort_applied);
    assert!(updated.active_service_label_applied);

    let reasoning_only = dispatch_agent_model_command(
        &mut dispatcher,
        14,
        ClientCommand::AgentModelParamsUpdate(AgentModelParamsUpdateRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
            reasoning_effort: Some("medium".into()),
            service_label: None,
        }),
    )
    .await;
    assert_eq!(
        reasoning_only.active_reasoning_effort.as_deref(),
        Some("medium")
    );
    assert_eq!(reasoning_only.active_service_label, None);

    let completions = dispatch_agent_model_command(
        &mut dispatcher,
        15,
        ClientCommand::AgentModelSelect(AgentModelSelectRequest {
            provider_id: "openai_completions".into(),
            model_id: "gpt-4o".into(),
        }),
    )
    .await;
    assert_eq!(completions.active_provider_id, "openai_completions");
    assert!(!completions.active_reasoning_effort_applied);
    assert!(!completions.active_service_label_applied);
    assert!(completions.service_label_options.is_empty());
    let completions_model = completions.providers[1]
        .models
        .iter()
        .find(|model| model.id == "gpt-4o")
        .expect("configured completions model");
    assert!(completions_model.native_web_search_enabled);
    assert!(!completions_model.native_web_search_applied);

    dispatch_agent_model_command(
        &mut dispatcher,
        16,
        ClientCommand::AgentModelParamsUpdate(AgentModelParamsUpdateRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
            reasoning_effort: Some("medium".into()),
            service_label: None,
        }),
    )
    .await;

    let disabled_search = reasoning_only.providers[0]
        .models
        .iter()
        .find(|model| model.id == "gpt-5-mini")
        .expect("configured mini model");
    assert!(disabled_search.native_web_search_enabled);
    assert!(!disabled_search.native_web_search_applied);
    assert_eq!(
        std::fs::read_to_string(workspace.join("agents.toml")).unwrap(),
        config_before,
    );

    let handshake = dispatcher
        .handshake(handshake_request())
        .await
        .expect("handshake should expose runtime model state");
    let handshake_registry = handshake
        .server_capabilities
        .agent_model_registry
        .expect("model registry capability");
    assert_eq!(handshake_registry.active_model_id, "gpt-5-mini");
    assert_eq!(
        handshake_registry.active_reasoning_effort.as_deref(),
        Some("medium")
    );
    assert_eq!(handshake_registry.active_service_label, None);
    cleanup_workspace(&workspace);
}

#[cfg(unix)]
#[test]
fn export_run_rejects_symlink_escape_output_target() {
    let workspace = temp_workspace("dispatcher-export-symlink");
    let outside =
        std::env::temp_dir().join(format!("dispatcher-export-outside-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, workspace.join("linked")).unwrap();

    let pushes = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
    let push_sink = {
        let pushes = Arc::clone(&pushes);
        Arc::new(move |push: ServerPushEnvelope| {
            pushes.lock().expect("push buffer lock").push(push);
        })
    };
    let mut dispatcher = HostRequestDispatcher::with_session_token(
        Some(workspace.to_path_buf()),
        SessionToken("session-1".into()),
        Vec::new(),
        push_sink,
    );
    let source = app_server_protocol::PathHandle::new(
        app_server_protocol::WorkspaceId::new("workspace"),
        ["model.stl"],
    )
    .unwrap();
    let output_path = app_server_protocol::PathHandle::new(
        app_server_protocol::WorkspaceId::new("workspace"),
        ["linked", "out.3mf"],
    )
    .unwrap();

    let response = dispatch(
        &mut dispatcher,
        10,
        ClientCommand::ExportRun(ExportRunRequest {
            configured_openscad_path: Some(HostLocalPath::new("/bin/false").unwrap()),
            configured_slicers: Vec::new(),
            source,
            defines: Vec::new(),
            output_path,
            format: ExportFormat::ThreeMf,
            slicer_name: None,
        }),
    );

    let error = response
        .result
        .expect_err("symlink escape should be rejected");
    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);

    let _ = std::fs::remove_file(workspace.join("linked"));
    let _ = std::fs::remove_dir(outside);
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_persists_chat_jsonl_and_selection_snapshot() {
    let workspace = temp_workspace("dispatcher-chat-selection");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let related = path_handle(["parts", "top_lid.py"]);
    let created = create_chat(&mut dispatcher, "main chat", vec![related.clone()]);

    assert_ne!(created.session_id, ChatSessionId("main-chat".into()));
    let ack = send_chat(&mut dispatcher, &created.session_id, vec![related.clone()]);
    assert_eq!(ack.session_id, created.session_id);
    let sessions = list_chats(&mut dispatcher, false);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].message_count, 3);
    assert_eq!(sessions[0].related_files, vec![related.clone()]);
    let history = read_chat_history(&mut dispatcher, &created.session_id);
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].role, ChatRole::Meta);
    assert_eq!(history[1].content, "Start main chat");
    assert_eq!(history[2].content, "make the lid taller");
    let updated = update_selection(&mut dispatcher);
    assert_eq!(updated.accepted_count, 1);
    archive_chat(&mut dispatcher, &created.session_id);
    let index = read_chats_json(&workspace);
    assert_eq!(index["chats"][0]["archived"].as_bool(), Some(true));
    let active = list_chats(&mut dispatcher, false);
    assert!(active.is_empty());
    assert!(
        pushes
            .lock()
            .expect("push buffer lock")
            .iter()
            .all(|push| matches!(push.event, ServerPushEvent::ChatListChanged(_)))
    );
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_rejects_chat_create_without_initial_user_message() {
    let workspace = temp_workspace("dispatcher-chat-create-requires-message");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = dispatch(
        &mut dispatcher,
        21,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "empty chat".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("create-empty".into()),
            initial_user_message: None,
            requested_model: None,
            initial_turn: None,
        }),
    )
    .result
    .expect_err("chat.create without first user message should fail");

    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_rejects_chat_create_with_empty_client_request_id() {
    let workspace = temp_workspace("dispatcher-chat-create-requires-request-id");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = dispatch(
        &mut dispatcher,
        22,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "empty request".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("  ".into()),
            initial_user_message: Some("start".into()),
            requested_model: None,
            initial_turn: None,
        }),
    )
    .result
    .expect_err("chat.create with blank request id should fail");

    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_rejects_chat_create_initial_turn_without_model() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-requires-model");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = dispatch(
        &mut dispatcher,
        23,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "missing model".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("create-with-turn".into()),
            initial_user_message: Some("start".into()),
            requested_model: None,
            initial_turn: Some(ChatCreateInitialTurn {
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: Vec::new(),
            }),
        }),
    )
    .result
    .expect_err("chat.create initial turn without model should fail");

    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_starts_agent_and_persists_bound_model() {
    let workspace = temp_workspace("dispatcher-chat-create-starts-turn");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let model = bound_agent_model();

    let response = dispatch_async(
        &mut dispatcher,
        24,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "initial turn".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("create-start".into()),
            initial_user_message: Some("make a hinge".into()),
            requested_model: Some(model.clone()),
            initial_turn: Some(ChatCreateInitialTurn {
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: vec!["@part[hinge]".into()],
            }),
        }),
    )
    .await;
    let created = match response.result.expect("chat.create should succeed") {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    };
    let started = created
        .initial_turn
        .as_ref()
        .expect("initial turn response");

    assert_eq!(started.session_id, created.session_id);
    assert_eq!(started.agent_id, created.agent_id);
    assert_eq!(started.turn_id.0, started.run_id);
    wait_for_terminal_event_async(&pushes, &started.run_id).await;

    let index = read_chats_json(&workspace);
    assert_eq!(
        index["chats"][0]["bound_model"]["provider_id"].as_str(),
        Some(model.provider_id.as_str())
    );
    assert_eq!(
        index["chats"][0]["bound_model"]["model_id"].as_str(),
        Some(model.model_id.as_str())
    );
    assert_eq!(index["chats"][0]["bound_model"].get("base_url"), None);
    let history = read_chat_history_async(&mut dispatcher, &created.session_id).await;
    assert!(
        history
            .iter()
            .any(|message| message.content == "make a hinge")
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_advances_workspace_run_id_cursor() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-run-id-cursor");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let existing = store
        .create("existing run cursor", None, Vec::new())
        .await
        .expect("existing chat session should be created");
    store
        .append_agent_event(
            &existing.agent_id,
            &agent_event_record(
                1,
                &existing.agent_id,
                &AgentTurnId("agent-100".into()),
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed high workspace run id");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);

    let created = match dispatch_async(
        &mut dispatcher,
        25,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "initial turn cursor".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("create-start-cursor".into()),
            initial_user_message: Some("make a hinge".into()),
            requested_model: Some(bound_agent_model()),
            initial_turn: Some(ChatCreateInitialTurn {
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: Vec::new(),
            }),
        }),
    )
    .await
    .result
    .expect("chat.create should succeed")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    };
    let started = created.initial_turn.expect("initial turn response");

    assert_eq!(started.turn_id, AgentTurnId("agent-101".into()));
    wait_for_terminal_event_async(&pushes, &started.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_rejects_missing_workspace_event_log() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-missing-log");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let broken = store
        .create("missing initial turn cursor", None, Vec::new())
        .await
        .expect("broken chat session should be created");
    std::fs::remove_file(
        workspace
            .join("agent-events")
            .join(format!("{}.jsonl", broken.agent_id.0)),
    )
    .expect("remove workspace event log");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = dispatch_async(
        &mut dispatcher,
        26,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "blocked initial turn".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("create-start-missing-log".into()),
            initial_user_message: Some("make a hinge".into()),
            requested_model: Some(bound_agent_model()),
            initial_turn: Some(ChatCreateInitialTurn {
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: Vec::new(),
            }),
        }),
    )
    .await
    .result
    .expect_err("missing workspace event log should reject initial turn");

    assert_eq!(error.code, ProtocolErrorCode::NotFound);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_rejects_busy_without_creating_chat() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-busy");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let existing = create_chat_async(&mut dispatcher, "busy source", Vec::new()).await;
    let running = invoke_agent_async(&mut dispatcher, 25, &existing.session_id, "start")
        .await
        .expect("agent starts");

    let response = dispatch_async(
        &mut dispatcher,
        26,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: "should not persist".into(),
            goal: None,
            related_files: Vec::new(),
            client_request_id: Some("busy-create".into()),
            initial_user_message: Some("make a hinge".into()),
            requested_model: Some(bound_agent_model()),
            initial_turn: Some(ChatCreateInitialTurn {
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: Vec::new(),
            }),
        }),
    )
    .await;

    let error = response
        .result
        .expect_err("busy initial turn create should reject");
    assert_eq!(error.code, ProtocolErrorCode::AgentBusy);
    assert_eq!(list_chats_async(&mut dispatcher, false).await.len(), 1);
    wait_for_terminal_event_async(&pushes, &running.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_retry_does_not_start_second_turn() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-retry");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let request = ChatCreateRequest {
        title: "initial turn retry".into(),
        goal: None,
        related_files: Vec::new(),
        client_request_id: Some("create-start-retry".into()),
        initial_user_message: Some("make a hinge".into()),
        requested_model: Some(bound_agent_model()),
        initial_turn: Some(ChatCreateInitialTurn {
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
        }),
    };
    let created = match dispatch_async(
        &mut dispatcher,
        27,
        ClientCommand::ChatCreate(request.clone()),
    )
    .await
    .result
    .expect("first create succeeds")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    };
    let first_run_id = created
        .initial_turn
        .as_ref()
        .expect("first initial turn")
        .run_id
        .clone();
    wait_for_terminal_event_async(&pushes, &first_run_id).await;
    pushes.lock().expect("push buffer lock").clear();

    let retried = match dispatch_async(&mut dispatcher, 28, ClientCommand::ChatCreate(request))
        .await
        .result
        .expect("retry succeeds")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    };

    assert_eq!(retried.session_id, created.session_id);
    assert_eq!(retried.agent_id, created.agent_id);
    assert!(retried.initial_turn.is_none());
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(
        pushes
            .lock()
            .expect("push buffer lock")
            .iter()
            .all(|push| !matches!(
                push.event,
                ServerPushEvent::AgentToken(_)
                    | ServerPushEvent::AgentReasoning(_)
                    | ServerPushEvent::AgentToolStart(_)
                    | ServerPushEvent::AgentToolResult(_)
                    | ServerPushEvent::AgentDone(_)
                    | ServerPushEvent::AgentError(_)
            )),
        "retry must not spawn another worker"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_chat_create_initial_turn_deduplicates_concurrent_create_request() {
    let workspace = temp_workspace("dispatcher-chat-create-turn-concurrent");
    let _agent_env = unset_agent_environment();
    let (mut first_dispatcher, first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let request = ChatCreateRequest {
        title: "initial turn concurrent".into(),
        goal: None,
        related_files: Vec::new(),
        client_request_id: Some("create-start-concurrent".into()),
        initial_user_message: Some("make a hinge".into()),
        requested_model: Some(bound_agent_model()),
        initial_turn: Some(ChatCreateInitialTurn {
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
        }),
    };

    let (first_response, second_response) = tokio::join!(
        dispatch_async(
            &mut first_dispatcher,
            29,
            ClientCommand::ChatCreate(request.clone())
        ),
        dispatch_async(
            &mut second_dispatcher,
            30,
            ClientCommand::ChatCreate(request)
        ),
    );
    let first = chat_created_from_response(first_response);
    let second = chat_created_from_response(second_response);

    assert_eq!(first.session_id, second.session_id);
    assert_eq!(first.agent_id, second.agent_id);
    assert_eq!(
        usize::from(first.initial_turn.is_some()) + usize::from(second.initial_turn.is_some()),
        1
    );
    let session_id = first.session_id.clone();
    let (started, pushes) = if let Some(started) = first.initial_turn {
        (started, &first_pushes)
    } else {
        (
            second.initial_turn.expect("one turn starts"),
            &second_pushes,
        )
    };
    wait_for_terminal_event_async(pushes, &started.run_id).await;
    assert_eq!(
        list_chats_async(&mut first_dispatcher, false).await.len(),
        1
    );
    let history = read_chat_history_async(&mut first_dispatcher, &session_id).await;
    assert_eq!(
        history
            .iter()
            .filter(|message| message.role == ChatRole::User && message.content == "make a hinge")
            .count(),
        1
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_subscribe_receives_events_from_other_dispatcher() {
    let workspace = temp_workspace("dispatcher-agent-subscribe-observer");
    let _agent_env = unset_agent_environment();
    let (mut first_dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut first_dispatcher, "runtime observer", Vec::new()).await;

    subscribe_agent_async(&mut second_dispatcher, 31, &created.agent_id).await;
    let started = start_agent_turn_async(
        &mut first_dispatcher,
        32,
        &created.agent_id,
        "summarize current model",
    )
    .await;

    wait_for_terminal_event_async(&second_pushes, &started.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_snapshot_reads_active_agent_from_second_dispatcher() {
    let workspace = temp_workspace("dispatcher-agent-snapshot-observer");
    let (config_path, server_handle) = hanging_agent_config(&workspace).await;
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut first_dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, _second_pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut first_dispatcher, "runtime snapshot", Vec::new()).await;
    let started = start_agent_turn_async(
        &mut first_dispatcher,
        33,
        &created.agent_id,
        "summarize current model",
    )
    .await;
    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", created.agent_id.0));
    wait_for_agent_event_records_async(&event_log_path, 1).await;

    let snapshot = agent_snapshot_async(&mut second_dispatcher, 34, &created.agent_id).await;

    assert_eq!(snapshot.agent_id, created.agent_id);
    assert_eq!(snapshot.chat_id, created.session_id);
    assert_eq!(snapshot.active_turn_id, Some(started.turn_id.clone()));
    assert_eq!(snapshot.state, AgentRuntimeStatus::Running);
    assert!(matches!(
        snapshot.events.first().map(|event| &event.payload),
        Some(app_server_protocol::AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::Running
        })
    ));
    let persisted_events = ChatStore::new(workspace.clone())
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read active event log");
    assert!(
        persisted_events.iter().all(|event| !matches!(
            event.payload,
            AgentEventPayload::StateChanged {
                state: AgentRuntimeStatus::Interrupted
            }
        )),
        "snapshot of a live runtime must not append interrupted"
    );
    let cancelled = cancel_agent_async(&mut first_dispatcher, 35, &created.agent_id).await;
    assert!(cancelled.cancelled);
    wait_for_terminal_event_async(&pushes, &started.run_id).await;
    server_handle.abort();
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_subscribe_replays_events_by_event_cursor() {
    let workspace = temp_workspace("dispatcher-agent-subscribe-replay");
    let _agent_env = unset_agent_environment();
    let (mut first_dispatcher, first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let (mut third_dispatcher, third_pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut first_dispatcher, "runtime replay", Vec::new()).await;
    let started = start_agent_turn_async(
        &mut first_dispatcher,
        39,
        &created.agent_id,
        "summarize current model",
    )
    .await;
    wait_for_terminal_event_async(&first_pushes, &started.run_id).await;

    let snapshot = agent_snapshot_async(&mut first_dispatcher, 40, &created.agent_id).await;
    assert!(
        snapshot
            .events
            .windows(2)
            .all(|events| { events[0].event_id.0 < events[1].event_id.0 })
    );
    let last_event_id = snapshot
        .events
        .last()
        .expect("snapshot should include terminal event")
        .event_id;

    subscribe_agent_with_cursor_async(&mut second_dispatcher, 41, &created.agent_id, None).await;
    wait_for_terminal_event_async(&second_pushes, &started.run_id).await;

    third_pushes.lock().expect("push buffer lock").clear();
    subscribe_agent_with_cursor_async(
        &mut third_dispatcher,
        42,
        &created.agent_id,
        Some(last_event_id),
    )
    .await;
    assert!(
        third_pushes.lock().expect("push buffer lock").is_empty(),
        "subscribe after the latest event cursor must not replay old events"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_event_log_persists_runtime_events_outside_chat_jsonl() {
    let workspace = temp_workspace("dispatcher-agent-event-log-file");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut dispatcher, "runtime event log", Vec::new()).await;
    let started = start_agent_turn_async(
        &mut dispatcher,
        46,
        &created.agent_id,
        "summarize current model",
    )
    .await;
    wait_for_terminal_event_async(&pushes, &started.run_id).await;

    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", created.agent_id.0));
    let records = wait_for_agent_event_records_async(&event_log_path, 2).await;
    assert!(
        records.len() >= 2,
        "running and terminal events should be persisted"
    );
    assert!(
        records
            .iter()
            .all(|record| record.agent_id == created.agent_id)
    );
    assert!(records.iter().all(|record| record.ts_ms > 0));
    assert!(
        records
            .windows(2)
            .all(|pair| pair[0].event_id.0 < pair[1].event_id.0),
        "event ids must be monotonic per agent"
    );
    assert!(
        records.iter().all(|record| match record.payload {
            app_server_protocol::AgentEventPayload::StateChanged { .. } => {
                record.turn_id.is_some()
            }
            _ => record.turn_id.is_some(),
        }),
        "turn runtime events must include turn_id"
    );

    let chat_history = std::fs::read_to_string(
        workspace
            .join("chats")
            .join(format!("{}.jsonl", created.session_id.0)),
    )
    .expect("chat jsonl should exist");
    assert!(
        !chat_history.contains("\"agent.token\"")
            && !chat_history.contains("\"agent.reasoning\"")
            && !chat_history.contains("\"event_id\""),
        "Chat JSONL must not store runtime replay events"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_event_id_continues_after_persisted_log() {
    let workspace = temp_workspace("dispatcher-agent-event-id-resume");
    let (config_path, server_handle) = hanging_agent_config(&workspace).await;
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("event id resume", None, Vec::new())
        .await
        .expect("chat session should be created");
    store
        .append_agent_event(
            &created.agent_id,
            &app_server_protocol::AgentEventRecord {
                event_id: AgentEventId(9),
                agent_id: created.agent_id.clone(),
                turn_id: Some(app_server_protocol::AgentTurnId("old-turn".into())),
                ts_ms: 100,
                payload: app_server_protocol::AgentEventPayload::Done { cancelled: false },
            },
        )
        .await
        .expect("seed persisted agent event");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);

    let started =
        start_agent_turn_async(&mut dispatcher, 34, &created.agent_id, "resume ids").await;
    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", created.agent_id.0));
    let records = wait_for_agent_event_records_async(&event_log_path, 2).await;

    assert_eq!(records[1].event_id, AgentEventId(10));
    let cancelled = cancel_agent_async(&mut dispatcher, 35, &created.agent_id).await;
    assert!(cancelled.cancelled);
    wait_for_terminal_event_async(&pushes, &started.run_id).await;
    server_handle.abort();
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_appends_interrupted_event_for_unfinished_persisted_turn() {
    let workspace = temp_workspace("dispatcher-agent-recover-interrupted");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover interrupted", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 36, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Interrupted);
    assert_eq!(snapshot.active_turn_id, None);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::Interrupted
        })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_recovery_is_idempotent_across_concurrent_observers() {
    let workspace = temp_workspace("dispatcher-agent-recover-concurrent");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover concurrent", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    let mut tasks = Vec::new();
    for request_id in 40..50 {
        let workspace = workspace.clone();
        let agent_id = created.agent_id.clone();
        tasks.push(tokio::spawn(async move {
            let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
            agent_snapshot_async(&mut dispatcher, request_id, &agent_id).await
        }));
    }

    let snapshots = join_all(tasks).await;

    assert!(snapshots.into_iter().all(|snapshot| {
        snapshot.expect("snapshot task joins").state == AgentRuntimeStatus::Interrupted
    }));
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    let interrupted = records
        .iter()
        .filter(|record| {
            matches!(
                record.payload,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Interrupted
                }
            )
        })
        .count();
    assert_eq!(interrupted, 1);
    assert!(
        records
            .windows(2)
            .all(|pair| pair[0].event_id.0 < pair[1].event_id.0),
        "recovery must preserve per-agent event id monotonicity"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_appends_recovered_done_when_final_fact_exists() {
    let workspace = temp_workspace("dispatcher-agent-recover-done");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover done", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed final assistant fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 37, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Done);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: false })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_recovers_done_when_event_log_empty_and_final_fact_exists() {
    let workspace = temp_workspace("dispatcher-agent-recover-empty-log-final");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover empty event log final", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("agent-1".into());
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed final assistant fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 45, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Done);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: false })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_recovers_newer_chat_final_fact_when_event_log_is_stale() {
    let workspace = temp_workspace("dispatcher-agent-recover-stale-log-final");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover stale event log final", None, Vec::new())
        .await
        .expect("chat session should be created");
    let old_turn_id = AgentTurnId("agent-1".into());
    let new_turn_id = AgentTurnId("agent-2".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &old_turn_id,
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed stale event log");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &new_turn_id,
            Some(new_turn_id.0.clone()),
        )
        .await
        .expect("seed newer final assistant fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 46, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Done);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| (&record.turn_id, &record.payload)),
        Some((Some(turn_id), AgentEventPayload::Done { cancelled: false }))
            if turn_id == &new_turn_id
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_marks_failed_when_error_fact_lacks_terminal_event() {
    let workspace = temp_workspace("dispatcher-agent-recover-error-fact");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover error fact", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            &format!("{AGENT_ERROR_FACT_PREFIX} (LlmError): Rig Agent is not configured"),
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed failed assistant fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 43, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Failed);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::Failed
        })
    ));
    assert!(
        !matches!(
            records.last().map(|record| &record.payload),
            Some(AgentEventPayload::Done { cancelled: false })
        ),
        "failed assistant fact must not recover as successful done"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_preserves_cancelled_without_final_fact() {
    let workspace = temp_workspace("dispatcher-agent-recover-cancelled");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover cancelled", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                2,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::Done { cancelled: true },
            ),
        )
        .await
        .expect("seed cancelled terminal event");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 44, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Cancelled);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: true })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_marks_failed_needs_recovery_when_failed_terminal_has_success_fact() {
    let workspace = temp_workspace("dispatcher-agent-recover-failed-success");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover failed success", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                2,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::Error {
                    error_type: app_server_protocol::AgentErrorType::LlmError,
                    message: "model failed".into(),
                },
            ),
        )
        .await
        .expect("seed failed terminal event");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed conflicting success fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 45, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::FailedNeedsRecovery);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_marks_failed_needs_recovery_when_cancelled_has_final_fact() {
    let workspace = temp_workspace("dispatcher-agent-recover-cancelled-success");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover cancelled success", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                2,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::Done { cancelled: true },
            ),
        )
        .await
        .expect("seed cancelled terminal event");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed conflicting final fact");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 46, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::FailedNeedsRecovery);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_startup_recovers_done_when_final_fact_exists() {
    let workspace = temp_workspace("dispatcher-agent-startup-recover-done");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("startup recover done", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed final assistant fact");
    let (_dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", created.agent_id.0));

    let records = wait_for_agent_event_records_async(&event_log_path, 2).await;

    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: false })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_startup_recovers_other_chats_when_one_event_log_is_missing() {
    let workspace = temp_workspace("dispatcher-agent-startup-recover-skips-corrupt");
    let store = ChatStore::new(workspace.clone());
    let broken = store
        .create("broken event log", None, Vec::new())
        .await
        .expect("broken chat session should be created");
    let recoverable = store
        .create("recoverable event log", None, Vec::new())
        .await
        .expect("recoverable chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    std::fs::remove_file(
        workspace
            .join("agent-events")
            .join(format!("{}.jsonl", broken.agent_id.0)),
    )
    .expect("remove broken event log");
    store
        .append_agent_event(
            &recoverable.agent_id,
            &agent_event_record(
                1,
                &recoverable.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed recoverable running event");
    store
        .append_message_with_agent_turn(
            &recoverable.session_id,
            ChatRole::Assistant,
            "final answer",
            &recoverable.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed recoverable final assistant fact");
    let (_dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", recoverable.agent_id.0));

    let records = wait_for_agent_event_records_async(&event_log_path, 2).await;

    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: false })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_startup_recovers_other_chats_when_one_messages_file_is_missing() {
    let workspace = temp_workspace("dispatcher-agent-startup-recover-skips-missing-messages");
    let store = ChatStore::new(workspace.clone());
    let broken = store
        .create("broken messages", None, Vec::new())
        .await
        .expect("broken chat session should be created");
    let recoverable = store
        .create("recoverable messages", None, Vec::new())
        .await
        .expect("recoverable chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    std::fs::remove_file(
        workspace
            .join("chats")
            .join(format!("{}.jsonl", broken.session_id.0)),
    )
    .expect("remove broken messages file");
    store
        .append_agent_event(
            &recoverable.agent_id,
            &agent_event_record(
                1,
                &recoverable.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed recoverable running event");
    store
        .append_message_with_agent_turn(
            &recoverable.session_id,
            ChatRole::Assistant,
            "final answer",
            &recoverable.agent_id,
            &turn_id,
            Some(turn_id.0.clone()),
        )
        .await
        .expect("seed recoverable final assistant fact");
    let (_dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let event_log_path = workspace
        .join("agent-events")
        .join(format!("{}.jsonl", recoverable.agent_id.0));

    let records = wait_for_agent_event_records_async(&event_log_path, 2).await;

    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::Done { cancelled: false })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_start_turn_advances_turn_id_after_recovered_interrupted() {
    let workspace = temp_workspace("dispatcher-agent-recover-turn-id-cursor");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover turn cursor", None, Vec::new())
        .await
        .expect("chat session should be created");
    let old_turn_id = AgentTurnId("agent-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &old_turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed old running event");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 39, &created.agent_id).await;
    let restarted = start_agent_turn_async(
        &mut dispatcher,
        40,
        &created.agent_id,
        "restart after recovery",
    )
    .await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::Interrupted);
    assert_ne!(restarted.turn_id, old_turn_id);
    wait_for_terminal_event_async(&pushes, &restarted.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_start_turn_advances_workspace_run_id_cursor() {
    let workspace = temp_workspace("dispatcher-agent-workspace-run-id-cursor");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let first = store
        .create("first run cursor", None, Vec::new())
        .await
        .expect("first chat session should be created");
    let second = store
        .create("second run cursor", None, Vec::new())
        .await
        .expect("second chat session should be created");
    store
        .append_agent_event(
            &first.agent_id,
            &agent_event_record(
                1,
                &first.agent_id,
                &AgentTurnId("agent-100".into()),
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed high workspace run id");
    store
        .append_agent_event(
            &second.agent_id,
            &agent_event_record(
                1,
                &second.agent_id,
                &AgentTurnId("agent-1".into()),
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed low target run id");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);

    let restarted = start_agent_turn_async(
        &mut dispatcher,
        41,
        &second.agent_id,
        "restart after cursor",
    )
    .await;

    assert_eq!(restarted.turn_id, AgentTurnId("agent-101".into()));
    wait_for_terminal_event_async(&pushes, &restarted.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_start_turn_advances_workspace_run_id_cursor_from_chat_final_fact() {
    let workspace = temp_workspace("dispatcher-agent-workspace-run-id-chat-fact");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let high = store
        .create("high chat fact cursor", None, Vec::new())
        .await
        .expect("high chat session should be created");
    let target = store
        .create("target cursor", None, Vec::new())
        .await
        .expect("target chat session should be created");
    let high_turn_id = AgentTurnId("agent-100".into());
    store
        .append_message_with_agent_turn(
            &high.session_id,
            ChatRole::Assistant,
            "final answer",
            &high.agent_id,
            &high_turn_id,
            Some(high_turn_id.0.clone()),
        )
        .await
        .expect("seed high chat final fact");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);

    let restarted = start_agent_turn_async(
        &mut dispatcher,
        47,
        &target.agent_id,
        "restart after chat cursor",
    )
    .await;

    assert_eq!(restarted.turn_id, AgentTurnId("agent-101".into()));
    wait_for_terminal_event_async(&pushes, &restarted.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_start_turn_rejects_when_workspace_event_log_is_missing() {
    let workspace = temp_workspace("dispatcher-agent-workspace-run-id-missing-log");
    let _agent_env = unset_agent_environment();
    let store = ChatStore::new(workspace.clone());
    let broken = store
        .create("missing run cursor", None, Vec::new())
        .await
        .expect("broken chat session should be created");
    let target = store
        .create("target run cursor", None, Vec::new())
        .await
        .expect("target chat session should be created");
    std::fs::remove_file(
        workspace
            .join("agent-events")
            .join(format!("{}.jsonl", broken.agent_id.0)),
    )
    .expect("remove workspace event log");
    store
        .append_agent_event(
            &target.agent_id,
            &agent_event_record(
                1,
                &target.agent_id,
                &AgentTurnId("agent-1".into()),
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed target event log");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let error = start_agent_turn_result_async(&mut dispatcher, 42, &target.agent_id, "blocked")
        .await
        .expect_err("workspace cursor scan must reject missing event log");

    assert_eq!(error.code, ProtocolErrorCode::NotFound);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_marks_failed_needs_recovery_when_terminal_lacks_final_fact() {
    let workspace = temp_workspace("dispatcher-agent-recover-needs-final");
    let store = ChatStore::new(workspace.clone());
    let created = store
        .create("recover missing final", None, Vec::new())
        .await
        .expect("chat session should be created");
    let turn_id = AgentTurnId("turn-1".into());
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                1,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            ),
        )
        .await
        .expect("seed running event");
    store
        .append_agent_event(
            &created.agent_id,
            &agent_event_record(
                2,
                &created.agent_id,
                &turn_id,
                AgentEventPayload::Done { cancelled: false },
            ),
        )
        .await
        .expect("seed terminal event");
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let snapshot = agent_snapshot_async(&mut dispatcher, 38, &created.agent_id).await;

    assert_eq!(snapshot.state, AgentRuntimeStatus::FailedNeedsRecovery);
    let records = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect("read recovered events");
    assert!(matches!(
        records.last().map(|record| &record.payload),
        Some(AgentEventPayload::StateChanged {
            state: AgentRuntimeStatus::FailedNeedsRecovery
        })
    ));
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_disconnect_does_not_cancel_active_agent_and_second_observer_can_cancel() {
    let workspace = temp_workspace("dispatcher-agent-disconnect-cancel");
    let (config_path, server_handle) = hanging_agent_config(&workspace).await;
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut first_dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut first_dispatcher, "runtime cancel", Vec::new()).await;
    subscribe_agent_async(&mut second_dispatcher, 35, &created.agent_id).await;
    let started = start_agent_turn_async(
        &mut first_dispatcher,
        36,
        &created.agent_id,
        "summarize current model",
    )
    .await;

    first_dispatcher.disconnect();
    let snapshot = agent_snapshot_async(&mut second_dispatcher, 37, &created.agent_id).await;
    assert_eq!(snapshot.state, AgentRuntimeStatus::Running);
    let cancelled = cancel_agent_async(&mut second_dispatcher, 38, &created.agent_id).await;
    assert!(cancelled.cancelled);

    let done = wait_for_done_event_async(&second_pushes, &started.run_id).await;
    assert!(done.cancelled);
    server_handle.abort();
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_second_observer_start_turn_uses_bound_model() {
    let workspace = temp_workspace("dispatcher-agent-bound-model-observer");
    let config_path = workspace.join("agents.toml");
    std::fs::write(
        &config_path,
        agent_model_registry_config_without_bound_model(),
    )
    .expect("write agent config");
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut first_dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let created =
        create_chat_with_model_async(&mut first_dispatcher, "bound model", bound_agent_model())
            .await;

    dispatch_agent_model_command(
        &mut second_dispatcher,
        43,
        ClientCommand::AgentModelSelect(AgentModelSelectRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
        }),
    )
    .await;
    let started = start_agent_turn_async(
        &mut second_dispatcher,
        44,
        &created.agent_id,
        "use the bound model",
    )
    .await;
    let snapshot = agent_snapshot_async(&mut second_dispatcher, 45, &created.agent_id).await;
    assert_eq!(snapshot.bound_model, Some(bound_agent_model()));
    assert_eq!(
        snapshot.model_lock_reason.as_deref(),
        Some("chat_bound_model")
    );

    let error = wait_for_error_event_async(&second_pushes, &started.run_id).await;
    assert!(
        error.message.contains("active model is missing"),
        "bound model should be used instead of second observer selected model: {}",
        error.message
    );
    wait_for_terminal_event_async(&second_pushes, &started.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_workspace_runtime_rejects_second_active_turn_across_dispatchers() {
    let workspace = temp_workspace("dispatcher-agent-cross-busy");
    let (config_path, server_handle) = hanging_agent_config(&workspace).await;
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut first_dispatcher, first_pushes) = dispatcher_with_pushes(&workspace);
    let (mut second_dispatcher, _second_pushes) = dispatcher_with_pushes(&workspace);
    let first = create_chat_async(&mut first_dispatcher, "busy first", Vec::new()).await;
    let second = create_chat_async(&mut second_dispatcher, "busy second", Vec::new()).await;
    let started = start_agent_turn_async(
        &mut first_dispatcher,
        45,
        &first.agent_id,
        "keep the runtime busy",
    )
    .await;

    let error = start_agent_turn_result_async(
        &mut second_dispatcher,
        46,
        &second.agent_id,
        "second active turn",
    )
    .await
    .expect_err("second active turn should reject");

    assert_eq!(error.code, ProtocolErrorCode::AgentBusy);
    let cancelled = cancel_agent_async(&mut first_dispatcher, 47, &first.agent_id).await;
    assert!(cancelled.cancelled);
    wait_for_terminal_event_async(&first_pushes, &started.run_id).await;
    server_handle.abort();
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_rejects_second_agent_invoke_until_cancelled() {
    let workspace = temp_workspace("dispatcher-agent-busy");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut dispatcher, "agent", Vec::new())
        .await
        .session_id;
    let started = invoke_agent_async(&mut dispatcher, 31, &session_id, "summarize current model")
        .await
        .expect("agent.invoke succeeds");
    assert_eq!(started.session_id, session_id);

    // Without Rig Agent configuration the worker finishes almost immediately. Wait for
    // done/error before verifying that a new invoke succeeds after the
    // previous run completes.
    wait_for_terminal_event_async(&pushes, &started.run_id).await;

    let restarted = invoke_agent_async(&mut dispatcher, 34, &session_id, "new run")
        .await
        .expect("restart succeeds");
    assert_eq!(restarted.session_id, session_id);

    wait_for_terminal_event_async(&pushes, &restarted.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_deduplicates_agent_invoke_by_client_request_id() {
    let workspace = temp_workspace("dispatcher-agent-invoke-idempotent");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut dispatcher, "agent idempotent", Vec::new())
        .await
        .session_id;

    let first = invoke_agent_with_client_request_id(
        &mut dispatcher,
        41,
        &session_id,
        "first prompt",
        Some("first-request"),
    )
    .await
    .expect("first invoke starts");
    let second = invoke_agent_with_client_request_id(
        &mut dispatcher,
        42,
        &session_id,
        "first prompt retry",
        Some("first-request"),
    )
    .await
    .expect("retry returns original run");

    assert_eq!(first.run_id, second.run_id);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_deduplicates_agent_invoke_by_client_request_id_across_dispatchers() {
    let workspace = temp_workspace("dispatcher-agent-invoke-cross-dispatcher");
    let _agent_env = unset_agent_environment();
    let (mut first_dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut first_dispatcher, "agent cross retry", Vec::new())
        .await
        .session_id;
    let first = invoke_agent_with_client_request_id(
        &mut first_dispatcher,
        41,
        &session_id,
        "first prompt",
        Some("first-request"),
    )
    .await
    .expect("first invoke starts");
    let (mut second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);

    let second = invoke_agent_with_client_request_id(
        &mut second_dispatcher,
        42,
        &session_id,
        "first prompt retry",
        Some("first-request"),
    )
    .await
    .expect("retry returns original run across dispatcher");

    assert_eq!(first.run_id, second.run_id);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(
        second_pushes.lock().expect("push buffer lock").is_empty(),
        "retry on a second dispatcher must not spawn a second worker"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_scopes_agent_invoke_request_id_to_chat_session() {
    let workspace = temp_workspace("dispatcher-agent-invoke-session-scope");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let first_session = create_chat_async(&mut dispatcher, "first agent", Vec::new())
        .await
        .session_id;
    let second_session = create_chat_async(&mut dispatcher, "second agent", Vec::new())
        .await
        .session_id;
    let _ = invoke_agent_with_client_request_id(
        &mut dispatcher,
        41,
        &first_session,
        "first prompt",
        Some("reused-request"),
    )
    .await
    .expect("first invoke starts");

    let error = invoke_agent_with_client_request_id(
        &mut dispatcher,
        42,
        &second_session,
        "second prompt",
        Some("reused-request"),
    )
    .await
    .expect_err("same request id on another chat must not return first run");

    assert_eq!(error.code, ProtocolErrorCode::AgentBusy);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_clears_agent_invoke_request_id_after_run_finishes() {
    let workspace = temp_workspace("dispatcher-agent-invoke-request-clear");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut dispatcher, "agent id clear", Vec::new())
        .await
        .session_id;
    let first = invoke_agent_with_client_request_id(
        &mut dispatcher,
        41,
        &session_id,
        "first prompt",
        Some("request-once"),
    )
    .await
    .expect("first invoke starts");
    wait_for_terminal_event_async(&pushes, &first.run_id).await;

    let second = invoke_agent_with_client_request_id(
        &mut dispatcher,
        42,
        &session_id,
        "new prompt",
        Some("request-once"),
    )
    .await
    .expect("request id is reusable after terminal run cleanup");

    assert_ne!(first.run_id, second.run_id);
    wait_for_terminal_event_async(&pushes, &second.run_id).await;
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_broadcasts_active_chat_changes_across_dispatchers() {
    let workspace = temp_workspace("dispatcher-chat-active-broadcast");
    let (mut first_dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let (second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let first = create_chat_async(&mut first_dispatcher, "first chat", Vec::new()).await;
    let second = create_chat_async(&mut first_dispatcher, "second chat", Vec::new()).await;
    second_pushes.lock().expect("push buffer lock").clear();

    let _ = dispatch_async(
        &mut first_dispatcher,
        43,
        ClientCommand::ChatHistory(ChatHistoryRequest {
            session_id: first.session_id.clone(),
            limit: Some(100),
        }),
    )
    .await
    .result
    .expect("chat.history succeeds");

    let active_chat_id = second_pushes
        .lock()
        .expect("push buffer lock")
        .iter()
        .rev()
        .find_map(|push| match &push.event {
            ServerPushEvent::ChatListChanged(response) => response.active_chat_id.clone(),
            _ => None,
        });
    assert_eq!(active_chat_id, Some(first.session_id));
    assert_ne!(active_chat_id, Some(second.session_id));
    drop(second_dispatcher);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_broadcasts_chat_summary_updates_from_agent_tools() {
    let workspace = temp_workspace("dispatcher-chat-summary-broadcast");
    let (mut dispatcher, _first_pushes) = dispatcher_with_pushes(&workspace);
    let (second_dispatcher, second_pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(&mut dispatcher, "summary chat", Vec::new()).await;
    second_pushes.lock().expect("push buffer lock").clear();

    ChatStore::new(workspace.to_path_buf())
        .update_summary(
            &created.session_id,
            ChatSummaryUpdate {
                summary: "current summary".into(),
                goal: "make progress".into(),
                related_files: vec![path_handle(["docs", "note.md"])],
                open_questions: Vec::new(),
            },
        )
        .await
        .expect("summary update succeeds");

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let pushed_events = second_pushes.lock().expect("push buffer lock");
    let chat_list_changed_count = pushed_events
        .iter()
        .filter(|push| matches!(push.event, ServerPushEvent::ChatListChanged(_)))
        .count();
    let related_files = pushed_events
        .iter()
        .rev()
        .find_map(|push| match &push.event {
            ServerPushEvent::ChatListChanged(response) => response
                .sessions
                .iter()
                .find(|session| session.session_id == created.session_id)
                .map(|session| session.related_files.clone()),
            _ => None,
        })
        .expect("chat list changed push");
    assert_eq!(chat_list_changed_count, 1);
    assert_eq!(related_files, vec![path_handle(["docs", "note.md"])]);
    drop(second_dispatcher);
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_persists_agent_error_message_when_llm_is_unavailable() {
    let workspace = temp_workspace("dispatcher-agent-llm-error-history");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut dispatcher, "agent error", Vec::new())
        .await
        .session_id;

    let started = invoke_agent_async(&mut dispatcher, 35, &session_id, "create a small part")
        .await
        .expect("agent starts");
    let error = wait_for_error_event_async(&pushes, &started.run_id).await;
    assert_eq!(
        error.error_type,
        app_server_protocol::AgentErrorType::LlmError
    );
    assert!(
        find_done_event(&pushes, &started.run_id).is_none(),
        "failed Agent run must not emit agent.done"
    );

    let history = read_chat_history_async(&mut dispatcher, &session_id).await;
    let assistant = history
        .iter()
        .find(|message| {
            message.role == ChatRole::Assistant
                && message.run_id.as_deref() == Some(started.run_id.as_str())
        })
        .expect("agent failure should be persisted as assistant history");
    assert!(assistant.content.contains("Agent run failed"));
    assert!(assistant.content.contains("Rig Agent is not configured"));

    let restarted = invoke_agent_async(&mut dispatcher, 136, &session_id, "retry after failure")
        .await
        .expect("failed run should release runtime");
    assert_ne!(started.run_id, restarted.run_id);
    let _ = wait_for_error_event_async(&pushes, &restarted.run_id).await;

    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_agent_invoke_accepts_plan_ref_without_confirmation_payload() {
    let workspace = temp_workspace("dispatcher-agent-plan-ref");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(workspace.join("parts/top_lid.py"), "old code\n").unwrap();
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat_async(&mut dispatcher, "agent execute", Vec::new())
        .await
        .session_id;
    let started = match dispatch_async(
        &mut dispatcher,
        36,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            client_request_id: None,
            prompt: "run plan".into(),
            mode: AgentMode::Agent,
            plan_ref: Some(path_handle(["plans", "2026050100-add-lid-vents"])),
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        }),
    )
    .await
    .result
    .expect("agent.invoke with plan_ref succeeds")
    {
        CommandSuccess::AgentStarted(response) => response,
        other => panic!("unexpected agent.invoke response: {other:?}"),
    };

    wait_for_terminal_event_async(&pushes, &started.run_id).await;
    assert_eq!(
        std::fs::read_to_string(workspace.join("parts/top_lid.py")).unwrap(),
        "old code\n"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_rejects_start_turn_when_chat_jsonl_path_is_invalid() {
    let workspace = temp_workspace("dispatcher-agent-start-invalid-chat-jsonl");
    let _agent_env = unset_agent_environment();
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(
        &mut dispatcher,
        "agent start invalid chat jsonl",
        Vec::new(),
    )
    .await;
    let index = read_chats_json(&workspace);
    let messages_path = index["chats"][0]["messages_path"]
        .as_str()
        .expect("messages path should be indexed");
    let absolute_messages_path = workspace.join(messages_path);
    std::fs::remove_file(&absolute_messages_path).expect("remove messages jsonl");
    std::fs::create_dir(&absolute_messages_path).expect("replace messages jsonl with directory");

    let error = start_agent_turn_result_async(&mut dispatcher, 49, &created.agent_id, "will fail")
        .await
        .expect_err("start turn should reject broken Chat JSONL");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(
        pushes
            .lock()
            .expect("push buffer lock")
            .iter()
            .all(|push| !matches!(
                push.event,
                ServerPushEvent::AgentDone(_) | ServerPushEvent::AgentError(_)
            )),
        "rejected start turn must not emit terminal Agent events"
    );
    cleanup_workspace(&workspace);
}

#[tokio::test]
async fn dispatcher_snapshot_reports_chat_jsonl_error_when_runtime_log_exists() {
    let workspace = temp_workspace("dispatcher-agent-terminal-write-fails-snapshot");
    let (config_path, server_handle) = hanging_agent_config(&workspace).await;
    let _agent_env = EnvGuard::set_many(vec![
        ("BUDN_AGENT_CONFIG", config_path.into_os_string()),
        ("BUDN_AGENT_OPENAI_API_KEY", "test-key".into()),
    ]);
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let created = create_chat_async(
        &mut dispatcher,
        "agent terminal write fails snapshot",
        Vec::new(),
    )
    .await;
    let started =
        start_agent_turn_async(&mut dispatcher, 50, &created.agent_id, "snapshot fail").await;
    let cancelled = cancel_agent_async(&mut dispatcher, 51, &created.agent_id).await;
    assert!(cancelled.cancelled);
    let done = wait_for_done_event_async(&pushes, &started.run_id).await;
    assert!(done.cancelled);

    let index = read_chats_json(&workspace);
    let messages_path = index["chats"][0]["messages_path"]
        .as_str()
        .expect("messages path should be indexed");
    let absolute_messages_path = workspace.join(messages_path);
    std::fs::remove_file(&absolute_messages_path).expect("remove messages jsonl");
    std::fs::create_dir(&absolute_messages_path).expect("replace messages jsonl with directory");

    let error = agent_snapshot_result_async(&mut dispatcher, 52, &created.agent_id)
        .await
        .expect_err("snapshot should report broken Chat JSONL");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    server_handle.abort();
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_cadquery_execute_rejects_outputs_outside_default_scope() {
    let workspace = temp_workspace("dispatcher-direct-execute-unconfirmed-output");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(workspace.join("parts/top_lid.py"), "old code\n").unwrap();
    let captured = workspace.join("captured-direct-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, true);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let response = dispatch(
        &mut dispatcher,
        39,
        ClientCommand::CadQueryExecute(confirmed_cadquery_request(path_handle([
            "parts",
            "top_lid.py",
        ]))),
    );

    let error = response
        .result
        .expect_err("unconfirmed output should reject direct execute");
    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    assert_eq!(
        std::fs::read_to_string(workspace.join("parts/top_lid.py")).unwrap(),
        "old code\n"
    );
    assert!(!workspace.join("outputs/top_lid.step").exists());
    assert!(!workspace.join("outputs/unconfirmed.step").exists());
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_cadquery_preview_rejects_export_formats_without_writing_outputs() {
    let workspace = temp_workspace("dispatcher-preview-export-reject");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(
        workspace.join("parts/top_lid.py"),
        "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n",
    )
    .unwrap();
    let captured = workspace.join("captured-preview-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, false);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let response = dispatch(
        &mut dispatcher,
        43,
        ClientCommand::CadQueryPreview(app_server_protocol::CadQueryPreviewRequest {
            target_path: path_handle(["parts", "top_lid.py"]),
            export_formats: vec![CadQueryExportFormat::Step],
            params_json: "{}".into(),
        }),
    );

    let error = response
        .result
        .expect_err("preview exports should require Execute confirmation");
    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    assert!(!captured.exists());
    assert!(!workspace.join("outputs/top_lid.step").exists());
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_cadquery_result_get_preserves_artifact_relation() {
    let workspace = temp_workspace("dispatcher-cadquery-artifact-relation");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(
        workspace.join("parts/top_lid.py"),
        "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n",
    )
    .unwrap();
    let captured = workspace.join("captured-preview-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, false);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    let response = dispatch(
        &mut dispatcher,
        44,
        ClientCommand::CadQueryPreview(app_server_protocol::CadQueryPreviewRequest {
            target_path: path_handle(["parts", "top_lid.py"]),
            export_formats: Vec::new(),
            params_json: "{}".into(),
        }),
    );
    match response.result.expect("preview should succeed") {
        CommandSuccess::CadQueryResultReady(ready) => {
            let relation = ready.artifact_relation.expect("ready relation");
            assert_eq!(relation.source_path, "parts/top_lid.py");
            assert_eq!(relation.exports[0].path, "outputs/top_lid.step");
        }
        other => panic!("unexpected preview response: {other:?}"),
    }

    let response = dispatch_cached_result_get(&mut dispatcher, 45, "cq_abc");
    match response.result.expect("result get should succeed") {
        CommandSuccess::CadQueryMesh(payload) => {
            let relation = payload.artifact_relation.expect("mesh relation");
            assert_eq!(relation.source_path, "parts/top_lid.py");
            assert_eq!(
                relation.exports[0].hash,
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            );
        }
        other => panic!("unexpected result get response: {other:?}"),
    }
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_plan_confirm_returns_deprecated_error_without_using_saved_plan() {
    let workspace = temp_workspace("dispatcher-plan-confirm-ref");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent plan", Vec::new()).session_id;
    append_saved_plan_result(&workspace, &session_id, "plan-run-1");
    pushes.lock().expect("push buffer lock").clear();
    let target_path = path_handle(["parts", "top_lid.py"]);
    let confirmation = AgentCadQueryConfirmation {
        request: confirmed_cadquery_request(target_path.clone()),
        plan_ref: None,
        affected_files: vec![target_path],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "top_lid.step"])],
    };

    let response = dispatch(
        &mut dispatcher,
        45,
        ClientCommand::AgentPlanConfirm(AgentPlanConfirmRequest {
            session_id,
            run_id: "plan-run-1".into(),
            confirmed_cadquery: confirmation,
        }),
    );

    let error = response
        .result
        .expect_err("deprecated plan confirm should reject");
    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    assert!(error.message.contains("已废弃"));
    assert!(pushes.lock().expect("push buffer lock").is_empty());
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_plan_confirm_rejects_before_scope_validation() {
    let workspace = temp_workspace("dispatcher-plan-confirm-scope");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent plan", Vec::new()).session_id;
    append_saved_plan_result(&workspace, &session_id, "plan-run-1");
    pushes.lock().expect("push buffer lock").clear();
    let target_path = path_handle(["parts", "top_lid.py"]);
    let confirmation = AgentCadQueryConfirmation {
        request: confirmed_cadquery_request(target_path.clone()),
        plan_ref: Some(path_handle(["plans", "add-lid-vents.md"])),
        affected_files: vec![target_path],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "other.step"])],
    };

    let response = dispatch(
        &mut dispatcher,
        46,
        ClientCommand::AgentPlanConfirm(AgentPlanConfirmRequest {
            session_id,
            run_id: "plan-run-1".into(),
            confirmed_cadquery: confirmation,
        }),
    );

    let error = response
        .result
        .expect_err("deprecated plan confirm should reject");
    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    assert!(error.message.contains("已废弃"));
    assert!(pushes.lock().expect("push buffer lock").is_empty());
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_cadquery_preview_rejects_outputs_outside_default_scope() {
    let workspace = temp_workspace("dispatcher-direct-preview-unconfirmed-output");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(
        workspace.join("parts/top_lid.py"),
        "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n",
    )
    .unwrap();
    let captured = workspace.join("captured-preview-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, true);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);
    let response = dispatch(
        &mut dispatcher,
        41,
        ClientCommand::CadQueryPreview(app_server_protocol::CadQueryPreviewRequest {
            target_path: path_handle(["parts", "top_lid.py"]),
            export_formats: vec![CadQueryExportFormat::Step],
            params_json: "{}".into(),
        }),
    );

    let error = response
        .result
        .expect_err("unconfirmed output should reject direct preview");
    assert_eq!(error.code, ProtocolErrorCode::InvalidCommand);
    assert!(!workspace.join("outputs/top_lid.step").exists());
    assert!(!workspace.join("outputs/unconfirmed.step").exists());
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_cadquery_result_cache_evicts_oldest_entries() {
    let workspace = temp_workspace("dispatcher-cadquery-cache-limit");
    create_cache_limit_part_files(&workspace, 9);
    let runner = fake_variable_result_cadquery_runner(&workspace);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, _pushes) = dispatcher_with_pushes(&workspace);

    for index in 0..9 {
        assert_cache_preview_result(&mut dispatcher, 50 + index as u64, index);
    }

    assert_cached_result_missing(&mut dispatcher, 70, "cq_part_0");
    assert_cached_result_present(&mut dispatcher, 71, "cq_part_8");
    cleanup_workspace(&workspace);
}

fn create_cache_limit_part_files(workspace: &std::path::Path, count: usize) {
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    for index in 0..count {
        std::fs::write(
            workspace.join(format!("parts/part_{index}.py")),
            "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n",
        )
        .unwrap();
    }
}

fn assert_cache_preview_result(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    index: usize,
) {
    let response = dispatch(
        dispatcher,
        request_id,
        ClientCommand::CadQueryPreview(app_server_protocol::CadQueryPreviewRequest {
            target_path: PathHandle::new(
                WorkspaceId::new("workspace"),
                ["parts", &format!("part_{index}.py")],
            )
            .expect("part path"),
            export_formats: Vec::new(),
            params_json: "{}".into(),
        }),
    );
    match response.result.expect("preview should succeed") {
        CommandSuccess::CadQueryResultReady(ready) => {
            assert_eq!(ready.result_id, format!("cq_part_{index}"));
        }
        other => panic!("unexpected preview response: {other:?}"),
    }
}

fn assert_cached_result_missing(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    result_id: &str,
) {
    let error = dispatch_cached_result_get(dispatcher, request_id, result_id)
        .result
        .expect_err("result should be missing");
    assert_eq!(error.code, ProtocolErrorCode::NotFound);
}

fn assert_cached_result_present(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    result_id: &str,
) {
    let response = dispatch_cached_result_get(dispatcher, request_id, result_id)
        .result
        .expect("result should remain");
    assert!(matches!(response, CommandSuccess::CadQueryMesh(_)));
}

fn dispatch_cached_result_get(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    result_id: &str,
) -> app_server_protocol::ServerResponseEnvelope {
    dispatch(
        dispatcher,
        request_id,
        ClientCommand::CadQueryResultGet(app_server_protocol::CadQueryResultGetRequest {
            result_id: result_id.into(),
        }),
    )
}

fn dispatcher_with_pushes(
    workspace: &std::path::Path,
) -> (HostRequestDispatcher, Arc<Mutex<Vec<ServerPushEnvelope>>>) {
    let pushes = Arc::new(Mutex::new(Vec::<ServerPushEnvelope>::new()));
    let push_sink = {
        let pushes = Arc::clone(&pushes);
        Arc::new(move |push: ServerPushEnvelope| {
            pushes.lock().expect("push buffer lock").push(push);
        })
    };
    let dispatcher = HostRequestDispatcher::with_session_token(
        Some(workspace.to_path_buf()),
        SessionToken("session-1".into()),
        Vec::new(),
        push_sink,
    );
    (dispatcher, pushes)
}

fn create_chat(
    dispatcher: &mut HostRequestDispatcher,
    title: &str,
    related_files: Vec<PathHandle>,
) -> app_server_protocol::ChatCreatedResponse {
    match dispatch(
        dispatcher,
        20,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: title.into(),
            goal: Some("lid iteration".into()),
            related_files,
            client_request_id: Some(format!("create-{title}")),
            initial_user_message: Some(format!("Start {title}")),
            requested_model: None,
            initial_turn: None,
        }),
    )
    .result
    .expect("chat.create succeeds")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    }
}

async fn create_chat_async(
    dispatcher: &mut HostRequestDispatcher,
    title: &str,
    related_files: Vec<PathHandle>,
) -> app_server_protocol::ChatCreatedResponse {
    match dispatch_async(
        dispatcher,
        20,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: title.into(),
            goal: Some("lid iteration".into()),
            related_files,
            client_request_id: Some(format!("create-{title}")),
            initial_user_message: Some(format!("Start {title}")),
            requested_model: None,
            initial_turn: None,
        }),
    )
    .await
    .result
    .expect("chat.create succeeds")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    }
}

async fn create_chat_with_model_async(
    dispatcher: &mut HostRequestDispatcher,
    title: &str,
    model: BoundAgentModel,
) -> app_server_protocol::ChatCreatedResponse {
    match dispatch_async(
        dispatcher,
        20,
        ClientCommand::ChatCreate(ChatCreateRequest {
            title: title.into(),
            goal: Some("lid iteration".into()),
            related_files: Vec::new(),
            client_request_id: Some(format!("create-{title}")),
            initial_user_message: Some(format!("Start {title}")),
            requested_model: Some(model),
            initial_turn: None,
        }),
    )
    .await
    .result
    .expect("chat.create succeeds")
    {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    }
}

fn chat_created_from_response(
    response: app_server_protocol::ServerResponseEnvelope,
) -> app_server_protocol::ChatCreatedResponse {
    match response.result.expect("chat.create succeeds") {
        CommandSuccess::ChatCreated(response) => response,
        other => panic!("unexpected chat.create response: {other:?}"),
    }
}

fn send_chat(
    dispatcher: &mut HostRequestDispatcher,
    session_id: &ChatSessionId,
    related_files: Vec<PathHandle>,
) -> app_server_protocol::ChatAckResponse {
    match dispatch(
        dispatcher,
        21,
        ClientCommand::ChatSend(ChatSendRequest {
            session_id: session_id.clone(),
            content: "make the lid taller".into(),
            related_files,
            client_request_id: None,
        }),
    )
    .result
    .expect("chat.send succeeds")
    {
        CommandSuccess::ChatAck(response) => response,
        other => panic!("unexpected chat.send response: {other:?}"),
    }
}

fn list_chats(
    dispatcher: &mut HostRequestDispatcher,
    include_archived: bool,
) -> Vec<app_server_protocol::ChatSessionSummary> {
    match dispatch(
        dispatcher,
        22,
        ClientCommand::ChatList(ChatListRequest { include_archived }),
    )
    .result
    .expect("chat.list succeeds")
    {
        CommandSuccess::ChatList(response) => response.sessions,
        other => panic!("unexpected chat.list response: {other:?}"),
    }
}

async fn list_chats_async(
    dispatcher: &mut HostRequestDispatcher,
    include_archived: bool,
) -> Vec<app_server_protocol::ChatSessionSummary> {
    match dispatch_async(
        dispatcher,
        22,
        ClientCommand::ChatList(ChatListRequest { include_archived }),
    )
    .await
    .result
    .expect("chat.list succeeds")
    {
        CommandSuccess::ChatList(response) => response.sessions,
        other => panic!("unexpected chat.list response: {other:?}"),
    }
}

fn read_chat_history(
    dispatcher: &mut HostRequestDispatcher,
    session_id: &ChatSessionId,
) -> Vec<app_server_protocol::ChatMessageRecord> {
    match dispatch(
        dispatcher,
        23,
        ClientCommand::ChatHistory(ChatHistoryRequest {
            session_id: session_id.clone(),
            limit: Some(10),
        }),
    )
    .result
    .expect("chat.history succeeds")
    {
        CommandSuccess::ChatHistory(response) => response.messages,
        other => panic!("unexpected chat.history response: {other:?}"),
    }
}

async fn read_chat_history_async(
    dispatcher: &mut HostRequestDispatcher,
    session_id: &ChatSessionId,
) -> Vec<app_server_protocol::ChatMessageRecord> {
    match dispatch_async(
        dispatcher,
        23,
        ClientCommand::ChatHistory(ChatHistoryRequest {
            session_id: session_id.clone(),
            limit: Some(10),
        }),
    )
    .await
    .result
    .expect("chat.history succeeds")
    {
        CommandSuccess::ChatHistory(response) => response.messages,
        other => panic!("unexpected chat.history response: {other:?}"),
    }
}

fn update_selection(
    dispatcher: &mut HostRequestDispatcher,
) -> app_server_protocol::SelectionUpdateResponse {
    let selection = SelectionRef {
        kind: SelectionKind::Face,
        ref_text: "@face[top_lid:f_0]".into(),
        owner_ref_text: Some("@part[top_lid]".into()),
        owner_object_kind: Some(app_server_protocol::CadQueryObjectKind::Part),
        instance_path: None,
        candidate_feature_ref: Some("@feature[top_lid.top_surface]".into()),
        build_id: Some("sha256:build".into()),
        result_id: Some("cq_1".into()),
        ambiguous: false,
    };
    match dispatch(
        dispatcher,
        24,
        ClientCommand::SelectionUpdate(SelectionUpdateRequest {
            selections: vec![selection],
            active_index: Some(0),
        }),
    )
    .result
    .expect("selection.update succeeds")
    {
        CommandSuccess::SelectionUpdated(response) => response,
        other => panic!("unexpected selection.update response: {other:?}"),
    }
}

fn archive_chat(dispatcher: &mut HostRequestDispatcher, session_id: &ChatSessionId) {
    match dispatch(
        dispatcher,
        25,
        ClientCommand::ChatArchive(ChatArchiveRequest {
            session_id: session_id.clone(),
        }),
    )
    .result
    .expect("chat.archive succeeds")
    {
        CommandSuccess::ChatArchived(response) => assert_eq!(response.session_id, *session_id),
        other => panic!("unexpected chat.archive response: {other:?}"),
    }
}

fn read_chats_json(workspace: &std::path::Path) -> Value {
    let content = std::fs::read_to_string(workspace.join("chats.json")).expect("read chats.json");
    serde_json::from_str(&content).expect("parse chats.json")
}

fn bound_agent_model() -> BoundAgentModel {
    BoundAgentModel {
        provider_id: "openai".into(),
        provider_type: AgentProviderType::OpenAiResponses,
        model_id: "gpt-5.2".into(),
        reasoning_effort: Some("high".into()),
        service_label: Some("flex".into()),
    }
}

fn append_saved_plan_result(workspace: &std::path::Path, session_id: &ChatSessionId, run_id: &str) {
    let result_json = format!(
        concat!(
            "{{\"status\":\"ok\",",
            "\"tool\":\"save_cad_plan\",",
            "\"run_id\":\"{}\",",
            "\"plan_ref\":\"plans/add-lid-vents.md\",",
            "\"target_path\":\"parts/top_lid.py\",",
            "\"affected_files\":[\"parts/top_lid.py\"],",
            "\"new_files\":[],",
            "\"export_targets\":[\"outputs/top_lid.step\"],",
            "\"summary\":\"Add lid vents\"}}"
        ),
        run_id
    );
    tokio::runtime::Runtime::new()
        .expect("test runtime should build")
        .block_on(ChatStore::new(workspace.to_path_buf()).append_tool_result(
            session_id,
            "agent tool completed",
            ChatToolResultRecord {
                tool_call_id: "call-save-plan".into(),
                tool_name: "save_cad_plan".into(),
                result_json,
            },
            None,
        ))
        .expect("saved plan tool result");
}

async fn invoke_agent_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
    prompt: &str,
) -> Result<app_server_protocol::AgentStartedResponse, app_server_protocol::ProtocolError> {
    invoke_agent_with_client_request_id(dispatcher, request_id, session_id, prompt, None).await
}

async fn invoke_agent_with_client_request_id(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
    prompt: &str,
    client_request_id: Option<&str>,
) -> Result<app_server_protocol::AgentStartedResponse, app_server_protocol::ProtocolError> {
    match dispatch_async(
        dispatcher,
        request_id,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            client_request_id: client_request_id.map(str::to_owned),
            prompt: prompt.into(),
            mode: AgentMode::Agent,
            plan_ref: None,
            context_refs: Vec::new(),
            provider_id: None,
            model_id: None,
            reasoning_effort: None,
            service_label: None,
        }),
    )
    .await
    .result?
    {
        CommandSuccess::AgentStarted(response) => Ok(response),
        other => panic!("unexpected agent.invoke response: {other:?}"),
    }
}

async fn start_agent_turn_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
    prompt: &str,
) -> app_server_protocol::AgentStartedResponse {
    start_agent_turn_result_async(dispatcher, request_id, agent_id, prompt)
        .await
        .expect("agent.start_turn succeeds")
}

async fn start_agent_turn_result_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
    prompt: &str,
) -> Result<app_server_protocol::AgentStartedResponse, app_server_protocol::ProtocolError> {
    Ok(
        match dispatch_async(
            dispatcher,
            request_id,
            ClientCommand::AgentStartTurn(AgentStartTurnRequest {
                agent_id: agent_id.clone(),
                client_request_id: Some(format!("start-{request_id}")),
                prompt: prompt.into(),
                mode: AgentMode::Agent,
                plan_ref: None,
                context_refs: Vec::new(),
            }),
        )
        .await
        .result?
        {
            CommandSuccess::AgentStarted(response) => response,
            other => panic!("unexpected agent.start_turn response: {other:?}"),
        },
    )
}

async fn subscribe_agent_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
) -> app_server_protocol::AgentSubscribeResponse {
    subscribe_agent_with_cursor_async(dispatcher, request_id, agent_id, None).await
}

async fn subscribe_agent_with_cursor_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
    since_event_id: Option<AgentEventId>,
) -> app_server_protocol::AgentSubscribeResponse {
    match dispatch_async(
        dispatcher,
        request_id,
        ClientCommand::AgentSubscribe(AgentSubscribeRequest {
            agent_id: agent_id.clone(),
            since_event_id,
        }),
    )
    .await
    .result
    .expect("agent.subscribe succeeds")
    {
        CommandSuccess::AgentSubscribed(response) => response,
        other => panic!("unexpected agent.subscribe response: {other:?}"),
    }
}

async fn agent_snapshot_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
) -> app_server_protocol::AgentSnapshotResponse {
    match agent_snapshot_result_async(dispatcher, request_id, agent_id)
        .await
        .expect("agent.snapshot succeeds")
    {
        CommandSuccess::AgentSnapshot(response) => response,
        other => panic!("unexpected agent.snapshot response: {other:?}"),
    }
}

async fn agent_snapshot_result_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
) -> Result<CommandSuccess, app_server_protocol::ProtocolError> {
    dispatch_async(
        dispatcher,
        request_id,
        ClientCommand::AgentSnapshot(AgentSnapshotRequest {
            agent_id: agent_id.clone(),
            since_event_id: None,
        }),
    )
    .await
    .result
}

async fn cancel_agent_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    agent_id: &app_server_protocol::AgentId,
) -> app_server_protocol::AgentCancelledResponse {
    match dispatch_async(
        dispatcher,
        request_id,
        ClientCommand::AgentCancel(AgentCancelRequest {
            agent_id: agent_id.clone(),
        }),
    )
    .await
    .result
    .expect("agent.cancel succeeds")
    {
        CommandSuccess::AgentCancelled(response) => response,
        other => panic!("unexpected agent.cancel response: {other:?}"),
    }
}

fn confirmed_cadquery_request(target_path: PathHandle) -> CadQueryExecuteRequest {
    CadQueryExecuteRequest {
        target_path,
        target_type: CadQueryObjectKind::Part,
        code: "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n".into(),
        export_formats: vec![CadQueryExportFormat::Step],
        params_json: "{}".into(),
    }
}

fn agent_event_record(
    event_id: u64,
    agent_id: &app_server_protocol::AgentId,
    turn_id: &AgentTurnId,
    payload: AgentEventPayload,
) -> AgentEventRecord {
    AgentEventRecord {
        event_id: AgentEventId(event_id),
        agent_id: agent_id.clone(),
        turn_id: Some(turn_id.clone()),
        ts_ms: 100 + event_id,
        payload,
    }
}

fn find_done_event(
    pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>,
    run_id: &str,
) -> Option<AgentDoneEvent> {
    pushes
        .lock()
        .expect("push buffer lock")
        .iter()
        .find_map(|push| match &push.event {
            ServerPushEvent::AgentDone(event) if event.run_id == run_id => Some(event.clone()),
            _ => None,
        })
}

fn find_error_event(
    pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>,
    run_id: &str,
) -> Option<AgentErrorEvent> {
    pushes
        .lock()
        .expect("push buffer lock")
        .iter()
        .find_map(|push| match &push.event {
            ServerPushEvent::AgentError(event) if event.run_id.as_deref() == Some(run_id) => {
                Some(event.clone())
            }
            _ => None,
        })
}

fn dispatch(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    command: ClientCommand,
) -> app_server_protocol::ServerResponseEnvelope {
    tokio::runtime::Runtime::new()
        .expect("test runtime should build")
        .block_on(dispatcher.dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(request_id),
            command,
        }))
}

async fn dispatch_async(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    command: ClientCommand,
) -> app_server_protocol::ServerResponseEnvelope {
    dispatcher
        .dispatch_envelope(ClientRequestEnvelope {
            request_id: RequestId(request_id),
            command,
        })
        .await
}

async fn dispatch_agent_model_command(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    command: ClientCommand,
) -> app_server_protocol::AgentModelRegistryResponse {
    match dispatch_async(dispatcher, request_id, command)
        .await
        .result
        .expect("agent model command succeeds")
    {
        CommandSuccess::AgentModelRegistry(response) => response,
        other => panic!("unexpected agent model response: {other:?}"),
    }
}

fn path_handle<const N: usize>(segments: [&str; N]) -> PathHandle {
    PathHandle::new(WorkspaceId::new("workspace"), segments).expect("path handle")
}

async fn wait_for_terminal_event_async(pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>, run_id: &str) {
    for _ in 0..250 {
        if find_done_event(pushes, run_id).is_some() || find_error_event(pushes, run_id).is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("agent terminal event not observed for {run_id}");
}

async fn wait_for_done_event_async(
    pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>,
    run_id: &str,
) -> AgentDoneEvent {
    for _ in 0..250 {
        if let Some(done) = find_done_event(pushes, run_id) {
            return done;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("agent.done not observed for {run_id}");
}

async fn wait_for_agent_event_records_async(
    path: &std::path::Path,
    min_records: usize,
) -> Vec<app_server_protocol::AgentEventRecord> {
    for _ in 0..200 {
        if let Ok(event_log) = std::fs::read_to_string(path) {
            let records = event_log
                .lines()
                .map(|line| serde_json::from_str::<app_server_protocol::AgentEventRecord>(line))
                .collect::<Result<Vec<_>, _>>()
                .expect("event log lines should decode as AgentEventRecord");
            if records.len() >= min_records {
                return records;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for Agent event log records");
}

async fn wait_for_error_event_async(
    pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>,
    run_id: &str,
) -> AgentErrorEvent {
    for _ in 0..250 {
        if let Some(error) = find_error_event(pushes, run_id) {
            return error;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("agent.error not observed for {run_id}");
}

fn handshake_request() -> CapabilityHandshakeRequest {
    handshake_request_with_version(ProtocolVersionRange::new(
        CURRENT_PROTOCOL_VERSION,
        CURRENT_PROTOCOL_VERSION,
    ))
}

fn handshake_request_with_version(
    protocol_version: ProtocolVersionRange,
) -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "dispatcher-test".into(),
            platform: ClientPlatform::Web,
            protocol_version,
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn temp_workspace(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{label}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("README.md"), "hello dispatcher").unwrap();

    let triangles = [stl_io::Triangle {
        normal: stl_io::Normal::new([0.0, 0.0, 1.0]),
        vertices: [
            stl_io::Vertex::new([0.0, 0.0, 0.0]),
            stl_io::Vertex::new([1.0, 0.0, 0.0]),
            stl_io::Vertex::new([0.0, 1.0, 0.0]),
        ],
    }];
    let mut bytes = Vec::new();
    stl_io::write_stl(&mut bytes, triangles.iter()).unwrap();
    std::fs::write(root.join("model.stl"), bytes).unwrap();
    root
}

fn cleanup_workspace(root: &std::path::Path) {
    let _ = std::fs::remove_dir_all(root);
}

fn agent_model_registry_config() -> &'static str {
    r#"active_provider = "openai"
active_model = "gpt-5.2"

[defaults]
discover_models = false

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
discover_models = false

[[providers.models]]
id = "gpt-5.2"
reasoning_effort = "high"
service_label = "default"

[[providers.models]]
id = "gpt-5-mini"
native_web_search = true
web_search_supported = false

[[providers]]
id = "openai_completions"
kind = "openai_completions"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
discover_models = false

[[providers.models]]
id = "gpt-4o"
reasoning_effort = "high"
service_label = "default"
native_web_search = true
web_search_supported = true
"#
}

fn agent_model_registry_config_without_bound_model() -> &'static str {
    r#"active_provider = "openai"
active_model = "gpt-5-mini"

[defaults]
discover_models = false

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
discover_models = false

[[providers.models]]
id = "gpt-5-mini"
"#
}

async fn hanging_agent_config(
    workspace: &std::path::Path,
) -> (std::path::PathBuf, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hanging agent server");
    let addr = listener.local_addr().expect("read hanging server addr");
    let handle = tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let _socket = socket;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            });
        }
    });
    let config_path = workspace.join("agents.toml");
    std::fs::write(
        &config_path,
        hanging_agent_registry_config(&addr.to_string()),
    )
    .expect("write hanging agent config");
    (config_path, handle)
}

fn hanging_agent_registry_config(addr: &str) -> String {
    format!(
        r#"active_provider = "openai"
active_model = "gpt-5.2"

[defaults]
discover_models = false
timeout_secs = 30

[[providers]]
id = "openai"
kind = "openai_responses"
api_key_env = "BUDN_AGENT_OPENAI_API_KEY"
base_url = "http://{addr}#"
discover_models = false

[[providers.models]]
id = "gpt-5.2"
"#
    )
}

fn fake_capturing_cadquery_runner(
    root: &std::path::Path,
    capture_path: &std::path::Path,
    write_extra_output: bool,
) -> std::path::PathBuf {
    let runner = root.join("fake-capturing-cadquery-runner.sh");
    let extra_output = if write_extra_output {
        "  printf 'extra\\n' > \"$out/unconfirmed.step\"\n"
    } else {
        ""
    };
    std::fs::write(
        &runner,
        format!(
            "#!/bin/sh\nproject=''\nscript=''\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --contract-file) cat <<'JSON'\n{{\"status\":\"success\",\"error\":null,\"error_type\":null,\"contract\":{{\"has_model_description\":true,\"syntax_error\":null}}}}\nJSON\n      exit 0 ;;\n    --project-root) shift; project=\"$1\" ;;\n    --script) shift; script=\"$1\" ;;\n    --output-dir) shift; out=\"$1\" ;;\n  esac\n  shift\ndone\nif [ -n \"$project\" ] && [ -n \"$script\" ]; then\n  cp \"$project/$script\" '{}'\nfi\nif [ -n \"$out\" ]; then\n  mkdir -p \"$out\"\n  printf 'artifact\\n' > \"$out/top_lid.step\"\n{}fi\ncat <<'JSON'\n{}\nJSON\n",
            capture_path.display(),
            extra_output,
            cadquery_success_json()
        ),
    )
    .expect("write fake cadquery runner");
    make_executable(&runner);
    runner
}

fn fake_variable_result_cadquery_runner(root: &std::path::Path) -> std::path::PathBuf {
    let runner = root.join("fake-variable-result-cadquery-runner.sh");
    let json = cadquery_success_json();
    std::fs::write(
        &runner,
        format!(
            "#!/bin/sh\nscript=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '--script' ]; then\n    shift\n    script=\"$1\"\n  fi\n  shift\ndone\nstem=$(basename \"$script\" .py)\nsed \"s/cq_abc/cq_${{stem}}/g\" <<'JSON'\n{json}\nJSON\n"
        ),
    )
    .expect("write fake cadquery runner");
    make_executable(&runner);
    runner
}

fn cadquery_success_json() -> &'static str {
    r#"{
      "status":"success",
      "result_id":"cq_abc",
      "build_id":"sha256:7d7152e43de9e062366d794b6319a4d3a90e6972ad00f940179245833d410403",
      "unit":"millimeter",
      "root_ref_text":"@part[top_lid]",
      "root_object_kind":"part",
      "parts":[{
        "name":"top_lid",
        "object_kind":"part",
        "ref_text":"@part[top_lid]",
        "instance_path":null,
        "transform":null,
        "mesh":{
          "faces":[{
            "face_idx":0,
            "positions":[0,0,0,1,0,0,0,1,0],
            "normals":[0,0,1,0,0,1,0,0,1],
            "features":["top_surface"],
            "ambiguous":false
          }],
          "edges":[{"edge_idx":0,"polyline":[0,0,0,1,0,0],"adjacent_faces":[0]}],
          "vertices":[{"vertex_idx":0,"position":[0,0,0],"adjacent_edges":[0]}]
        },
        "feature_map":{"top_surface":{"face_indices":[0],"selector":"faces(\">Z\")"}}
      }],
      "exports":{"step":"outputs/top_lid.step"},
      "metadata":{"bounding_box":{"min":[0,0,0],"max":[1,1,1]}},
      "manifest":{
        "source_path":"parts/top_lid.py",
        "source_hash":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "params":{},
        "params_hash":"sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        "dependencies":[{"path":"parts/top_lid.py","hash":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
        "deps_hash":"sha256:486f81788f9250ca562a11da138c690884aebda032157fe8fa66e2ad952ebfdc",
        "export_hashes":{"step":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
      }
    }"#
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .expect("runner metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("runner permissions");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn unset_agent_environment() -> EnvGuard {
    EnvGuard::unset_many(&[
        "BUDN_AGENT_CONFIG",
        "BUDN_AGENT_OPENAI_API_KEY",
        "OPENAI_API_KEY",
        "BUDN_AGENT_MODEL",
        "BUDN_AGENT_REASONING_EFFORT",
        "BUDN_AGENT_MAX_TOKENS",
        "BUDN_AGENT_TIMEOUT_SECS",
        "BUDN_AGENT_MAX_TOKENS",
        "BUDN_AGENT_TEMPERATURE",
        "CADQUERY_RUNNER_PYTHON",
    ])
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self {
            previous: vec![(key, previous)],
            _lock: lock,
        }
    }

    fn set_many(entries: Vec<(&'static str, std::ffi::OsString)>) -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = entries
            .into_iter()
            .map(|(key, value)| {
                let previous = std::env::var_os(key);
                unsafe {
                    std::env::set_var(key, value);
                }
                (key, previous)
            })
            .collect();
        Self {
            previous,
            _lock: lock,
        }
    }

    fn unset_many(keys: &[&'static str]) -> Self {
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
