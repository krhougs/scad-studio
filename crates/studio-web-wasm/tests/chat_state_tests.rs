#![cfg(feature = "legacy-shell")]

use studio_web_wasm::{FakeChatState, MessageRole};

#[test]
fn send_message_appends_user_and_placeholder_reply() {
    let mut chat = FakeChatState::default();
    let initial_len = chat.messages().len();

    chat.set_draft("Preview this workspace");
    let sent = chat.send_draft();

    assert_eq!(sent.as_deref(), Some("Preview this workspace"));
    assert_eq!(chat.messages().len(), initial_len + 2);
    assert_eq!(chat.messages()[initial_len].role, MessageRole::User);
    assert_eq!(
        chat.messages()[initial_len + 1].role,
        MessageRole::Assistant
    );
    assert!(chat.messages()[initial_len + 1].content.contains("fake"));
    assert!(chat.draft().is_empty());
}

#[test]
fn clear_conversation_keeps_empty_state_without_touching_draft() {
    let mut chat = FakeChatState::default();
    chat.set_draft("Keep this input");
    chat.clear_messages();

    assert!(chat.messages().is_empty());
    assert_eq!(chat.draft(), "Keep this input");
}
