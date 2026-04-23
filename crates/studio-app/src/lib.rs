use std::path::PathBuf;

use protocol_client::PreviewSuccess;
use scad_ui::tab_system::TabId;

pub mod app;
pub mod font_setup;
pub mod image_tab;
pub mod layout;
pub mod left_panel;
pub mod log_panel;
pub mod macos_fused_titlebar;
pub mod markdown_tab;
pub mod platform_menu;
pub mod platform_support;
pub mod protocol_client;
pub mod studio_document;
pub mod transport_port;
pub mod viewer_tab;
pub mod work_area;

#[derive(Debug, Clone)]
pub enum UserEvent {
    Menu(String),
    PreviewReady(
        winit::window::WindowId,
        TabId,
        u64,
        Result<PreviewSuccess, String>,
    ),
    SourceChanged(winit::window::WindowId, TabId, PathBuf),
    WatchError(winit::window::WindowId, TabId, String),
}

pub fn run_desktop_smoke_for_test(workspace: PathBuf) -> Result<(), String> {
    protocol_client::DesktopProtocolClient::run_smoke_check(workspace)
}
