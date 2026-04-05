use scad_ui::chat_panel::{ChatAction, ChatPanel, MessageRole};

#[test]
fn sending_message_appends_user_message_and_placeholder_reply() {
    let mut chat = ChatPanel::default();
    let initial_len = chat.messages().len();

    chat.set_input_text("Help me structure the file tree");
    let action = chat.submit_input();

    assert_eq!(
        action,
        Some(ChatAction::SendMessage(
            "Help me structure the file tree".to_string()
        ))
    );
    assert_eq!(chat.messages().len(), initial_len + 2);
    assert_eq!(chat.messages()[initial_len].role, MessageRole::User);
    assert_eq!(
        chat.messages()[initial_len + 1].role,
        MessageRole::Assistant
    );
    assert!(chat.messages()[initial_len + 1].content.contains("Agent"));
    assert!(chat.input_text().is_empty());
}
