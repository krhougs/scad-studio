use std::fs;

use app_server_core::ChatStore;
use app_server_protocol::{ChatRole, ChatSessionId, PathHandle, ProtocolErrorCode, WorkspaceId};

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

fn temp_dir(label: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{}-{stamp}", std::process::id()))
}
