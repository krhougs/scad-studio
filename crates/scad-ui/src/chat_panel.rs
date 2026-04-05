use crate::{
    theme::{self, palette},
    widgets::section_label,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    SendMessage(String),
}

#[derive(Debug, Clone)]
pub struct ChatPanel {
    messages: Vec<ChatMessage>,
    input_text: String,
    scroll_to_bottom: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            messages: sample_messages(),
            input_text: String::new(),
            scroll_to_bottom: true,
        }
    }
}

impl ChatPanel {
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn input_text(&self) -> &str {
        &self.input_text
    }

    pub fn set_input_text(&mut self, text: impl Into<String>) {
        self.input_text = text.into();
    }

    pub fn submit_input(&mut self) -> Option<ChatAction> {
        let text = self.input_text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.input_text.clear();
        self.push_user_message(text.clone());
        Some(ChatAction::SendMessage(text))
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<ChatAction> {
        let mut action = None;

        theme::floating_frame(1.0).show(ui, |ui| {
            section_label(ui, "agent chat");

            let scroll = egui::ScrollArea::vertical().stick_to_bottom(self.scroll_to_bottom);
            scroll.show(ui, |ui| {
                for message in &self.messages {
                    render_message(ui, message);
                    ui.add_space(6.0);
                }
            });
            self.scroll_to_bottom = false;

            ui.separator();
            ui.add_space(4.0);
            let mut send_now = false;
            ui.horizontal(|ui| {
                let editor = egui::TextEdit::multiline(&mut self.input_text)
                    .desired_rows(3)
                    .hint_text("输入消息，Enter 发送，Shift+Enter 换行");
                let response = ui.add_sized([ui.available_width() - 56.0, 72.0], editor);
                if response.has_focus() {
                    let enter_pressed = ui.input(|input| {
                        input.key_pressed(egui::Key::Enter) && !input.modifiers.shift
                    });
                    if enter_pressed {
                        send_now = true;
                    }
                }
                let send_enabled = !self.input_text.trim().is_empty();
                let send_button =
                    egui::Button::new(egui::RichText::new("发送").color(if send_enabled {
                        palette::TEXT_PRIMARY
                    } else {
                        palette::TEXT_SECONDARY
                    }))
                    .fill(if send_enabled {
                        palette::BG_WIDGET
                    } else {
                        palette::BG_PANEL
                    })
                    .corner_radius(egui::CornerRadius::same(3))
                    .min_size(egui::vec2(28.0, 0.0));
                if ui.add_enabled(send_enabled, send_button).clicked() {
                    send_now = true;
                }
            });

            if send_now {
                action = self.submit_input();
            }
        });

        action
    }

    fn push_user_message(&mut self, text: String) {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: text,
            timestamp: "刚刚".to_string(),
        });
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: "[Agent 功能开发中...]".to_string(),
            timestamp: "刚刚".to_string(),
        });
        self.scroll_to_bottom = true;
    }
}

fn render_message(ui: &mut egui::Ui, message: &ChatMessage) {
    let (align, fill, text_color) = match message.role {
        MessageRole::User => (egui::Align::Max, palette::ACCENT, palette::TEXT_BRIGHT),
        MessageRole::Assistant => (egui::Align::Min, palette::BG_WIDGET, palette::TEXT_PRIMARY),
    };

    ui.with_layout(egui::Layout::left_to_right(align), |ui| {
        let frame = egui::Frame::default()
            .fill(fill)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 8));
        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(&message.content)
                        .color(text_color)
                        .size(13.0),
                );
                ui.label(
                    egui::RichText::new(&message.timestamp)
                        .color(palette::TEXT_SECONDARY)
                        .size(10.0),
                );
            });
        });
    });
}

fn sample_messages() -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: MessageRole::Assistant,
            content: "我可以帮你浏览 workspace 目录结构。".to_string(),
            timestamp: "09:18".to_string(),
        },
        ChatMessage {
            role: MessageRole::User,
            content: "先看一下文件树排序规则。".to_string(),
            timestamp: "09:19".to_string(),
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: "目录优先，文件按名称排序。".to_string(),
            timestamp: "09:19".to_string(),
        },
    ]
}
