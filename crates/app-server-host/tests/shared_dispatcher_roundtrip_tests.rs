use std::sync::{Arc, Mutex, OnceLock};

use app_server_host::HostRequestDispatcher;
use app_server_protocol::{
    AgentCadQueryConfirmation, AgentCancelRequest, AgentDoneEvent, AgentErrorEvent, AgentErrorType,
    AgentInvokeRequest, AgentOperationLevel, AgentToolResultEvent, CadQueryExecuteRequest,
    CadQueryExportFormat, CadQueryObjectKind, CapabilityHandshakeRequest, ChatArchiveRequest,
    ChatCreateRequest, ChatHistoryRequest, ChatListRequest, ChatRole, ChatSendRequest,
    ChatSessionId, ClientCapabilities, ClientCommand, ClientPlatform, ClientRequestEnvelope,
    CommandSuccess, ExportFormat, ExportRunRequest, HostLocalPath, PathHandle, PreviewArtifact,
    PreviewRequest, PreviewRequestKind, ProtocolErrorCode, ProtocolVersionRange, RequestId,
    SelectionKind, SelectionRef, SelectionUpdateRequest, ServerPushEnvelope, ServerPushEvent,
    SessionToken, WorkspaceId, WorkspaceListRequest, web_file_read_capability,
};

#[test]
fn shared_dispatcher_roundtrips_handshake_workspace_file_and_preview() {
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

    let handshake = dispatcher.handshake(handshake_request());
    assert_eq!(handshake.session_token.0, "session-1");

    let current = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(1),
        command: ClientCommand::WorkspaceCurrent,
    });
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

    let list = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(2),
        command: ClientCommand::WorkspaceList(WorkspaceListRequest { directory: None }),
    });
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

    let file_read = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(3),
        command: ClientCommand::FileRead(app_server_protocol::FileReadRequest { path: readme }),
    });
    match file_read.result.expect("file read should succeed") {
        CommandSuccess::FileRead(response) => match response.contents {
            app_server_protocol::FileReadContents::Utf8Text(text) => {
                assert!(text.contains("hello"))
            }
            other => panic!("unexpected file contents: {other:?}"),
        },
        other => panic!("unexpected file read response: {other:?}"),
    }

    let preview = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(4),
        command: ClientCommand::PreviewRequest(PreviewRequest {
            source: model,
            defines: vec![],
            kind: PreviewRequestKind::GeometryArtifact,
            configured_openscad_path: None,
        }),
    });
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

    let response = dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(10),
        command: ClientCommand::ExportRun(ExportRunRequest {
            configured_openscad_path: Some(HostLocalPath::new("/bin/false").unwrap()),
            configured_slicers: Vec::new(),
            source,
            defines: Vec::new(),
            output_path,
            format: ExportFormat::ThreeMf,
            slicer_name: None,
        }),
    });

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

