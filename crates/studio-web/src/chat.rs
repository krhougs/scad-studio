#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeChatState {
    messages: Vec<ChatMessage>,
    draft: String,
}

impl Default for FakeChatState {
    fn default() -> Self {
        Self {
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                content: "This is a fake chat shell for the web slice.".into(),
            }],
            draft: String::new(),
        }
    }
}

impl FakeChatState {
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn set_draft(&mut self, draft: impl Into<String>) {
        self.draft = draft.into();
    }

    pub fn send_draft(&mut self) -> Option<String> {
        let draft = self.draft.trim().to_string();
        if draft.is_empty() {
            return None;
        }
        self.draft.clear();
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: draft.clone(),
        });
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "This is a fake assistant reply for Phase 6.".into(),
        });
        Some(draft)
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }
}
