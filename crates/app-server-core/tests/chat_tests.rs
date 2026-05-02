use std::{fs, sync::Arc};

use app_server_core::{
    AGENT_ERROR_FACT_PREFIX, AgentTurnFinalFactKind, ChatStore, ChatSummaryUpdate,
};
use app_server_protocol::{
    AgentEventId, AgentEventPayload, AgentEventRecord, AgentId, AgentProviderType,
    AgentRuntimeStatus, AgentTurnId, BoundAgentModel, ChatRole, ChatSessionId, ChatToolCallRecord,
    ChatToolResultRecord, PathHandle, ProtocolErrorCode, WorkspaceId,
};
use serde_json::Value;

#[tokio::test]
async fn chat_store_creates_sends_reads_and_archives_jsonl_sessions() {
    let root = temp_dir("chat-store");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let related = PathHandle::new(WorkspaceId::new("ws"), ["parts", "top_lid.py"]).unwrap();

    let created = store
        .create(
            "main chat",
            Some("lid iteration".into()),
            vec![related.clone()],
        )
        .await
        .expect("create chat");
    assert_ne!(created.session_id, ChatSessionId("main-chat".into()));

    let ack = store
        .append_message(
            &created.session_id,
            ChatRole::User,
            "make the lid taller",
            vec![related.clone()],
            None,
        )
        .await
        .expect("append user message");
    assert_eq!(ack.session_id, created.session_id);
    assert!(ack.message_id.starts_with("msg-"));

    let sessions = store.list(false).await.expect("list sessions");
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(sessions.sessions[0].message_count, 2);
    assert_eq!(sessions.sessions[0].related_files, vec![related.clone()]);

    let history = store
        .history(&created.session_id, Some(10))
        .await
        .expect("read history");
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[0].role, ChatRole::Meta);
    assert_eq!(history.messages[1].content, "make the lid taller");

    let archived = store
        .archive(&created.session_id)
        .await
        .expect("archive chat");
    assert_eq!(archived.session_id, created.session_id);
    assert!(
        store
            .list(false)
            .await
            .expect("list active")
            .sessions
            .is_empty()
    );
    assert_eq!(
        store
            .list(true)
            .await
            .expect("list archived")
            .sessions
            .len(),
        1
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_creates_random_chat_id_and_chats_json_index() {
    let root = temp_dir("chat-store-index-create");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let created = store
        .create("Main Chat", Some("goal".into()), Vec::new())
        .await
        .unwrap();

    assert_ne!(created.session_id, ChatSessionId("main-chat".into()));
    assert!(root.join("chats.json").is_file());

    let index = read_chats_json(&root);
    let chats = index["chats"].as_array().expect("chats array");
    assert_eq!(chats.len(), 1);
    let entry = &chats[0];
    assert_eq!(
        entry["chat_id"].as_str(),
        Some(created.session_id.0.as_str())
    );
    assert_eq!(entry["title"].as_str(), Some("Main Chat"));
    assert!(
        entry["agent_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(entry["goal"].as_str(), Some("goal"));
    assert_eq!(
        entry["messages_path"].as_str(),
        Some(format!("chats/{}.jsonl", created.session_id.0).as_str())
    );
    assert_eq!(
        entry["events_path"].as_str(),
        Some(format!("agent-events/{}.jsonl", entry["agent_id"].as_str().unwrap()).as_str())
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_lists_sessions_in_chats_json_order_not_filesystem_order() {
    let root = temp_dir("chat-store-index-order");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let first = store.create("Zulu", None, Vec::new()).await.unwrap();
    let second = store.create("Alpha", None, Vec::new()).await.unwrap();

    let sessions = store.list(false).await.unwrap().sessions;

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_id, first.session_id);
    assert_eq!(sessions[1].session_id, second.session_id);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_reads_history_from_chats_json_messages_path() {
    let root = temp_dir("chat-store-index-history-path");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("Path Stable", None, Vec::new()).await.unwrap();

    let moved_path = root.join("renamed-history-file.jsonl");
    fs::rename(
        root.join(format!("chats/{}.jsonl", created.session_id.0)),
        &moved_path,
    )
    .unwrap();
    let mut index = read_chats_json(&root);
    index["chats"][0]["messages_path"] = Value::String("renamed-history-file.jsonl".into());
    fs::write(
        root.join("chats.json"),
        serde_json::to_string_pretty(&index).unwrap(),
    )
    .unwrap();

    let history = store.history(&created.session_id, None).await.unwrap();

    assert_eq!(history.session_id, created.session_id);
    assert_eq!(history.messages.len(), 1);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_reports_missing_indexed_messages_path_as_corrupt_index() {
    let root = temp_dir("chat-store-missing-indexed-messages-path");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store
        .create("Missing Messages", None, Vec::new())
        .await
        .unwrap();
    fs::remove_file(root.join(format!("chats/{}.jsonl", created.session_id.0))).unwrap();

    let error = store
        .history(&created.session_id, None)
        .await
        .expect_err("missing indexed messages path should be reported");

    assert_eq!(error.code, ProtocolErrorCode::NotFound);
    assert!(error.message.contains("chats.json"));
    assert!(error.message.contains("messages_path"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_deduplicates_create_by_client_request_id() {
    let root = temp_dir("chat-store-create-idempotent");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let first = store
        .create_with_client_request_id("request-1", "First", None, Vec::new())
        .await
        .unwrap();
    let second = store
        .create_with_client_request_id("request-1", "Retried", None, Vec::new())
        .await
        .unwrap();

    assert_eq!(first.session_id, second.session_id);
    let sessions = store.list(false).await.unwrap().sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "First");
    assert_eq!(
        store.list(false).await.unwrap().active_chat_id,
        Some(first.session_id.clone())
    );
    let history = store.history(&first.session_id, None).await.unwrap();
    assert_eq!(history.messages.len(), 1);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_deduplicates_first_user_message_by_client_request_id() {
    let root = temp_dir("chat-store-send-idempotent");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store
        .create_with_client_request_id("request-create", "First", None, Vec::new())
        .await
        .unwrap();

    let first = store
        .append_message_with_run_id(
            &created.session_id,
            ChatRole::User,
            "first prompt",
            Vec::new(),
            None,
            None,
            Some("request-create".into()),
        )
        .await
        .unwrap();
    let second = store
        .append_message_with_run_id(
            &created.session_id,
            ChatRole::User,
            "first prompt retried",
            Vec::new(),
            None,
            None,
            Some("request-create".into()),
        )
        .await
        .unwrap();

    assert_eq!(first.message_id, second.message_id);
    let history = store.history(&created.session_id, None).await.unwrap();
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[1].content, "first prompt");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_creates_initial_user_message_atomically_with_index() {
    let root = temp_dir("chat-store-create-initial-message");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let first = store
        .create_with_client_request_id_and_initial_message(
            "request-create",
            "First",
            None,
            Vec::new(),
            Some("first prompt".into()),
        )
        .await
        .unwrap();
    let second = store
        .create_with_client_request_id_and_initial_message(
            "request-create",
            "Retried",
            None,
            Vec::new(),
            Some("first prompt retried".into()),
        )
        .await
        .unwrap();

    assert_eq!(first.session_id, second.session_id);
    let history = store.history(&first.session_id, None).await.unwrap();
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[1].role, ChatRole::User);
    assert_eq!(history.messages[1].content, "first prompt");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_deduplicates_concurrent_first_user_message_by_client_request_id() {
    let root = temp_dir("chat-store-send-idempotent-concurrent");
    fs::create_dir_all(&root).unwrap();
    let store = Arc::new(ChatStore::new(root.clone()));
    let created = store
        .create_with_client_request_id("request-create", "First", None, Vec::new())
        .await
        .unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(16));
    let mut tasks = Vec::new();
    for index in 0..16 {
        let barrier = Arc::clone(&barrier);
        let store = Arc::clone(&store);
        let session_id = created.session_id.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store
                .append_message_with_run_id(
                    &session_id,
                    ChatRole::User,
                    &format!("first prompt {index}"),
                    Vec::new(),
                    None,
                    None,
                    Some("request-create".into()),
                )
                .await
        }));
    }

    let mut message_ids = Vec::new();
    for task in tasks {
        message_ids.push(task.await.unwrap().unwrap().message_id);
    }

    assert!(message_ids.iter().all(|id| id == &message_ids[0]));
    let history = store.history(&created.session_id, None).await.unwrap();
    assert_eq!(history.messages.len(), 2);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_deduplicates_concurrent_create_by_client_request_id() {
    let root = temp_dir("chat-store-create-idempotent-concurrent");
    fs::create_dir_all(&root).unwrap();
    let store = Arc::new(ChatStore::new(root.clone()));
    let mut tasks = Vec::new();
    for index in 0..12 {
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            store
                .create_with_client_request_id(
                    "request-concurrent",
                    &format!("Draft {index}"),
                    None,
                    Vec::new(),
                )
                .await
        }));
    }

    let mut ids = Vec::new();
    for task in tasks {
        ids.push(task.await.unwrap().unwrap().session_id);
    }

    assert!(ids.iter().all(|id| id == &ids[0]));
    let listed = store.list(false).await.unwrap();
    let sessions = listed.sessions;
    assert_eq!(sessions.len(), 1);
    assert_eq!(listed.active_chat_id, Some(ids[0].clone()));
    assert_eq!(
        store.history(&ids[0], None).await.unwrap().messages.len(),
        1
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_updates_active_chat_id_on_select_and_archive() {
    let root = temp_dir("chat-store-active-chat");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let first = store.create("First", None, Vec::new()).await.unwrap();
    let second = store.create("Second", None, Vec::new()).await.unwrap();

    store.select(&first.session_id).await.unwrap();
    assert_eq!(
        read_chats_json(&root)["active_chat_id"].as_str(),
        Some(first.session_id.0.as_str())
    );

    store.archive(&first.session_id).await.unwrap();
    assert_eq!(
        read_chats_json(&root)["active_chat_id"].as_str(),
        Some(second.session_id.0.as_str())
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_persists_summary_metadata_in_chats_json() {
    let root = temp_dir("chat-store-summary-index");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let related = PathHandle::new(WorkspaceId::new("ws"), ["docs", "note.md"]).unwrap();
    let created = store.create("Summary", None, Vec::new()).await.unwrap();

    store
        .update_summary(
            &created.session_id,
            ChatSummaryUpdate {
                summary: "current summary".into(),
                goal: "make progress".into(),
                related_files: vec![related.clone()],
                open_questions: vec!["Which material?".into()],
            },
        )
        .await
        .unwrap();

    let index = read_chats_json(&root);
    let entry = &index["chats"][0];
    let agent_id = entry["agent_id"].as_str().expect("agent id").to_owned();
    assert_eq!(entry["summary"].as_str(), Some("current summary"));
    assert_eq!(entry["goal"].as_str(), Some("make progress"));
    assert_eq!(entry["open_questions"][0].as_str(), Some("Which material?"));
    assert_eq!(entry["agent_id"].as_str(), Some(agent_id.as_str()));
    assert!(entry["created_at_ms"].as_u64().is_some());
    assert!(entry["updated_at_ms"].as_u64().is_some());
    assert!(entry["updated_at_ms"].as_u64() >= entry["created_at_ms"].as_u64());
    assert!(entry["messages_path"].as_str().is_some());
    assert!(entry["events_path"].as_str().is_some());
    assert!(entry["bound_model"].is_null());
    assert_eq!(
        store.list(false).await.unwrap().sessions[0].related_files,
        vec![related]
    );
    assert_eq!(
        store
            .history(&created.session_id, None)
            .await
            .unwrap()
            .messages
            .len(),
        1
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_rejects_corrupt_chats_json_without_filename_fallback() {
    let root = temp_dir("chat-store-corrupt-index");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::write(
        root.join("chats/legacy.jsonl"),
        "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
    )
    .unwrap();
    fs::write(root.join("chats.json"), "{not valid json").unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .list(false)
        .await
        .expect_err("corrupt index should fail");

    assert_eq!(error.code, ProtocolErrorCode::Internal);
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_chats_json_symlink() {
    let root = temp_dir("chat-store-index-symlink");
    let outside = temp_dir("chat-store-index-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("chats.json"),
        "{\"version\":1,\"active_chat_id\":null,\"chats\":[]}",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.join("chats.json"), root.join("chats.json")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .list(false)
        .await
        .expect_err("chats.json symlink should be rejected");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    let _ = fs::remove_file(root.join("chats.json"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_chats_json_tmp_symlink() {
    let root = temp_dir("chat-store-index-tmp-symlink");
    let outside = temp_dir("chat-store-index-tmp-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("target"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.join("target"), root.join("chats.json.tmp")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .create_with_client_request_id("request-1", "Main", None, Vec::new())
        .await
        .expect_err("chats.json.tmp symlink should be rejected");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert_eq!(
        fs::read_to_string(outside.join("target")).unwrap(),
        "outside"
    );
    let _ = fs::remove_file(root.join("chats.json.tmp"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_removes_create_files_when_event_log_creation_fails() {
    let root = temp_dir("chat-store-create-cleanup");
    let outside = temp_dir("chat-store-create-cleanup-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("agent-events")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .create_with_client_request_id("request-1", "Main", None, Vec::new())
        .await
        .expect_err("symlinked agent-events should fail create");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    let chat_files = fs::read_dir(root.join("chats"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(chat_files, 0);
    let _ = fs::remove_file(root.join("agent-events"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[tokio::test]
async fn chat_store_uses_random_session_ids_for_repeated_titles() {
    let root = temp_dir("chat-store-ids");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let first = store.create("main", None, Vec::new()).await.unwrap();
    let second = store.create("main", None, Vec::new()).await.unwrap();

    assert_ne!(first.session_id, ChatSessionId("main".into()));
    assert_ne!(second.session_id, ChatSessionId("main-2".into()));
    assert_ne!(first.session_id, second.session_id);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_persists_tool_call_and_result_records_in_history() {
    let root = temp_dir("chat-store-tool-history");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("agent tools", None, Vec::new()).await.unwrap();

    let call_ack = store
        .append_tool_call(
            &created.session_id,
            "agent tool started",
            ChatToolCallRecord {
                tool_call_id: "call_read".into(),
                tool_name: "read_file".into(),
                args_json: "{\"path\":\"README.md\"}".into(),
            },
        )
        .await
        .expect("append tool call");
    assert_eq!(call_ack.message_id, "msg-2");

    let result_ack = store
        .append_tool_result(
            &created.session_id,
            "agent tool completed",
            ChatToolResultRecord {
                tool_call_id: "call_read".into(),
                tool_name: "read_file".into(),
                result_json: "{\"status\":\"ok\"}".into(),
            },
            None,
        )
        .await
        .expect("append tool result");
    assert_eq!(result_ack.message_id, "msg-3");

    let history = store.history(&created.session_id, Some(10)).await.unwrap();
    assert_eq!(history.messages.len(), 3);
    let call_message = &history.messages[1];
    assert_eq!(call_message.role, ChatRole::Assistant);
    assert_eq!(call_message.tool_call_id.as_deref(), Some("call_read"));
    assert_eq!(call_message.tool_calls.len(), 1);
    assert_eq!(call_message.tool_calls[0].tool_name, "read_file");

    let result_message = &history.messages[2];
    assert_eq!(result_message.role, ChatRole::Tool);
    assert_eq!(result_message.tool_call_id.as_deref(), Some("call_read"));
    let result = result_message.tool_result.as_ref().unwrap();
    assert_eq!(result.tool_name, "read_file");
    assert_eq!(result.result_json, "{\"status\":\"ok\"}");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_persists_bound_model_in_chats_json_on_first_create() {
    let root = temp_dir("chat-store-bound-model");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let model = BoundAgentModel {
        provider_id: "openai".into(),
        provider_type: AgentProviderType::OpenAiResponses,
        model_id: "gpt-5.2".into(),
        reasoning_effort: Some("high".into()),
        service_label: Some("flex".into()),
    };

    let created = store
        .create_with_client_request_id_initial_message_and_model(
            "first-send-1",
            "Bound Model",
            None,
            Vec::new(),
            "make a bracket",
            Some(model.clone()),
        )
        .await
        .unwrap();

    let index = read_chats_json(&root);
    let entry = &index["chats"][0];
    assert_eq!(
        entry["chat_id"].as_str(),
        Some(created.session_id.0.as_str())
    );
    assert_eq!(
        entry["agent_id"].as_str(),
        Some(created.agent_id.0.as_str())
    );
    assert_eq!(entry["bound_model"]["provider_id"].as_str(), Some("openai"));
    assert_eq!(
        entry["bound_model"]["provider_type"].as_str(),
        Some("openai_responses")
    );
    assert_eq!(entry["bound_model"]["model_id"].as_str(), Some("gpt-5.2"));
    assert_eq!(entry["bound_model"].get("base_url"), None);

    let listed = store.list(false).await.unwrap();
    assert_eq!(listed.sessions[0].bound_model.as_ref(), Some(&model));
    assert_eq!(listed.sessions[0].agent_id, created.agent_id);
    assert_eq!(
        store
            .bound_model_for_session(&created.session_id)
            .await
            .unwrap(),
        Some(model.clone())
    );
    assert_eq!(
        store
            .bound_model_for_agent(&created.agent_id)
            .await
            .unwrap(),
        Some(model)
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_rejects_untrusted_session_ids_before_path_join() {
    let root = temp_dir("chat-store-invalid-id");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::write(root.join("escape.jsonl"), "{}\n").unwrap();
    let store = ChatStore::new(root.clone());
    let invalid = ChatSessionId("../escape".into());

    let send_error = store
        .append_message(&invalid, ChatRole::User, "escape", Vec::new(), None)
        .await
        .expect_err("chat.send should reject path-like session id");
    assert_eq!(send_error.code, ProtocolErrorCode::InvalidCommand);

    let history_error = store
        .history(&invalid, None)
        .await
        .expect_err("chat.history should reject path-like session id");
    assert_eq!(history_error.code, ProtocolErrorCode::InvalidCommand);

    let archive_error = store
        .archive(&invalid)
        .await
        .expect_err("chat.archive should reject path-like session id");
    assert_eq!(archive_error.code, ProtocolErrorCode::InvalidCommand);
    assert!(root.join("escape.jsonl").is_file());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_chats_symlink_escape() {
    let root = temp_dir("chat-store-symlink");
    let outside = temp_dir("chat-store-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .create("escaped chat", None, Vec::new())
        .await
        .expect_err("chat.create should reject symlinked chats directory");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(!outside.join("escaped-chat.jsonl").exists());
    let _ = fs::remove_file(root.join("chats"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_archive_through_chats_symlink_escape() {
    let root = temp_dir("chat-store-archive-symlink");
    let outside = temp_dir("chat-store-archive-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(outside.join("archived")).unwrap();
    fs::write(outside.join("main.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .archive(&ChatSessionId("main".into()))
        .await
        .expect_err("chat.archive should reject symlinked chats parent");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(outside.join("main.jsonl").is_file());
    assert!(!outside.join("archived/main.jsonl").exists());
    let _ = fs::remove_file(root.join("chats"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_archived_dir_symlink_escape() {
    let root = temp_dir("chat-store-archived-dir-symlink");
    let outside = temp_dir("chat-store-archived-dir-outside");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        root.join("chats/main.jsonl"),
        "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats/archived")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .archive(&ChatSessionId("main".into()))
        .await
        .expect_err("chat.archive should reject symlinked archived directory");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(root.join("chats/main.jsonl").is_file());
    assert!(!outside.join("main.jsonl").exists());
    let _ = fs::remove_file(root.join("chats/archived"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[tokio::test]
async fn chat_store_rejects_jsonl_file_symlink_escape() {
    let root = temp_dir("chat-store-jsonl-symlink");
    let outside = temp_dir("chat-store-jsonl-outside");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("main.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(outside.join("main.jsonl"), root.join("chats/main.jsonl")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .history(&ChatSessionId("main".into()), None)
        .await
        .expect_err("chat.history should reject symlinked JSONL file");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    let _ = fs::remove_file(root.join("chats/main.jsonl"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[tokio::test]
async fn old_jsonl_without_run_id_deserializes_with_none() {
    let root = temp_dir("chat-store-old-jsonl");
    fs::create_dir_all(root.join("chats")).unwrap();
    let jsonl_path = root.join("chats/old-session.jsonl");
    fs::write(
        &jsonl_path,
        "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
    )
    .unwrap();
    let store = ChatStore::new(root.clone());
    let migrated = store
        .list(false)
        .await
        .expect("migrate old JSONL")
        .sessions
        .into_iter()
        .find(|session| session.title == "old-session")
        .expect("migrated old session");
    assert_ne!(migrated.session_id, ChatSessionId("old-session".into()));
    let history = store.history(&migrated.session_id, None).await.unwrap();
    assert_eq!(history.messages.len(), 1);
    assert_eq!(history.messages[0].run_id, None);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn legacy_jsonl_migration_serializes_concurrent_index_writes() {
    let root = temp_dir("chat-store-concurrent-migration");
    fs::create_dir_all(root.join("chats")).unwrap();
    for index in 0..8 {
        fs::write(
            root.join(format!("chats/legacy-{index}.jsonl")),
            "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
        )
        .unwrap();
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(32));
    let store = Arc::new(ChatStore::new(root.clone()));
    let mut tasks = Vec::new();
    for _ in 0..32 {
        let barrier = Arc::clone(&barrier);
        let store = Arc::clone(&store);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store.list(false).await
        }));
    }

    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }

    assert!(results.iter().all(Result::is_ok));
    let index = read_chats_json(&root);
    assert_eq!(index["chats"].as_array().unwrap().len(), 8);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn legacy_jsonl_migration_does_not_activate_archived_only_chat() {
    let root = temp_dir("chat-store-archived-only-migration");
    fs::create_dir_all(root.join("chats/archived")).unwrap();
    fs::write(
        root.join("chats/archived/old.jsonl"),
        "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
    )
    .unwrap();
    let store = ChatStore::new(root.clone());

    let active = store.list(false).await.unwrap();
    let archived = store.list(true).await.unwrap();

    assert!(active.sessions.is_empty());
    assert_eq!(active.active_chat_id, None);
    assert_eq!(archived.sessions.len(), 1);
    assert_eq!(archived.active_chat_id, None);
    assert_eq!(read_chats_json(&root)["active_chat_id"], Value::Null);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn run_id_roundtrips_through_jsonl() {
    let root = temp_dir("chat-store-run-id");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("run-id test", None, Vec::new()).await.unwrap();

    store
        .append_message_with_run_id(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            Vec::new(),
            None,
            Some("agent-7".into()),
            None,
        )
        .await
        .expect("append with run_id");

    let history = store
        .history(&created.session_id, Some(10))
        .await
        .expect("read history");
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[0].run_id, None); // Meta message
    assert_eq!(history.messages[1].run_id, Some("agent-7".into()));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn agent_turn_refs_roundtrip_through_final_chat_jsonl_records() {
    let root = temp_dir("chat-store-agent-turn-ref");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store
        .create("agent turn refs", None, Vec::new())
        .await
        .unwrap();
    let agent_id = AgentId("agent-main".into());
    let turn_id = AgentTurnId("agent-7".into());

    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &agent_id,
            &turn_id,
            Some("agent-7".into()),
        )
        .await
        .expect("append assistant with agent turn refs");
    store
        .append_tool_call_with_agent_turn(
            &created.session_id,
            "agent tool started",
            ChatToolCallRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "read_file".into(),
                args_json: "{}".into(),
            },
            &agent_id,
            &turn_id,
            Some("agent-7".into()),
        )
        .await
        .expect("append tool call with agent turn refs");
    store
        .append_tool_result_with_agent_turn(
            &created.session_id,
            "agent tool completed",
            ChatToolResultRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "read_file".into(),
                result_json: "{\"ok\":true}".into(),
            },
            None,
            &agent_id,
            &turn_id,
            Some("agent-7".into()),
        )
        .await
        .expect("append tool result with agent turn refs");

    let history = store
        .history(&created.session_id, Some(10))
        .await
        .expect("read history");
    for message in history.messages.iter().skip(1) {
        assert_eq!(message.agent_id, Some(agent_id.clone()));
        assert_eq!(message.turn_id, Some(turn_id.clone()));
        assert_eq!(message.run_id.as_deref(), Some("agent-7"));
    }
    assert!(
        store
            .agent_turn_has_final_fact(&created.session_id, &agent_id, &turn_id)
            .await
            .expect("final fact lookup")
    );
    assert!(
        !store
            .agent_turn_has_final_fact(
                &created.session_id,
                &agent_id,
                &AgentTurnId("missing-turn".into()),
            )
            .await
            .expect("missing final fact lookup")
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_turn_final_fact_ignores_tool_intermediate_records() {
    let root = temp_dir("chat-store-final-fact-ignore-tools");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("tool only", None, Vec::new()).await.unwrap();
    let turn_id = AgentTurnId("agent-7".into());
    store
        .append_tool_call_with_agent_turn(
            &created.session_id,
            "agent tool started",
            ChatToolCallRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "read_file".into(),
                args_json: "{}".into(),
            },
            &created.agent_id,
            &turn_id,
            Some("agent-7".into()),
        )
        .await
        .expect("append tool call");
    store
        .append_tool_result_with_agent_turn(
            &created.session_id,
            "agent tool completed",
            ChatToolResultRecord {
                tool_call_id: "tool-1".into(),
                tool_name: "read_file".into(),
                result_json: "{\"ok\":true}".into(),
            },
            None,
            &created.agent_id,
            &turn_id,
            Some("agent-7".into()),
        )
        .await
        .expect("append tool result");

    assert!(
        !store
            .agent_turn_has_final_fact(&created.session_id, &created.agent_id, &turn_id)
            .await
            .expect("final fact lookup")
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_classifies_agent_turn_final_fact_kind() {
    let root = temp_dir("chat-store-final-fact-kind");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store
        .create("final fact kind", None, Vec::new())
        .await
        .unwrap();
    let success_turn_id = AgentTurnId("agent-7".into());
    let failed_turn_id = AgentTurnId("agent-8".into());
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            &created.agent_id,
            &success_turn_id,
            Some(success_turn_id.0.clone()),
        )
        .await
        .expect("append success final fact");
    store
        .append_message_with_agent_turn(
            &created.session_id,
            ChatRole::Assistant,
            &format!("{AGENT_ERROR_FACT_PREFIX} (LlmError): Rig Agent is not configured"),
            &created.agent_id,
            &failed_turn_id,
            Some(failed_turn_id.0.clone()),
        )
        .await
        .expect("append failed final fact");

    assert_eq!(
        store
            .agent_turn_final_fact_kind(&created.session_id, &created.agent_id, &success_turn_id)
            .await
            .expect("success final fact lookup"),
        Some(AgentTurnFinalFactKind::Success)
    );
    assert_eq!(
        store
            .agent_turn_final_fact_kind(&created.session_id, &created.agent_id, &failed_turn_id)
            .await
            .expect("failed final fact lookup"),
        Some(AgentTurnFinalFactKind::Failure)
    );
    assert!(
        store
            .agent_turn_has_final_fact(&created.session_id, &created.agent_id, &failed_turn_id)
            .await
            .expect("failed final fact boolean lookup")
    );
    assert_eq!(
        store
            .latest_agent_turn_final_fact(&created.session_id, &created.agent_id)
            .await
            .expect("latest final fact lookup")
            .map(|fact| (fact.turn_id, fact.kind)),
        Some((failed_turn_id.clone(), AgentTurnFinalFactKind::Failure))
    );
    assert_eq!(
        store
            .max_agent_turn_run_id(&created.session_id, &created.agent_id)
            .await
            .expect("max agent turn run id lookup"),
        Some(8)
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_reports_missing_agent_event_log_from_index() {
    let root = temp_dir("chat-store-missing-agent-event-log");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store
        .create("missing event log", None, Vec::new())
        .await
        .unwrap();
    fs::remove_file(
        root.join("agent-events")
            .join(format!("{}.jsonl", created.agent_id.0)),
    )
    .unwrap();

    let error = store
        .read_agent_events(&created.agent_id, None)
        .await
        .expect_err("missing event log should be rejected");

    assert_eq!(error.code, ProtocolErrorCode::NotFound);
    assert!(
        error.message.contains("Agent event log"),
        "unexpected error message: {}",
        error.message
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn chat_store_turn_final_fact_ignores_runtime_event_log() {
    let root = temp_dir("chat-store-final-fact-ignore-events");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("event only", None, Vec::new()).await.unwrap();
    let turn_id = AgentTurnId("agent-7".into());
    store
        .append_agent_event(
            &created.agent_id,
            &AgentEventRecord {
                event_id: AgentEventId(1),
                agent_id: created.agent_id.clone(),
                turn_id: Some(turn_id.clone()),
                ts_ms: 100,
                payload: AgentEventPayload::StateChanged {
                    state: AgentRuntimeStatus::Running,
                },
            },
        )
        .await
        .expect("append runtime event");

    assert!(
        !store
            .agent_turn_has_final_fact(&created.session_id, &created.agent_id, &turn_id)
            .await
            .expect("final fact lookup")
    );
    let _ = fs::remove_dir_all(root);
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{}-{stamp}", std::process::id()))
}

fn read_chats_json(root: &std::path::Path) -> Value {
    let content = fs::read_to_string(root.join("chats.json")).expect("read chats.json");
    serde_json::from_str(&content).expect("parse chats.json")
}
