mod chat;

pub use chat::{ChatMessage, FakeChatState, MessageRole};

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod preview_canvas;
#[cfg(target_arch = "wasm32")]
mod transport_port;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn boot_studio_web(url: &str) -> Result<(), wasm_bindgen::JsValue> {
    app::boot_studio_web(url)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn boot_studio_web_from_window() -> Result<(), wasm_bindgen::JsValue> {
    app::boot_studio_web_from_window()
}

pub fn web_shell_label() -> &'static str {
    "studio-web wasm shell"
}
