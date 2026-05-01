use std::sync::{Arc, Mutex, OnceLock};

use app_server_core::ChatStore;
use app_server_host::HostRequestDispatcher;
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentDoneEvent, AgentInvokeRequest, AgentMode,
    AgentModelParamsUpdateRequest, AgentModelSelectRequest, AgentPlanConfirmRequest,
    CURRENT_PROTOCOL_VERSION, CadQueryExecuteRequest, CadQueryExportFormat, CadQueryObjectKind,
    CapabilityHandshakeRequest, ChatArchiveRequest, ChatCreateRequest, ChatHistoryRequest,
    ChatListRequest, ChatRole, ChatSendRequest, ChatSessionId, ChatToolResultRecord,
    ClientCapabilities, ClientCommand, ClientPlatform, ClientRequestEnvelope, CommandSuccess,
    ExportFormat, ExportRunRequest, HostLocalPath, PathHandle, PreviewArtifact, PreviewRequest,
    PreviewRequestKind, ProtocolErrorCode, ProtocolVersionRange, RequestId, SelectionKind,
    SelectionRef, SelectionUpdateRequest, ServerPushEnvelope, ServerPushEvent, SessionToken,
    WorkspaceId, WorkspaceListRequest, web_file_read_capability,
};

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

    let selected = dispatch_agent_model_command(
        &mut dispatcher,
        11,
        ClientCommand::AgentModelSelect(AgentModelSelectRequest {
            provider_id: "openai".into(),
            model_id: "gpt-5-mini".into(),
        }),
    )
    .await;
    assert_eq!(selected.active_model_id, "gpt-5-mini");

    let updated = dispatch_agent_model_command(
        &mut dispatcher,
        12,
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
        13,
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
    assert_eq!(reasoning_only.active_service_label.as_deref(), Some("flex"));

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
    assert_eq!(
        handshake_registry.active_service_label.as_deref(),
        Some("flex")
    );
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

    assert_eq!(created.session_id, ChatSessionId("main-chat".into()));
    let ack = send_chat(&mut dispatcher, &created.session_id, vec![related.clone()]);
    assert_eq!(ack.session_id, created.session_id);
    let sessions = list_chats(&mut dispatcher, false);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].message_count, 2);
    assert_eq!(sessions[0].related_files, vec![related.clone()]);
    let history = read_chat_history(&mut dispatcher, &created.session_id);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, ChatRole::Meta);
    assert_eq!(history[1].content, "make the lid taller");
    let updated = update_selection(&mut dispatcher);
    assert_eq!(updated.accepted_count, 1);
    archive_chat(&mut dispatcher, &created.session_id);
    assert!(workspace.join("chats/archived/main-chat.jsonl").is_file());
    let active = list_chats(&mut dispatcher, false);
    assert!(active.is_empty());
    assert!(pushes.lock().expect("push buffer lock").is_empty());
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
    wait_for_done_async(&pushes, &started.run_id).await;

    let restarted = invoke_agent_async(&mut dispatcher, 34, &session_id, "new run")
        .await
        .expect("restart succeeds");
    assert_eq!(restarted.session_id, session_id);

    wait_for_done_async(&pushes, &restarted.run_id).await;
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
    wait_for_done_async(&pushes, &started.run_id).await;

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

    wait_for_done_async(&pushes, &started.run_id).await;
    assert_eq!(
        std::fs::read_to_string(workspace.join("parts/top_lid.py")).unwrap(),
        "old code\n"
    );
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
    match dispatch_async(
        dispatcher,
        request_id,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
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

fn confirmed_cadquery_request(target_path: PathHandle) -> CadQueryExecuteRequest {
    CadQueryExecuteRequest {
        target_path,
        target_type: CadQueryObjectKind::Part,
        code: "import cadquery as cq\n\ndef build(params=None):\n    return cq.Workplane('XY').box(1, 1, 1)\n".into(),
        export_formats: vec![CadQueryExportFormat::Step],
        params_json: "{}".into(),
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

async fn wait_for_done_async(pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>, run_id: &str) {
    for _ in 0..250 {
        if find_done_event(pushes, run_id).is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("agent.done not observed for {run_id}");
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
"#
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
