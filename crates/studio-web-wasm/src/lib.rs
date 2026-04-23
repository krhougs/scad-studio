#[cfg(feature = "legacy-shell")]
mod chat;

#[cfg(feature = "legacy-shell")]
pub use chat::{ChatMessage, FakeChatState, MessageRole};

#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
mod legacy_dom_shell;
#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
mod preview_canvas;
#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
mod transport_port;

#[cfg(target_arch = "wasm32")]
pub mod wasm_bridge;

#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
#[wasm_bindgen]
pub fn boot_studio_web(url: &str) -> Result<(), wasm_bindgen::JsValue> {
    legacy_dom_shell::boot_studio_web(url)
}

#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]
#[wasm_bindgen]
pub fn boot_studio_web_from_window() -> Result<(), wasm_bindgen::JsValue> {
    legacy_dom_shell::boot_studio_web_from_window()
}

pub fn web_shell_label() -> &'static str {
    "studio-web-wasm shell"
}