#[test]
fn dispatcher_rejects_second_agent_invoke_until_cancelled() {
    let workspace = temp_workspace("dispatcher-agent-busy");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent", Vec::new()).session_id;
    let started = invoke_agent(&mut dispatcher, 31, &session_id, "summarize current model")
        .expect("agent.invoke succeeds");
    assert_eq!(started.session_id, session_id);

    let busy = invoke_agent_error(&mut dispatcher, 32, &session_id);
    assert_eq!(busy.code, ProtocolErrorCode::AgentBusy);
    let cancelled = cancel_agent(&mut dispatcher, &started.run_id);
    assert_eq!(cancelled.run_id, Some(started.run_id.clone()));

    let still_busy = invoke_agent_error(&mut dispatcher, 40, &session_id);
    assert_eq!(still_busy.code, ProtocolErrorCode::AgentBusy);
    wait_for_done(&pushes, &started.run_id);
    let done = find_done_event(&pushes, &started.run_id).expect("cancel should push agent.done");
    assert_eq!(done.session_id, session_id.clone());
    assert_eq!(done.run_id, started.run_id);
    assert!(done.cancelled);

    let restarted =
        invoke_agent(&mut dispatcher, 34, &session_id, "new run").expect("restart succeeds");
    assert_eq!(restarted.session_id, session_id);
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_rejects_execute_target_outside_confirmed_scope() {
    let workspace = temp_workspace("dispatcher-agent-confirm-scope");
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent execute", Vec::new()).session_id;
    let request = confirmed_cadquery_request(path_handle(["parts", "top_lid.py"]));
    let confirmation = AgentCadQueryConfirmation {
        request,
        plan_ref: None,
        affected_files: vec![path_handle(["parts", "bottom.py"])],
        new_files: Vec::new(),
        export_targets: Vec::new(),
    };
    let started = invoke_agent_with_confirmation(&mut dispatcher, 35, &session_id, confirmation);

    wait_for_done(&pushes, &started.run_id);
    let error = find_error_event(&pushes, &started.run_id).expect("permission error");
    assert_eq!(error.error_type, AgentErrorType::PermissionDenied);
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_execute_agent_runs_confirmed_cadquery_and_records_tool_history() {
    let workspace = temp_workspace("dispatcher-agent-execute");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(workspace.join("parts/top_lid.py"), "old code\n").unwrap();
    let runner = fake_cadquery_runner(&workspace);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent execute", Vec::new()).session_id;
    let target_path = path_handle(["parts", "top_lid.py"]);
    let confirmation = AgentCadQueryConfirmation {
        request: confirmed_cadquery_request(target_path.clone()),
        plan_ref: None,
        affected_files: vec![target_path],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "top_lid.step"])],
    };
    let started = invoke_agent_with_confirmation(&mut dispatcher, 36, &session_id, confirmation);

    wait_for_done(&pushes, &started.run_id);
    let result = find_tool_result_event(&pushes, &started.run_id).expect("tool result");
    assert_eq!(result.tool_name, "cadquery");
    assert!(find_done_event(&pushes, &started.run_id).is_some());
    let history = read_chat_history(&mut dispatcher, &session_id);
    assert!(
        history
            .iter()
            .any(|message| message.role == ChatRole::Tool && message.tool_result.is_some())
    );
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_execute_agent_generates_cadquery_code_from_prompt() {
    let workspace = temp_workspace("dispatcher-agent-generate-code");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(workspace.join("parts/top_lid.py"), "old code\n").unwrap();
    let captured = workspace.join("captured-agent-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, false);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent execute", Vec::new()).session_id;
    let target_path = path_handle(["parts", "top_lid.py"]);
    let mut request = confirmed_cadquery_request(target_path.clone());
    request.code = "make a taller lid from chat".into();
    let confirmation = AgentCadQueryConfirmation {
        request,
        plan_ref: None,
        affected_files: vec![target_path],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "top_lid.step"])],
    };
    let started = invoke_agent_with_confirmation_and_prompt(
        &mut dispatcher,
        37,
        &session_id,
        "make a taller lid from chat",
        confirmation,
    );

    wait_for_done(&pushes, &started.run_id);
    let captured_code = std::fs::read_to_string(captured).expect("captured agent code");
    assert!(captured_code.contains("import cadquery as cq"));
    assert!(!captured_code.contains("make a taller lid from chat"));
    assert_eq!(
        std::fs::read_to_string(workspace.join("parts/top_lid.py")).unwrap(),
        captured_code
    );
    cleanup_workspace(&workspace);
}

#[test]
fn dispatcher_rejects_execute_outputs_outside_confirmed_scope() {
    let workspace = temp_workspace("dispatcher-agent-unconfirmed-output");
    std::fs::create_dir_all(workspace.join("parts")).unwrap();
    std::fs::write(workspace.join("parts/top_lid.py"), "old code\n").unwrap();
    let captured = workspace.join("captured-agent-code.py");
    let runner = fake_capturing_cadquery_runner(&workspace, &captured, true);
    let _env = EnvGuard::set("CADQUERY_RUNNER_PYTHON", runner.as_os_str());
    let (mut dispatcher, pushes) = dispatcher_with_pushes(&workspace);
    let session_id = create_chat(&mut dispatcher, "agent execute", Vec::new()).session_id;
    let target_path = path_handle(["parts", "top_lid.py"]);
    let confirmation = AgentCadQueryConfirmation {
        request: confirmed_cadquery_request(target_path.clone()),
        plan_ref: None,
        affected_files: vec![target_path],
        new_files: Vec::new(),
        export_targets: vec![path_handle(["outputs", "top_lid.step"])],
    };
    let started = invoke_agent_with_confirmation(&mut dispatcher, 38, &session_id, confirmation);

    wait_for_done(&pushes, &started.run_id);
    let error = find_error_event(&pushes, &started.run_id).expect("permission error");
    assert_eq!(error.error_type, AgentErrorType::PermissionDenied);
    assert_eq!(
        std::fs::read_to_string(workspace.join("parts/top_lid.py")).unwrap(),
        "old code\n"
    );
    assert!(!workspace.join("outputs/top_lid.step").exists());
    assert!(!workspace.join("outputs/unconfirmed.step").exists());
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

fn invoke_agent(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
    prompt: &str,
) -> Result<app_server_protocol::AgentStartedResponse, app_server_protocol::ProtocolError> {
    match dispatch(
        dispatcher,
        request_id,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            prompt: prompt.into(),
            operation: AgentOperationLevel::Inform,
            confirmed_cadquery: None,
        }),
    )
    .result?
    {
        CommandSuccess::AgentStarted(response) => Ok(response),
        other => panic!("unexpected agent.invoke response: {other:?}"),
    }
}

