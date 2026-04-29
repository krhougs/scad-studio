use std::fs;

use app_server_core::ChatStore;
use app_server_protocol::{
    ChatRole, ChatSessionId, ChatToolCallRecord, ChatToolResultRecord, PathHandle,
    ProtocolErrorCode, WorkspaceId,
};

#[test]
fn chat_store_creates_sends_reads_and_archives_jsonl_sessions() {
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
        .expect("create chat");
    assert_eq!(created.session_id, ChatSessionId("main-chat".into()));

    let ack = store
        .append_message(
            &created.session_id,
            ChatRole::User,
            "make the lid taller",
            vec![related.clone()],
            None,
        )
        .expect("append user message");
    assert_eq!(ack.session_id, created.session_id);
    assert!(ack.message_id.starts_with("msg-"));

    let sessions = store.list(false).expect("list sessions");
    assert_eq!(sessions.sessions.len(), 1);
    assert_eq!(sessions.sessions[0].message_count, 2);
    assert_eq!(sessions.sessions[0].related_files, vec![related.clone()]);

    let history = store
        .history(&created.session_id, Some(10))
        .expect("read history");
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[0].role, ChatRole::Meta);
    assert_eq!(history.messages[1].content, "make the lid taller");

    let archived = store.archive(&created.session_id).expect("archive chat");
    assert_eq!(archived.session_id, created.session_id);
    assert!(store.list(false).expect("list active").sessions.is_empty());
    assert_eq!(store.list(true).expect("list archived").sessions.len(), 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn chat_store_uses_unique_session_ids_for_repeated_titles() {
    let root = temp_dir("chat-store-ids");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());

    let first = store.create("main", None, Vec::new()).unwrap();
    let second = store.create("main", None, Vec::new()).unwrap();

    assert_eq!(first.session_id, ChatSessionId("main".into()));
    assert_eq!(second.session_id, ChatSessionId("main-2".into()));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn chat_store_persists_tool_call_and_result_records_in_history() {
    let root = temp_dir("chat-store-tool-history");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("agent tools", None, Vec::new()).unwrap();

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
        .expect("append tool result");
    assert_eq!(result_ack.message_id, "msg-3");

    let history = store.history(&created.session_id, Some(10)).unwrap();
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

#[test]
fn chat_store_rejects_untrusted_session_ids_before_path_join() {
    let root = temp_dir("chat-store-invalid-id");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::write(root.join("escape.jsonl"), "{}\n").unwrap();
    let store = ChatStore::new(root.clone());
    let invalid = ChatSessionId("../escape".into());

    let send_error = store
        .append_message(&invalid, ChatRole::User, "escape", Vec::new(), None)
        .expect_err("chat.send should reject path-like session id");
    assert_eq!(send_error.code, ProtocolErrorCode::InvalidCommand);

    let history_error = store
        .history(&invalid, None)
        .expect_err("chat.history should reject path-like session id");
    assert_eq!(history_error.code, ProtocolErrorCode::InvalidCommand);

    let archive_error = store
        .archive(&invalid)
        .expect_err("chat.archive should reject path-like session id");
    assert_eq!(archive_error.code, ProtocolErrorCode::InvalidCommand);
    assert!(root.join("escape.jsonl").is_file());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn chat_store_rejects_chats_symlink_escape() {
    let root = temp_dir("chat-store-symlink");
    let outside = temp_dir("chat-store-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .create("escaped chat", None, Vec::new())
        .expect_err("chat.create should reject symlinked chats directory");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(!outside.join("escaped-chat.jsonl").exists());
    let _ = fs::remove_file(root.join("chats"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn chat_store_rejects_archive_through_chats_symlink_escape() {
    let root = temp_dir("chat-store-archive-symlink");
    let outside = temp_dir("chat-store-archive-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(outside.join("archived")).unwrap();
    fs::write(outside.join("main.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .archive(&ChatSessionId("main".into()))
        .expect_err("chat.archive should reject symlinked chats parent");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(outside.join("main.jsonl").is_file());
    assert!(!outside.join("archived/main.jsonl").exists());
    let _ = fs::remove_file(root.join("chats"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn chat_store_rejects_archived_dir_symlink_escape() {
    let root = temp_dir("chat-store-archived-dir-symlink");
    let outside = temp_dir("chat-store-archived-dir-outside");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(root.join("chats/main.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("chats/archived")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .archive(&ChatSessionId("main".into()))
        .expect_err("chat.archive should reject symlinked archived directory");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    assert!(root.join("chats/main.jsonl").is_file());
    assert!(!outside.join("main.jsonl").exists());
    let _ = fs::remove_file(root.join("chats/archived"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn chat_store_rejects_jsonl_file_symlink_escape() {
    let root = temp_dir("chat-store-jsonl-symlink");
    let outside = temp_dir("chat-store-jsonl-outside");
    fs::create_dir_all(root.join("chats")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("main.jsonl"), "{}\n").unwrap();
    std::os::unix::fs::symlink(outside.join("main.jsonl"), root.join("chats/main.jsonl")).unwrap();
    let store = ChatStore::new(root.clone());

    let error = store
        .history(&ChatSessionId("main".into()), None)
        .expect_err("chat.history should reject symlinked JSONL file");

    assert_eq!(error.code, ProtocolErrorCode::InvalidPathHandle);
    let _ = fs::remove_file(root.join("chats/main.jsonl"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn old_jsonl_without_run_id_deserializes_with_none() {
    let root = temp_dir("chat-store-old-jsonl");
    fs::create_dir_all(root.join("chats")).unwrap();
    let jsonl_path = root.join("chats/old-session.jsonl");
    fs::write(
        &jsonl_path,
        "{\"message_id\":\"msg-1\",\"ts_ms\":100,\"role\":\"user\",\"content\":\"hello\",\"related_files\":[]}\n",
    )
    .unwrap();
    let store = ChatStore::new(root.clone());
    let history = store
        .history(&ChatSessionId("old-session".into()), None)
        .expect("should read old JSONL without run_id");
    assert_eq!(history.messages.len(), 1);
    assert_eq!(history.messages[0].run_id, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn run_id_roundtrips_through_jsonl() {
    let root = temp_dir("chat-store-run-id");
    fs::create_dir_all(&root).unwrap();
    let store = ChatStore::new(root.clone());
    let created = store.create("run-id test", None, Vec::new()).unwrap();

    store
        .append_message_with_run_id(
            &created.session_id,
            ChatRole::Assistant,
            "final answer",
            Vec::new(),
            None,
            Some("agent-7".into()),
        )
        .expect("append with run_id");

    let history = store
        .history(&created.session_id, Some(10))
        .expect("read history");
    assert_eq!(history.messages.len(), 2);
    assert_eq!(history.messages[0].run_id, None); // Meta message
    assert_eq!(history.messages[1].run_id, Some("agent-7".into()));
    let _ = fs::remove_dir_all(root);
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{}-{stamp}", std::process::id()))
}