fn invoke_agent_with_confirmation(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
    confirmation: AgentCadQueryConfirmation,
) -> app_server_protocol::AgentStartedResponse {
    invoke_agent_with_confirmation_and_prompt(
        dispatcher,
        request_id,
        session_id,
        "execute confirmed cadquery",
        confirmation,
    )
}

fn invoke_agent_with_confirmation_and_prompt(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
    prompt: &str,
    confirmation: AgentCadQueryConfirmation,
) -> app_server_protocol::AgentStartedResponse {
    match dispatch(
        dispatcher,
        request_id,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            prompt: prompt.into(),
            operation: AgentOperationLevel::Execute,
            confirmed_cadquery: Some(confirmation),
        }),
    )
    .result
    .expect("agent.invoke succeeds")
    {
        CommandSuccess::AgentStarted(response) => response,
        other => panic!("unexpected agent.invoke response: {other:?}"),
    }
}

fn invoke_agent_error(
    dispatcher: &mut HostRequestDispatcher,
    request_id: u64,
    session_id: &ChatSessionId,
) -> app_server_protocol::ProtocolError {
    dispatch(
        dispatcher,
        request_id,
        ClientCommand::AgentInvoke(AgentInvokeRequest {
            session_id: session_id.clone(),
            prompt: "try again".into(),
            operation: AgentOperationLevel::Inform,
            confirmed_cadquery: None,
        }),
    )
    .result
    .expect_err("agent.invoke should fail")
}

fn cancel_agent(
    dispatcher: &mut HostRequestDispatcher,
    run_id: &str,
) -> app_server_protocol::AgentCancelledResponse {
    match dispatch(
        dispatcher,
        33,
        ClientCommand::AgentCancel(AgentCancelRequest {
            run_id: Some(run_id.into()),
        }),
    )
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
    dispatcher.dispatch_envelope(ClientRequestEnvelope {
        request_id: RequestId(request_id),
        command,
    })
}

fn path_handle<const N: usize>(segments: [&str; N]) -> PathHandle {
    PathHandle::new(WorkspaceId::new("workspace"), segments).expect("path handle")
}

fn wait_for_done(pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>, run_id: &str) {
    for _ in 0..30 {
        if find_done_event(pushes, run_id).is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("agent.done not observed for {run_id}");
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

fn find_tool_result_event(
    pushes: &Arc<Mutex<Vec<ServerPushEnvelope>>>,
    run_id: &str,
) -> Option<AgentToolResultEvent> {
    pushes
        .lock()
        .expect("push buffer lock")
        .iter()
        .find_map(|push| match &push.event {
            ServerPushEvent::AgentToolResult(event) if event.run_id == run_id => {
                Some(event.clone())
            }
            _ => None,
        })
}

fn handshake_request() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "dispatcher-test".into(),
            platform: ClientPlatform::Desktop,
            protocol_version: ProtocolVersionRange::new(2, 2),
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

fn fake_cadquery_runner(root: &std::path::Path) -> std::path::PathBuf {
    let runner = root.join("fake-cadquery-runner.sh");
    std::fs::write(
        &runner,
        format!(
            "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '--output-dir' ]; then\n    shift\n    out=\"$1\"\n  fi\n  shift\ndone\nif [ -n \"$out\" ]; then\n  mkdir -p \"$out\"\n  printf 'artifact\\n' > \"$out/top_lid.step\"\nfi\ncat <<'JSON'\n{}\nJSON\n",
            cadquery_success_json()
        ),
    )
    .expect("write fake cadquery runner");
    make_executable(&runner);
    runner
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
            "#!/bin/sh\nproject=''\nscript=''\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --project-root) shift; project=\"$1\" ;;\n    --script) shift; script=\"$1\" ;;\n    --output-dir) shift; out=\"$1\" ;;\n  esac\n  shift\ndone\nif [ -n \"$project\" ] && [ -n \"$script\" ]; then\n  cp \"$project/$script\" '{}'\nfi\nif [ -n \"$out\" ]; then\n  mkdir -p \"$out\"\n  printf 'artifact\\n' > \"$out/top_lid.step\"\n{}fi\ncat <<'JSON'\n{}\nJSON\n",
            capture_path.display(),
            extra_output,
            cadquery_success_json()
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

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
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
            key,
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = &self.previous {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
